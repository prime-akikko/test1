//! Poll-Scheduler: read all catalog parameters every N seconds (default **5 s**).
//!
//! - After [`PollScheduler::connect`]: immediate poll, then interval.
//! - Manual [`PollScheduler::read_all`] / [`PollScheduler::read_group`] independent of timer.
//! - Paused while disconnected / error — no Modbus reads until reconnect.
//! - Partial block failures mark affected params `Error`; successful peers stay `Good`.
//! - Drive with [`PollScheduler::tick`] and an injectable [`Clock`] (use [`FakeClock`] in tests).

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use lambda_catalog::{decode_display, decode_raw, plan_read_blocks, Catalog, Parameter, ReadBlock};
use lambda_modbus::{ModbusError, ModbusTransport, MAX_READ_QUANTITY};

use crate::clock::{Clock, SystemClock};
use crate::value::{ParameterValue, ValueQuality};
use crate::{ConnectionStatus, Event, PollTrigger};

/// Production default poll interval (locked v1 / `config.poll_interval_secs`).
pub const DEFAULT_POLL_INTERVAL: Duration =
    Duration::from_secs(crate::defaults::POLL_INTERVAL_SECS as u64);

/// Configuration for the poll scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollConfig {
    pub unit_id: u8,
    /// Interval between automatic polls while connected. Production default: 5 s.
    pub poll_interval: Duration,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            unit_id: crate::defaults::UNIT_ID,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

/// Orchestrates catalog reads over a [`ModbusTransport`].
#[derive(Debug)]
pub struct PollScheduler<T: ModbusTransport, C: Clock = SystemClock> {
    transport: T,
    clock: C,
    catalog: &'static Catalog,
    config: PollConfig,
    status: ConnectionStatus,
    values: HashMap<String, ParameterValue>,
    /// Instant of last poll that decoded at least one parameter successfully.
    last_success_at: Option<Instant>,
    /// When the next timer-driven poll is due (`None` = paused).
    next_poll_at: Option<Instant>,
    events: VecDeque<Event>,
}

impl<T: ModbusTransport> PollScheduler<T, SystemClock> {
    /// Create a scheduler with the system clock and official catalog.
    pub fn new(transport: T, config: PollConfig) -> Self {
        Self::with_clock(transport, config, SystemClock, Catalog::official())
    }
}

impl<T: ModbusTransport, C: Clock> PollScheduler<T, C> {
    /// Create a scheduler with an injectable clock (tests) and catalog.
    pub fn with_clock(
        transport: T,
        config: PollConfig,
        clock: C,
        catalog: &'static Catalog,
    ) -> Self {
        Self {
            transport,
            clock,
            catalog,
            config,
            status: ConnectionStatus::Disconnected,
            values: HashMap::new(),
            last_success_at: None,
            next_poll_at: None,
            events: VecDeque::new(),
        }
    }

    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }

    pub fn config(&self) -> &PollConfig {
        &self.config
    }

    pub fn poll_interval(&self) -> Duration {
        self.config.poll_interval
    }

    /// Last successful poll time (monotonic), if any.
    pub fn last_success_at(&self) -> Option<Instant> {
        self.last_success_at
    }

    pub fn values(&self) -> &HashMap<String, ParameterValue> {
        &self.values
    }

    pub fn get_value(&self, param_id: &str) -> Option<&ParameterValue> {
        self.values.get(param_id)
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// When the next timer poll would fire (`None` while paused / disconnected).
    pub fn next_poll_at(&self) -> Option<Instant> {
        self.next_poll_at
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

    /// Connect the transport, emit `Connected`, and run an immediate poll.
    pub fn connect(&mut self, host: &str, port: u16) -> Result<(), ModbusError> {
        self.set_status(ConnectionStatus::Connecting);
        match self.transport.connect(host, port) {
            Ok(()) => {
                self.set_status(ConnectionStatus::Connected);
                // Immediate poll after connect, then arm interval.
                self.run_poll(PollTrigger::Timer, None);
                Ok(())
            }
            Err(err) => {
                self.next_poll_at = None;
                self.set_status(ConnectionStatus::Error {
                    message: err.to_string(),
                });
                Err(err)
            }
        }
    }

    /// Disconnect, pause the timer, and mark cached values stale.
    pub fn disconnect(&mut self) {
        self.transport.disconnect();
        self.next_poll_at = None;
        self.mark_all_stale();
        self.set_status(ConnectionStatus::Disconnected);
    }

    fn mark_all_stale(&mut self) {
        for value in self.values.values_mut() {
            if value.quality == ValueQuality::Good {
                value.quality = ValueQuality::Stale;
            }
        }
    }

    fn is_poll_allowed(&self) -> bool {
        matches!(self.status, ConnectionStatus::Connected) && self.transport.is_connected()
    }

    /// Timer step: if due and connected, run one poll cycle.
    ///
    /// Returns `true` when a poll ran.
    pub fn tick(&mut self) -> bool {
        if !self.is_poll_allowed() {
            return false;
        }
        let Some(due) = self.next_poll_at else {
            return false;
        };
        if self.clock.now() < due {
            return false;
        }
        self.run_poll(PollTrigger::Timer, None);
        true
    }

    /// Manual refresh of all catalog parameters (`ReadAll`). No-op when disconnected.
    pub fn read_all(&mut self) -> bool {
        if !self.is_poll_allowed() {
            return false;
        }
        self.run_poll(PollTrigger::Manual, None);
        true
    }

    /// Read a single UI group / catalog `group` label (`ReadGroup`).
    pub fn read_group(&mut self, group_id: &str) -> bool {
        if !self.is_poll_allowed() {
            return false;
        }
        self.run_poll(PollTrigger::Manual, Some(group_id));
        true
    }

    fn plan_blocks(&self, group_filter: Option<&str>) -> Vec<ReadBlock> {
        match group_filter {
            Some(group) => {
                let params = self.catalog.by_group(group);
                plan_read_blocks(&params, MAX_READ_QUANTITY)
            }
            None => self.catalog.read_blocks(),
        }
    }

    fn run_poll(&mut self, trigger: PollTrigger, group_filter: Option<&str>) {
        self.push_event(Event::PollStarted { trigger });

        let blocks = self.plan_blocks(group_filter);
        let mut ok_count = 0u32;
        let mut err_count = 0u32;

        // Collect results first so we do not hold conflicting borrows on `self`.
        let mut updates: Vec<PollUpdate> = Vec::new();

        for block in &blocks {
            match self
                .transport
                .read_holding(self.config.unit_id, block.start, block.count)
            {
                Ok(words) => match decode_block(self.catalog, block, &words) {
                    Ok(decoded) => {
                        for item in decoded {
                            ok_count = ok_count.saturating_add(1);
                            updates.push(PollUpdate::Ok(item));
                        }
                    }
                    Err(message) => {
                        for param_id in &block.param_ids {
                            err_count = err_count.saturating_add(1);
                            updates.push(PollUpdate::Err {
                                param_id: param_id.clone(),
                                message: message.clone(),
                            });
                        }
                    }
                },
                Err(err) => {
                    // Connection drop: pause further polling this cycle.
                    if matches!(
                        err,
                        ModbusError::ConnectionClosed | ModbusError::NotConnected
                    ) {
                        for param_id in &block.param_ids {
                            err_count = err_count.saturating_add(1);
                            updates.push(PollUpdate::Err {
                                param_id: param_id.clone(),
                                message: err.to_string(),
                            });
                        }
                        // Apply what we have, then mark offline.
                        self.apply_updates(updates);
                        self.push_event(Event::PollCompleted {
                            ok_count,
                            err_count,
                        });
                        self.next_poll_at = None;
                        self.transport.disconnect();
                        self.mark_all_stale();
                        self.set_status(ConnectionStatus::Error {
                            message: err.to_string(),
                        });
                        return;
                    }
                    for param_id in &block.param_ids {
                        err_count = err_count.saturating_add(1);
                        updates.push(PollUpdate::Err {
                            param_id: param_id.clone(),
                            message: err.to_string(),
                        });
                    }
                }
            }
        }

        self.apply_updates(updates);

        if ok_count > 0 {
            self.last_success_at = Some(self.clock.now());
        }

        self.push_event(Event::PollCompleted {
            ok_count,
            err_count,
        });

        // Re-arm interval after every successful scheduling point while connected.
        if self.is_poll_allowed() {
            self.next_poll_at = Some(self.clock.now() + self.config.poll_interval);
        } else {
            self.next_poll_at = None;
        }
    }

    fn apply_updates(&mut self, updates: Vec<PollUpdate>) {
        for update in updates {
            match update {
                PollUpdate::Ok(decoded) => {
                    let param_id = decoded.param_id.clone();
                    self.push_event(Event::ParameterUpdated {
                        param_id: param_id.clone(),
                        raw: decoded.raw,
                        display: decoded.display,
                        quality: ValueQuality::Good,
                    });
                    self.values.insert(
                        param_id,
                        ParameterValue {
                            param_id: decoded.param_id,
                            raw: decoded.raw,
                            display: decoded.display,
                            quality: ValueQuality::Good,
                        },
                    );
                }
                PollUpdate::Err { param_id, message } => {
                    self.push_event(Event::ParameterError {
                        param_id: param_id.clone(),
                        message: message.clone(),
                    });
                    if let Some(existing) = self.values.get_mut(&param_id) {
                        existing.quality = ValueQuality::Error;
                    } else {
                        self.values.insert(
                            param_id.clone(),
                            ParameterValue {
                                param_id,
                                raw: 0,
                                display: 0.0,
                                quality: ValueQuality::Error,
                            },
                        );
                    }
                }
            }
        }
    }
}

enum PollUpdate {
    Ok(DecodedParam),
    Err { param_id: String, message: String },
}

struct DecodedParam {
    param_id: String,
    raw: i64,
    display: f64,
}

fn decode_block(
    catalog: &Catalog,
    block: &ReadBlock,
    words: &[u16],
) -> Result<Vec<DecodedParam>, String> {
    if words.len() < block.count as usize {
        return Err(format!(
            "Unerwartete Registerlänge: erwartet {}, erhalten {}",
            block.count,
            words.len()
        ));
    }

    let mut out = Vec::with_capacity(block.param_ids.len());
    for param_id in &block.param_ids {
        let param = catalog
            .get(param_id)
            .ok_or_else(|| format!("Parameter nicht im Katalog: {param_id}"))?;
        let offset = usize::from(param.address.saturating_sub(block.start));
        let end = offset + param.register_count as usize;
        if end > words.len() {
            return Err(format!(
                "Block zu kurz für Parameter {param_id} (offset {offset})"
            ));
        }
        let slice = &words[offset..end];
        let decoded = decode_param(param, slice)?;
        out.push(decoded);
    }
    Ok(out)
}

fn decode_param(param: &Parameter, words: &[u16]) -> Result<DecodedParam, String> {
    let raw = decode_raw(param.data_type, words).map_err(|e| e.to_string())?;
    let display = decode_display(param, words).map_err(|e| e.to_string())?;
    Ok(DecodedParam {
        param_id: param.param_id.clone(),
        raw,
        display,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FakeClock;
    use lambda_mock::{InjectedFault, MockSlave};

    fn test_scheduler(interval: Duration) -> (PollScheduler<MockSlave, FakeClock>, FakeClock) {
        let clock = FakeClock::new();
        let mock = MockSlave::from_official_catalog();
        let config = PollConfig {
            unit_id: 1,
            poll_interval: interval,
        };
        let scheduler = PollScheduler::with_clock(mock, config, clock.clone(), Catalog::official());
        (scheduler, clock)
    }

    #[test]
    fn default_interval_is_five_seconds() {
        assert_eq!(DEFAULT_POLL_INTERVAL, Duration::from_secs(5));
        assert_eq!(PollConfig::default().poll_interval, Duration::from_secs(5));
        assert_eq!(
            PollConfig::default().poll_interval.as_secs(),
            u64::from(crate::defaults::POLL_INTERVAL_SECS)
        );
    }

    #[test]
    fn connect_runs_immediate_poll_then_two_timer_cycles() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        // Seed a distinctive value before connect.
        sched.transport_mut().seed_i16(2, 123); // general.0.outdoor_temp scale 0.1

        sched.connect("mock", 502).unwrap();
        let events = sched.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::ConnectionChanged {
                status: ConnectionStatus::Connected
            }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            Event::PollStarted {
                trigger: PollTrigger::Timer
            }
        )));
        let completed: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::PollCompleted { .. }))
            .collect();
        assert_eq!(completed.len(), 1);
        assert!(sched.last_success_at().is_some());
        assert_eq!(
            sched.get_value("general.0.outdoor_temp").map(|v| v.raw),
            Some(123)
        );
        assert_eq!(
            sched.values().len(),
            Catalog::official().len(),
            "first poll should cover every catalog parameter"
        );
        let reads_after_connect = sched.transport().read_count;

        // Not due yet.
        assert!(!sched.tick());
        assert_eq!(sched.transport().read_count, reads_after_connect);

        // First timer tick.
        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());
        // Second timer tick.
        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());

        let poll_completed = sched
            .drain_events()
            .into_iter()
            .filter(|e| matches!(e, Event::PollCompleted { .. }))
            .count();
        assert_eq!(poll_completed, 2);
        assert!(sched.transport().read_count > reads_after_connect);
        // Still full coverage after ≥2 cycles.
        assert_eq!(sched.values().len(), Catalog::official().len());
        assert!(sched
            .values()
            .values()
            .all(|v| v.quality == ValueQuality::Good));
    }

    #[test]
    fn disconnect_pauses_poll_reconnect_polls_immediately() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();
        let reads = sched.transport().read_count;

        sched.disconnect();
        assert_eq!(sched.status(), &ConnectionStatus::Disconnected);
        assert!(sched.next_poll_at().is_none());
        assert!(sched
            .values()
            .values()
            .all(|v| v.quality == ValueQuality::Stale));

        clock.advance(Duration::from_millis(500));
        assert!(!sched.tick(), "no poll while disconnected");
        assert!(
            !sched.read_all(),
            "manual refresh paused while disconnected"
        );
        assert_eq!(sched.transport().read_count, reads);

        sched.connect("mock", 502).unwrap();
        assert!(sched
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::PollCompleted { .. })));
        assert!(sched.transport().read_count > reads);
        assert!(sched.next_poll_at().is_some());
    }

    #[test]
    fn manual_read_all_independent_of_timer() {
        let (mut sched, clock) = test_scheduler(Duration::from_secs(5));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();
        let reads = sched.transport().read_count;

        // Manual refresh before interval elapses.
        assert!(sched.read_all());
        assert!(sched.transport().read_count > reads);
        let events = sched.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::PollStarted {
                trigger: PollTrigger::Manual
            }
        )));

        // Manual re-armed from "now"; advancing ≪ 5s must not tick.
        clock.advance(Duration::from_millis(50));
        assert!(!sched.tick());
    }

    #[test]
    fn partial_block_failure_marks_error_keeps_others_good() {
        let (mut sched, _clock) = test_scheduler(Duration::from_millis(50));
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();
        assert!(sched
            .get_value("general.0.outdoor_temp")
            .is_some_and(|v| v.quality == ValueQuality::Good));
        assert!(sched
            .get_value("wp.0.flow_temp")
            .is_some_and(|v| v.quality == ValueQuality::Good));

        // Fail heat-pump address range (1000+) on overlapping blocks.
        sched
            .transport_mut()
            .inject_fault(InjectedFault::AddressRange {
                start: 1000,
                count: 30,
                code: 0x02,
            });

        assert!(sched.read_all());
        let events = sched.drain_events();

        assert!(events.iter().any(|e| matches!(
            e,
            Event::ParameterError { param_id, .. } if param_id.starts_with("wp.0.")
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            Event::ParameterUpdated {
                param_id,
                quality: ValueQuality::Good,
                ..
            } if param_id == "general.0.outdoor_temp"
        )));

        match events
            .iter()
            .find(|e| matches!(e, Event::PollCompleted { .. }))
        {
            Some(Event::PollCompleted {
                ok_count,
                err_count,
            }) => {
                assert!(*ok_count > 0, "other modules must succeed");
                assert!(*err_count > 0, "heatpump block must fail");
            }
            other => panic!("expected PollCompleted, got {other:?}"),
        }

        assert_eq!(
            sched.get_value("general.0.outdoor_temp").map(|v| v.quality),
            Some(ValueQuality::Good)
        );
        let hp_errors = sched
            .values()
            .iter()
            .filter(|(id, v)| id.starts_with("wp.0.") && v.quality == ValueQuality::Error)
            .count();
        assert!(hp_errors > 0);
    }

    #[test]
    fn last_success_updates_on_good_poll() {
        let (mut sched, clock) = test_scheduler(Duration::from_millis(50));
        assert!(sched.last_success_at().is_none());
        sched.connect("mock", 502).unwrap();
        let first = sched.last_success_at().expect("success after connect poll");
        clock.advance(Duration::from_millis(50));
        assert!(sched.tick());
        let second = sched.last_success_at().expect("success after tick");
        assert!(second > first);
    }

    #[test]
    fn read_group_only_updates_that_group() {
        let (mut sched, _clock) = test_scheduler(Duration::from_millis(50));
        sched.transport_mut().seed_i16(5005, 425);
        sched.transport_mut().seed_i16(2, 50);
        sched.connect("mock", 502).unwrap();
        let _ = sched.drain_events();

        sched.transport_mut().seed_i16(5005, 500);
        assert!(sched.read_group("Heizkreis 1"));
        let events = sched.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::ParameterUpdated { param_id, raw: 500, .. }
            if param_id == "hk.1.flow_setpoint"
        )));
        // Outdoor temp not part of this group poll — value unchanged from connect.
        assert_eq!(
            sched.get_value("general.0.outdoor_temp").map(|v| v.raw),
            Some(50)
        );
        assert_eq!(
            sched.get_value("hk.1.flow_setpoint").map(|v| v.raw),
            Some(500)
        );
    }
}
