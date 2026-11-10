//! Cyclic-Write-Scheduler: rewrite Active Setpoints every N seconds (default **60 s**).
//!
//! - First user write requires `confirmed: true`; cyclic rewrite does **not**.
//! - `persistence_class: cyclic` → Active entry after successful user write.
//! - `retained` → no Active entry unless caller uses [`CyclicWriteScheduler::activate_retained`].
//! - Pause while disconnected; optional immediate rewrite pass after connect when Active ≠ empty.
//! - Rewrite errors keep the entry (F-43) and retry next cycle.
//! - Drive with [`CyclicWriteScheduler::tick`] + injectable [`Clock`] / short test intervals.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use lambda_catalog::{
    decode_display, decode_raw, encode_display, Access, Catalog, Parameter, PersistenceClass,
};
use lambda_modbus::{ModbusError, ModbusTransport};

use crate::active::{wall_now_iso8601, ActiveSetpoint, ActiveSetpointStore, GroupValue};
use crate::clock::{Clock, SystemClock};
use crate::value::ValueQuality;
use crate::{ConnectionStatus, Event, WriteSource};

/// Production default cyclic-write interval (locked v1 / `config.cyclic_write_interval_secs`).
pub const DEFAULT_CYCLIC_WRITE_INTERVAL: Duration =
    Duration::from_secs(crate::defaults::CYCLIC_WRITE_INTERVAL_SECS as u64);

/// Configuration for the cyclic-write scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CyclicWriteConfig {
    pub unit_id: u8,
    /// Interval between automatic rewrite passes while connected. Production default: 60 s.
    pub write_interval: Duration,
    /// When true (default), run one rewrite pass immediately after connect if Active ≠ empty.
    pub rewrite_on_connect: bool,
}

impl Default for CyclicWriteConfig {
    fn default() -> Self {
        Self {
            unit_id: crate::defaults::UNIT_ID,
            write_interval: DEFAULT_CYCLIC_WRITE_INTERVAL,
            rewrite_on_connect: true,
        }
    }
}

/// Orchestrates confirmed user writes and 60 s Active-Setpoint rewrites.
#[derive(Debug)]
pub struct CyclicWriteScheduler<T: ModbusTransport, C: Clock = SystemClock> {
    transport: T,
    clock: C,
    catalog: &'static Catalog,
    config: CyclicWriteConfig,
    status: ConnectionStatus,
    store: ActiveSetpointStore,
    /// Instant of last rewrite pass that wrote at least one entry successfully.
    last_success_at: Option<Instant>,
    /// When the next timer-driven rewrite is due (`None` = paused).
    next_write_at: Option<Instant>,
    events: VecDeque<Event>,
}

impl<T: ModbusTransport> CyclicWriteScheduler<T, SystemClock> {
    /// Create a scheduler with the system clock and official catalog.
    pub fn new(transport: T, config: CyclicWriteConfig) -> Self {
        Self::with_clock(transport, config, SystemClock, Catalog::official())
    }
}

impl<T: ModbusTransport, C: Clock> CyclicWriteScheduler<T, C> {
    /// Create a scheduler with an injectable clock (tests) and catalog.
    pub fn with_clock(
        transport: T,
        config: CyclicWriteConfig,
        clock: C,
        catalog: &'static Catalog,
    ) -> Self {
        Self {
            transport,
            clock,
            catalog,
            config,
            status: ConnectionStatus::Disconnected,
            store: ActiveSetpointStore::new(),
            last_success_at: None,
            next_write_at: None,
            events: VecDeque::new(),
        }
    }

    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }

    pub fn config(&self) -> &CyclicWriteConfig {
        &self.config
    }

    pub fn write_interval(&self) -> Duration {
        self.config.write_interval
    }

    pub fn last_success_at(&self) -> Option<Instant> {
        self.last_success_at
    }

    pub fn next_write_at(&self) -> Option<Instant> {
        self.next_write_at
    }

    pub fn store(&self) -> &ActiveSetpointStore {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut ActiveSetpointStore {
        &mut self.store
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Drain queued events (oldest first).
    pub fn drain_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }

    fn push_event(&mut self, event: Event) {
        self.events.push_back(event);
    }

    fn set_status(&mut self, status: ConnectionStatus) {
        self.status = status.clone();
        self.push_event(Event::ConnectionChanged { status });
    }

    fn is_write_allowed(&self) -> bool {
        matches!(self.status, ConnectionStatus::Connected) && self.transport.is_connected()
    }

    /// Connect the transport; optionally run an immediate Active rewrite pass.
    pub fn connect(&mut self, host: &str, port: u16) -> Result<(), ModbusError> {
        self.set_status(ConnectionStatus::Connecting);
        match self.transport.connect(host, port) {
            Ok(()) => {
                self.set_status(ConnectionStatus::Connected);
                if self.config.rewrite_on_connect && !self.store.is_empty() {
                    self.run_cyclic_pass();
                } else if self.is_write_allowed() {
                    self.next_write_at = Some(self.clock.now() + self.config.write_interval);
                }
                Ok(())
            }
            Err(err) => {
                self.next_write_at = None;
                self.set_status(ConnectionStatus::Error {
                    message: err.to_string(),
                });
                Err(err)
            }
        }
    }

    /// Disconnect and pause the cyclic timer. Active entries remain.
    pub fn disconnect(&mut self) {
        self.transport.disconnect();
        self.next_write_at = None;
        self.set_status(ConnectionStatus::Disconnected);
    }

    /// Timer step: if due and connected, rewrite all Active entries.
    ///
    /// Returns `true` when a cyclic pass ran.
    pub fn tick(&mut self) -> bool {
        if !self.is_write_allowed() {
            return false;
        }
        let Some(due) = self.next_write_at else {
            return false;
        };
        if self.clock.now() < due {
            return false;
        }
        self.run_cyclic_pass();
        true
    }

    /// Confirmed user write of a single register (`WriteRegister`).
    ///
    /// Requires `confirmed == true`. On success, cyclic params enter the Active store.
    pub fn write_register(
        &mut self,
        param_id: &str,
        display_value: f64,
        confirmed: bool,
    ) -> Result<(), Event> {
        if !confirmed {
            let ev = Event::WriteRejected {
                reason: "Schreibvorgang nicht bestätigt".into(),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }
        if !self.is_write_allowed() {
            let ev = Event::WriteRejected {
                reason: "Nicht verbunden".into(),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }

        let param = match self.catalog.get(param_id) {
            Some(p) => p.clone(),
            None => {
                let ev = Event::WriteRejected {
                    reason: format!("Parameter nicht im Katalog: {param_id}"),
                };
                self.push_event(ev.clone());
                return Err(ev);
            }
        };

        if param.access != Access::ReadWrite {
            let ev = Event::WriteRejected {
                reason: format!("Parameter nicht schreibbar: {param_id}"),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }

        if let Some(group_id) = param.write_group_id.as_deref() {
            let ev = Event::WriteRejected {
                reason: format!(
                    "Parameter gehört zur Schreibgruppe {group_id} — WriteRegisterGroup verwenden"
                ),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }

        let words = match encode_display(&param, display_value) {
            Ok(w) => w,
            Err(err) => {
                let ev = Event::WriteRejected {
                    reason: err.to_string(),
                };
                self.push_event(ev.clone());
                return Err(ev);
            }
        };

        match self
            .transport
            .write_multiple(self.config.unit_id, param.address, &words)
        {
            Ok(()) => {
                self.after_user_single_write(&param, display_value);
                Ok(())
            }
            Err(err) => {
                let ev = Event::WriteFailed {
                    param_id: Some(param_id.to_string()),
                    write_group_id: None,
                    message: err.to_string(),
                    source: WriteSource::User,
                };
                self.push_event(ev.clone());
                self.handle_connection_error(&err);
                Err(ev)
            }
        }
    }

    /// Confirmed user write of a manufacturer write group (`WriteRegisterGroup`).
    pub fn write_register_group(
        &mut self,
        write_group_id: &str,
        values: &[(String, f64)],
        confirmed: bool,
    ) -> Result<(), Event> {
        if !confirmed {
            let ev = Event::WriteRejected {
                reason: "Schreibvorgang nicht bestätigt".into(),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }
        if !self.is_write_allowed() {
            let ev = Event::WriteRejected {
                reason: "Nicht verbunden".into(),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }

        let group_params = self.catalog.by_write_group(write_group_id);
        if group_params.is_empty() {
            let ev = Event::WriteRejected {
                reason: format!("Unbekannte Schreibgruppe: {write_group_id}"),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }

        // Encode in address order as one contiguous FC 0x10 frame.
        let mut ordered: Vec<(Parameter, f64)> = Vec::with_capacity(group_params.len());
        for param in &group_params {
            let display = values
                .iter()
                .find(|(id, _)| id == &param.param_id)
                .map(|(_, v)| *v);
            let Some(display) = display else {
                let ev = Event::WriteRejected {
                    reason: format!("Fehlender Wert für Gruppenparameter {}", param.param_id),
                };
                self.push_event(ev.clone());
                return Err(ev);
            };
            ordered.push(((*param).clone(), display));
        }
        ordered.sort_by_key(|(p, _)| p.address);

        let (start, words) = match encode_group_frame(&ordered) {
            Ok(frame) => frame,
            Err(reason) => {
                let ev = Event::WriteRejected { reason };
                self.push_event(ev.clone());
                return Err(ev);
            }
        };

        match self
            .transport
            .write_multiple(self.config.unit_id, start, &words)
        {
            Ok(()) => {
                self.after_user_group_write(write_group_id, &ordered);
                Ok(())
            }
            Err(err) => {
                let ev = Event::WriteFailed {
                    param_id: None,
                    write_group_id: Some(write_group_id.to_string()),
                    message: err.to_string(),
                    source: WriteSource::User,
                };
                self.push_event(ev.clone());
                self.handle_connection_error(&err);
                Err(ev)
            }
        }
    }

    /// Explicitly activate a retained (or any) param for cyclic rewrite after a prior write.
    ///
    /// Used when `persistence_class: retained` must still be held active (architecture §10.2).
    pub fn activate_retained(&mut self, param_id: &str, display_value: f64) -> Result<(), Event> {
        let param = match self.catalog.get(param_id) {
            Some(p) => p,
            None => {
                let ev = Event::WriteRejected {
                    reason: format!("Parameter nicht im Katalog: {param_id}"),
                };
                self.push_event(ev.clone());
                return Err(ev);
            }
        };
        if param.access != Access::ReadWrite {
            let ev = Event::WriteRejected {
                reason: format!("Parameter nicht schreibbar: {param_id}"),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }
        // Validate encode even though we do not write here.
        if let Err(err) = encode_display(param, display_value) {
            let ev = Event::WriteRejected {
                reason: err.to_string(),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }
        self.store.upsert(ActiveSetpoint {
            param_id: param_id.to_string(),
            write_group_id: None,
            display_value,
            group_values: None,
            activated_at: wall_now_iso8601(),
            last_write_at: None,
            last_error: None,
        });
        let _ = self.store.save_stub();
        self.push_event(Event::ActiveSetpointsChanged);
        if self.is_write_allowed() && self.next_write_at.is_none() {
            self.next_write_at = Some(self.clock.now() + self.config.write_interval);
        }
        Ok(())
    }

    /// Remove one Active entry (`ClearActiveSetpoint`). No Modbus write.
    pub fn clear_active_setpoint(&mut self, key: &str) -> bool {
        let removed = self.store.remove(key);
        if removed {
            let _ = self.store.save_stub();
            self.push_event(Event::ActiveSetpointsChanged);
            if self.store.is_empty() {
                // Keep timer armed while connected so a later activation still cycles;
                // empty passes are no-ops.
            }
        }
        removed
    }

    /// Clear all Active entries (`ClearAllActiveSetpoints`). Requires `confirmed`.
    pub fn clear_all_active_setpoints(&mut self, confirmed: bool) -> Result<(), Event> {
        if !confirmed {
            let ev = Event::WriteRejected {
                reason: "Löschen aller aktiven Sollwerte nicht bestätigt".into(),
            };
            self.push_event(ev.clone());
            return Err(ev);
        }
        if !self.store.is_empty() {
            self.store.clear();
            let _ = self.store.save_stub();
            self.push_event(Event::ActiveSetpointsChanged);
        }
        Ok(())
    }

    fn after_user_single_write(&mut self, param: &Parameter, display_value: f64) {
        let now = wall_now_iso8601();
        // Re-read affected register for UI freshness.
        self.reread_param(param);

        self.push_event(Event::WriteSucceeded {
            param_id: Some(param.param_id.clone()),
            write_group_id: None,
            source: WriteSource::User,
        });

        if param.persistence_class == PersistenceClass::Cyclic {
            self.store.upsert(ActiveSetpoint {
                param_id: param.param_id.clone(),
                write_group_id: None,
                display_value,
                group_values: None,
                activated_at: now.clone(),
                last_write_at: Some(now),
                last_error: None,
            });
            let _ = self.store.save_stub();
            self.push_event(Event::ActiveSetpointsChanged);
            // Arm / keep cyclic timer while connected.
            if self.is_write_allowed() && self.next_write_at.is_none() {
                self.next_write_at = Some(self.clock.now() + self.config.write_interval);
            }
        }
    }

    fn after_user_group_write(&mut self, write_group_id: &str, ordered: &[(Parameter, f64)]) {
        let now = wall_now_iso8601();
        for (param, _) in ordered {
            self.reread_param(param);
        }

        self.push_event(Event::WriteSucceeded {
            param_id: None,
            write_group_id: Some(write_group_id.to_string()),
            source: WriteSource::User,
        });

        // Groups are cyclic in v1 (buffer request temps < 50).
        let cyclic = ordered
            .iter()
            .any(|(p, _)| p.persistence_class == PersistenceClass::Cyclic);
        if cyclic {
            let anchor = ordered
                .first()
                .map(|(p, _)| p.param_id.clone())
                .unwrap_or_else(|| write_group_id.to_string());
            let group_values = ordered
                .iter()
                .map(|(p, v)| GroupValue {
                    param_id: p.param_id.clone(),
                    display_value: *v,
                })
                .collect::<Vec<_>>();
            let display_value = group_values.first().map(|g| g.display_value).unwrap_or(0.0);
            self.store.upsert(ActiveSetpoint {
                param_id: anchor,
                write_group_id: Some(write_group_id.to_string()),
                display_value,
                group_values: Some(group_values),
                activated_at: now.clone(),
                last_write_at: Some(now),
                last_error: None,
            });
            let _ = self.store.save_stub();
            self.push_event(Event::ActiveSetpointsChanged);
            if self.is_write_allowed() && self.next_write_at.is_none() {
                self.next_write_at = Some(self.clock.now() + self.config.write_interval);
            }
        }
    }

    fn reread_param(&mut self, param: &Parameter) {
        match self.transport.read_holding(
            self.config.unit_id,
            param.address,
            u16::from(param.register_count),
        ) {
            Ok(words) => {
                if let (Ok(raw), Ok(display)) = (
                    decode_raw(param.data_type, &words),
                    decode_display(param, &words),
                ) {
                    self.push_event(Event::ParameterUpdated {
                        param_id: param.param_id.clone(),
                        raw,
                        display,
                        quality: ValueQuality::Good,
                    });
                }
            }
            Err(err) => {
                self.push_event(Event::ParameterError {
                    param_id: param.param_id.clone(),
                    message: err.to_string(),
                });
                self.handle_connection_error(&err);
            }
        }
    }

    fn run_cyclic_pass(&mut self) {
        let count = self.store.len() as u32;
        self.push_event(Event::CyclicWriteStarted { count });

        if count == 0 {
            self.push_event(Event::CyclicWriteCompleted {
                ok_count: 0,
                err_count: 0,
            });
            if self.is_write_allowed() {
                self.next_write_at = Some(self.clock.now() + self.config.write_interval);
            } else {
                self.next_write_at = None;
            }
            return;
        }

        // Snapshot keys/values to avoid borrow issues while mutating store + transport.
        let snapshot = self.store.entries().to_vec();
        let mut ok_count = 0u32;
        let mut err_count = 0u32;
        let mut connection_lost = false;

        for entry in snapshot {
            if connection_lost {
                err_count = err_count.saturating_add(1);
                continue;
            }
            match self.rewrite_entry(&entry) {
                Ok(()) => {
                    ok_count = ok_count.saturating_add(1);
                    let key = entry.key().to_string();
                    if let Some(live) = self.store.get_mut(&key) {
                        live.last_write_at = Some(wall_now_iso8601());
                        live.last_error = None;
                    }
                    self.push_event(Event::WriteSucceeded {
                        param_id: if entry.write_group_id.is_some() {
                            None
                        } else {
                            Some(entry.param_id.clone())
                        },
                        write_group_id: entry.write_group_id.clone(),
                        source: WriteSource::Cyclic,
                    });
                }
                Err(err) => {
                    err_count = err_count.saturating_add(1);
                    let key = entry.key().to_string();
                    if let Some(live) = self.store.get_mut(&key) {
                        live.last_error = Some(err.clone());
                    }
                    self.push_event(Event::WriteFailed {
                        param_id: if entry.write_group_id.is_some() {
                            None
                        } else {
                            Some(entry.param_id.clone())
                        },
                        write_group_id: entry.write_group_id.clone(),
                        message: err,
                        source: WriteSource::Cyclic,
                    });
                    // Active entry intentionally kept (F-43).
                    if !self.transport.is_connected() {
                        connection_lost = true;
                    }
                }
            }
        }

        if ok_count > 0 {
            self.last_success_at = Some(self.clock.now());
            let _ = self.store.save_stub();
            self.push_event(Event::ActiveSetpointsChanged);
        } else if err_count > 0 {
            let _ = self.store.save_stub();
            self.push_event(Event::ActiveSetpointsChanged);
        }

        self.push_event(Event::CyclicWriteCompleted {
            ok_count,
            err_count,
        });

        if connection_lost {
            self.next_write_at = None;
            self.set_status(ConnectionStatus::Error {
                message: "Verbindung während zyklischem Schreiben verloren".into(),
            });
        } else if self.is_write_allowed() {
            self.next_write_at = Some(self.clock.now() + self.config.write_interval);
        } else {
            self.next_write_at = None;
        }
    }

    fn rewrite_entry(&mut self, entry: &ActiveSetpoint) -> Result<(), String> {
        if let Some(group_id) = entry.write_group_id.as_deref() {
            let values = entry
                .group_values
                .as_ref()
                .ok_or_else(|| "Aktiver Gruppeneintrag ohne Werte".to_string())?;
            let pairs: Vec<(String, f64)> = values
                .iter()
                .map(|g| (g.param_id.clone(), g.display_value))
                .collect();
            self.write_group_raw(group_id, &pairs)
        } else {
            let param = self
                .catalog
                .get(&entry.param_id)
                .ok_or_else(|| format!("Parameter nicht im Katalog: {}", entry.param_id))?;
            let words = encode_display(param, entry.display_value).map_err(|e| e.to_string())?;
            self.transport
                .write_multiple(self.config.unit_id, param.address, &words)
                .map_err(|e| e.to_string())
        }
    }

    /// Internal group write without confirmation / Active updates (cyclic path).
    fn write_group_raw(
        &mut self,
        write_group_id: &str,
        values: &[(String, f64)],
    ) -> Result<(), String> {
        let group_params = self.catalog.by_write_group(write_group_id);
        if group_params.is_empty() {
            return Err(format!("Unbekannte Schreibgruppe: {write_group_id}"));
        }
        let mut ordered: Vec<(Parameter, f64)> = Vec::with_capacity(group_params.len());
        for param in &group_params {
            let display = values
                .iter()
                .find(|(id, _)| id == &param.param_id)
                .map(|(_, v)| *v)
                .ok_or_else(|| format!("Fehlender Wert für {}", param.param_id))?;
            ordered.push(((*param).clone(), display));
        }
        ordered.sort_by_key(|(p, _)| p.address);
        let (start, words) = encode_group_frame(&ordered)?;
        self.transport
            .write_multiple(self.config.unit_id, start, &words)
            .map_err(|e| e.to_string())
    }

    fn handle_connection_error(&mut self, err: &ModbusError) {
        if matches!(
            err,
            ModbusError::ConnectionClosed | ModbusError::NotConnected
        ) {
            self.next_write_at = None;
            self.transport.disconnect();
            self.set_status(ConnectionStatus::Error {
                message: err.to_string(),
            });
        }
    }
}

fn encode_group_frame(ordered: &[(Parameter, f64)]) -> Result<(u16, Vec<u16>), String> {
    if ordered.is_empty() {
        return Err("Leere Schreibgruppe".into());
    }
    let start = ordered[0].0.address;
    let mut expected = start;
    let mut words = Vec::new();
    for (param, display) in ordered {
        if param.address != expected {
            return Err(format!(
                "Schreibgruppe nicht zusammenhängend bei {} (erwartet {expected})",
                param.param_id
            ));
        }
        let encoded = encode_display(param, *display).map_err(|e| e.to_string())?;
        expected = expected.saturating_add(encoded.len() as u16);
        words.extend(encoded);
    }
    Ok((start, words))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FakeClock;
    use lambda_mock::{InjectedFault, MockSlave};

    fn test_scheduler(
        interval: Duration,
    ) -> (CyclicWriteScheduler<MockSlave, FakeClock>, FakeClock) {
        let clock = FakeClock::new();
        let mock = MockSlave::from_official_catalog();
        let config = CyclicWriteConfig {
            unit_id: 1,
            write_interval: interval,
            rewrite_on_connect: true,
        };
        let scheduler =
            CyclicWriteScheduler::with_clock(mock, config, clock.clone(), Catalog::official());
        (scheduler, clock)
    }

    #[test]
    fn default_interval_is_sixty_seconds() {
        assert_eq!(DEFAULT_CYCLIC_WRITE_INTERVAL, Duration::from_secs(60));
        assert_eq!(
            CyclicWriteConfig::default().write_interval,
            Duration::from_secs(60)
        );
        assert_eq!(
            CyclicWriteConfig::default().write_interval.as_secs(),
            u64::from(crate::defaults::CYCLIC_WRITE_INTERVAL_SECS)
        );
    }

    #[test]
    fn first_write_requires_confirmation() {
        let (mut sched, _clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();

        let err = sched
            .write_register("hk.1.flow_setpoint", 42.5, false)
            .unwrap_err();
        assert!(matches!(err, Event::WriteRejected { .. }));
        assert_eq!(sched.transport().write_count, 0);
        assert!(sched.store().is_empty());
    }

    #[test]
    fn cyclic_user_write_rewrites_on_timer_without_reconfirm() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();

        sched
            .write_register("hk.1.flow_setpoint", 42.5, true)
            .unwrap();
        let events = sched.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::WriteSucceeded {
                source: WriteSource::User,
                ..
            }
        )));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ActiveSetpointsChanged)));
        assert!(sched.store().get("hk.1.flow_setpoint").is_some());
        let writes_after_user = sched.transport().write_count;
        assert!(writes_after_user >= 1);

        // Not due yet.
        assert!(!sched.tick());
        assert_eq!(sched.transport().write_count, writes_after_user);

        // First cyclic tick — no confirm flag involved.
        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());
        // Second cyclic tick.
        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());

        assert!(sched.transport().write_count >= writes_after_user + 2);
        assert_eq!(
            sched.transport().get(5005),
            Some(425),
            "raw value must stay 42.5 °C → 425"
        );

        let cyclic_events = sched.drain_events();
        let cyclic_ok = cyclic_events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::WriteSucceeded {
                        source: WriteSource::Cyclic,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(cyclic_ok, 2);
        assert!(sched.store().get("hk.1.flow_setpoint").is_some());
    }

    #[test]
    fn retained_write_without_explicit_activate_skips_cyclic() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();

        sched
            .write_register("hk.1.setpoint_offset", 1.5, true)
            .unwrap();
        assert!(
            sched.store().is_empty(),
            "retained must not enter Active by default"
        );
        let writes = sched.transport().write_count;

        clock.advance(Duration::from_millis(200));
        assert!(
            !sched.tick() || sched.transport().write_count == writes,
            "no periodic rewrite without Active entry"
        );
        // Timer may be unarmed when store empty after connect-with-empty.
        // Force: even if next_write_at were set, empty pass writes nothing new for this param.
        assert_eq!(sched.transport().get(5050), Some(15)); // 1.5 / 0.1
    }

    #[test]
    fn clear_stops_further_rewrites() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();

        sched
            .write_register("hk.1.flow_setpoint", 40.0, true)
            .unwrap();
        let _ = sched.drain_events();
        assert!(sched.clear_active_setpoint("hk.1.flow_setpoint"));
        assert!(sched.store().is_empty());
        let writes = sched.transport().write_count;

        clock.advance(Duration::from_millis(50));
        let _ = sched.tick();
        clock.advance(Duration::from_millis(50));
        let _ = sched.tick();
        assert_eq!(
            sched.transport().write_count,
            writes,
            "cleared Active → no further FC 0x10"
        );
    }

    #[test]
    fn rewrite_error_keeps_active_entry() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();

        sched
            .write_register("hk.1.flow_setpoint", 41.0, true)
            .unwrap();
        let _ = sched.drain_events();

        sched
            .transport_mut()
            .inject_fault_once(InjectedFault::Exception(0x03));

        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());
        let events = sched.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::WriteFailed {
                source: WriteSource::Cyclic,
                ..
            }
        )));
        let entry = sched
            .store()
            .get("hk.1.flow_setpoint")
            .expect("Active entry must remain");
        assert!(entry.last_error.is_some());

        // Next cycle retries after clearing the one-shot fault.
        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());
        let events = sched.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::WriteSucceeded {
                source: WriteSource::Cyclic,
                ..
            }
        )));
        assert!(sched
            .store()
            .get("hk.1.flow_setpoint")
            .unwrap()
            .last_error
            .is_none());
    }

    #[test]
    fn disconnect_pauses_cyclic_reconnect_rewrites_when_active() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        sched
            .write_register("hk.1.flow_setpoint", 38.0, true)
            .unwrap();
        let _ = sched.drain_events();
        let writes = sched.transport().write_count;

        sched.disconnect();
        assert!(sched.next_write_at().is_none());
        clock.advance(Duration::from_millis(500));
        assert!(!sched.tick());
        assert_eq!(sched.transport().write_count, writes);

        sched.connect("mock", 502).unwrap();
        // Immediate rewrite on connect when Active ≠ empty.
        assert!(sched.transport().write_count > writes);
        assert!(sched
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::CyclicWriteCompleted { .. })));
    }

    #[test]
    fn write_group_becomes_single_active_entry() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();

        let values = vec![
            ("buffer.0.request_flow_temp".into(), 35.0),
            ("buffer.0.request_return_temp".into(), 30.0),
            ("buffer.0.request_spread".into(), 5.0),
        ];
        sched
            .write_register_group("buffer.request_temps", &values, true)
            .unwrap();
        assert_eq!(sched.store().len(), 1);
        assert!(sched.store().get("buffer.request_temps").is_some());
        // One FC 0x10 for the group (+ re-reads do not count as writes).
        assert_eq!(sched.transport().write_count, 1);
        assert_eq!(sched.transport().get(3006), Some(350));
        assert_eq!(sched.transport().get(3007), Some(300));
        assert_eq!(sched.transport().get(3008), Some(50));

        let writes = sched.transport().write_count;
        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());
        assert_eq!(sched.transport().write_count, writes + 1);
    }

    #[test]
    fn clear_all_requires_confirmation() {
        let (mut sched, _clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        sched
            .write_register("hk.1.flow_setpoint", 40.0, true)
            .unwrap();
        let _ = sched.drain_events();
        assert!(matches!(
            sched.clear_all_active_setpoints(false),
            Err(Event::WriteRejected { .. })
        ));
        assert_eq!(sched.store().len(), 1);
        sched.clear_all_active_setpoints(true).unwrap();
        assert!(sched.store().is_empty());
    }

    #[test]
    fn explicit_retained_activation_enables_rewrite() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();
        sched
            .write_register("hk.1.setpoint_offset", 2.0, true)
            .unwrap();
        assert!(sched.store().is_empty());
        sched
            .activate_retained("hk.1.setpoint_offset", 2.0)
            .unwrap();
        assert!(sched.store().get("hk.1.setpoint_offset").is_some());

        // Connect armed next_write_at even with empty store; after activation, tick rewrites.
        let writes_before = sched.transport().write_count;
        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());
        assert!(sched.transport().write_count > writes_before);
        assert_eq!(sched.transport().get(5050), Some(20)); // 2.0 °C → raw 20
    }
}
