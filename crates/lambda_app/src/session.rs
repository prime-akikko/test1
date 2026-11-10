//! Mock-first backend session: shared-queue poll + cyclic write schedulers.

use std::time::Duration;

use lambda_core::{
    defaults, ActiveSetpoint, ActiveSetpointStore, ConnectionStatus, CyclicWriteConfig,
    CyclicWriteScheduler, Event, PollConfig, PollScheduler, QueuedTransport,
    DEFAULT_CYCLIC_WRITE_INTERVAL, DEFAULT_POLL_INTERVAL,
};

use crate::state::ShellState;
use crate::strings;

#[cfg(feature = "mock")]
type MockQueue = QueuedTransport<lambda_mock::MockSlave>;
#[cfg(not(feature = "mock"))]
type LiveQueue = QueuedTransport<lambda_core::TcpModbusClient>;

/// Owns poll + cyclic schedulers on one serialized Modbus transport.
pub struct AppSession {
    inner: SessionInner,
}

enum SessionInner {
    #[cfg(feature = "mock")]
    Mock {
        poll: PollScheduler<MockQueue>,
        cyclic: CyclicWriteScheduler<MockQueue>,
    },
    #[cfg(not(feature = "mock"))]
    Live {
        poll: PollScheduler<LiveQueue>,
        cyclic: CyclicWriteScheduler<LiveQueue>,
    },
}

impl AppSession {
    /// Build a session. Default app features → in-memory mock.
    pub fn new(unit_id: u8) -> Self {
        let poll_cfg = PollConfig {
            unit_id,
            poll_interval: DEFAULT_POLL_INTERVAL,
        };
        let cyclic_cfg = CyclicWriteConfig {
            unit_id,
            write_interval: DEFAULT_CYCLIC_WRITE_INTERVAL,
            rewrite_on_connect: true,
        };
        #[cfg(feature = "mock")]
        {
            let slave = lambda_core::new_mock_transport(unit_id);
            let transport = QueuedTransport::new(slave);
            let poll = PollScheduler::new(transport.clone(), poll_cfg);
            let cyclic = CyclicWriteScheduler::new(transport, cyclic_cfg);
            Self {
                inner: SessionInner::Mock { poll, cyclic },
            }
        }
        #[cfg(not(feature = "mock"))]
        {
            let client = lambda_core::TcpModbusClient::new(lambda_core::ClientConfig {
                unit_id,
                ..Default::default()
            });
            let transport = QueuedTransport::new(client);
            let poll = PollScheduler::new(transport.clone(), poll_cfg);
            let cyclic = CyclicWriteScheduler::new(transport, cyclic_cfg);
            Self {
                inner: SessionInner::Live { poll, cyclic },
            }
        }
    }

    pub fn from_shell_defaults() -> Self {
        Self::new(defaults::UNIT_ID)
    }

    pub fn status(&self) -> &ConnectionStatus {
        match &self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, .. } => poll.status(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, .. } => poll.status(),
        }
    }

    pub fn poll_interval(&self) -> Duration {
        match &self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, .. } => poll.poll_interval(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, .. } => poll.poll_interval(),
        }
    }

    pub fn active_entries(&self) -> &[ActiveSetpoint] {
        match &self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { cyclic, .. } => cyclic.store().entries(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { cyclic, .. } => cyclic.store().entries(),
        }
    }

    /// Install a previously loaded Active store (M8 restore).
    pub fn install_active_store(&mut self, store: ActiveSetpointStore) {
        match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { cyclic, .. } => cyclic.replace_store(store),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { cyclic, .. } => cyclic.replace_store(store),
        }
    }

    /// Snapshot the Active store (e.g. before recreating the session for a new unit id).
    pub fn take_active_store(&mut self) -> ActiveSetpointStore {
        match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { cyclic, .. } => {
                let snapshot = cyclic.store().clone();
                cyclic.replace_store(ActiveSetpointStore::new());
                snapshot
            }
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { cyclic, .. } => {
                let snapshot = cyclic.store().clone();
                cyclic.replace_store(ActiveSetpointStore::new());
                snapshot
            }
        }
    }

    /// Connect using shell fields; drains resulting events into `state`.
    pub fn connect(&mut self, state: &mut ShellState) {
        let host = state.connection.host.trim().to_string();
        let port = match state.connection.parse_port() {
            Ok(p) => p,
            Err(msg) => {
                state.status_message = msg.to_string();
                return;
            }
        };
        let unit_id = match state.connection.parse_unit_id() {
            Ok(u) => u,
            Err(msg) => {
                state.status_message = msg.to_string();
                return;
            }
        };

        if unit_id != self.unit_id() {
            let active = self.take_active_store();
            *self = Self::new(unit_id);
            self.install_active_store(active);
        }

        let result = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, cyclic } => {
                let r = poll.connect(&host, port);
                match &r {
                    Ok(()) => cyclic.bind_connected(),
                    Err(err) => cyclic.bind_error(err.to_string()),
                }
                r
            }
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, cyclic } => {
                let r = poll.connect(&host, port);
                match &r {
                    Ok(()) => cyclic.bind_connected(),
                    Err(err) => cyclic.bind_error(err.to_string()),
                }
                r
            }
        };
        self.drain_into(state);
        if let Err(err) = result {
            state.status_message = format!("{}: {err}", strings::STATUS_ERROR_PREFIX);
        }
    }

    pub fn disconnect(&mut self, state: &mut ShellState) {
        match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, cyclic } => {
                poll.disconnect();
                cyclic.bind_disconnected();
            }
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, cyclic } => {
                poll.disconnect();
                cyclic.bind_disconnected();
            }
        }
        self.drain_into(state);
    }

    pub fn refresh(&mut self, state: &mut ShellState) {
        if !matches!(self.status(), ConnectionStatus::Connected) {
            state.status_message = strings::MSG_NOT_CONNECTED.to_string();
            return;
        }
        let ran = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, .. } => poll.read_all(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, .. } => poll.read_all(),
        };
        self.drain_into(state);
        if !ran {
            state.status_message = strings::MSG_NOT_CONNECTED.to_string();
        }
    }

    pub fn refresh_selected_group(&mut self, state: &mut ShellState) {
        if !matches!(self.status(), ConnectionStatus::Connected) {
            return;
        }
        let group = state.selected_group.clone();
        let _ = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, .. } => poll.read_group(&group),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, .. } => poll.read_group(&group),
        };
        self.drain_into(state);
    }

    /// Advance poll + cyclic timers if due.
    pub fn tick(&mut self, state: &mut ShellState) -> bool {
        let poll_ran = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, .. } => poll.tick(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, .. } => poll.tick(),
        };
        let cyclic_ran = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { cyclic, .. } => cyclic.tick(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { cyclic, .. } => cyclic.tick(),
        };
        self.drain_into(state);
        poll_ran || cyclic_ran
    }

    /// Confirmed single-register write (`confirmed` must be true).
    pub fn write_register(
        &mut self,
        state: &mut ShellState,
        param_id: &str,
        display_value: f64,
        confirmed: bool,
    ) {
        let _ = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { cyclic, .. } => {
                cyclic.write_register(param_id, display_value, confirmed)
            }
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { cyclic, .. } => {
                cyclic.write_register(param_id, display_value, confirmed)
            }
        };
        self.drain_into(state);
    }

    /// Confirmed write-group FC 0x10.
    pub fn write_register_group(
        &mut self,
        state: &mut ShellState,
        write_group_id: &str,
        values: &[(String, f64)],
        confirmed: bool,
    ) {
        let _ = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { cyclic, .. } => {
                cyclic.write_register_group(write_group_id, values, confirmed)
            }
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { cyclic, .. } => {
                cyclic.write_register_group(write_group_id, values, confirmed)
            }
        };
        self.drain_into(state);
    }

    pub fn clear_active_setpoint(&mut self, state: &mut ShellState, key: &str) {
        let _ = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { cyclic, .. } => cyclic.clear_active_setpoint(key),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { cyclic, .. } => cyclic.clear_active_setpoint(key),
        };
        self.drain_into(state);
    }

    pub fn clear_all_active_setpoints(&mut self, state: &mut ShellState, confirmed: bool) {
        let _ = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { cyclic, .. } => cyclic.clear_all_active_setpoints(confirmed),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { cyclic, .. } => cyclic.clear_all_active_setpoints(confirmed),
        };
        self.drain_into(state);
    }

    pub fn drain_into(&mut self, state: &mut ShellState) {
        let mut events: Vec<Event> = Vec::new();
        match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, cyclic } => {
                events.extend(poll.drain_events());
                events.extend(cyclic.drain_events());
            }
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, cyclic } => {
                events.extend(poll.drain_events());
                events.extend(cyclic.drain_events());
            }
        }
        state.apply_events(events);
        state.sync_active_from(self.active_entries());
    }

    fn unit_id(&self) -> u8 {
        match &self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock { poll, .. } => poll.config().unit_id,
            #[cfg(not(feature = "mock"))]
            SessionInner::Live { poll, .. } => poll.config().unit_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_catalog::{Access, Catalog, PersistenceClass};
    use lambda_core::ValueQuality;

    #[test]
    fn mock_connect_poll_disconnect() {
        let mut session = AppSession::from_shell_defaults();
        let mut state = ShellState::new(cfg!(feature = "mock"));
        session.connect(&mut state);
        assert!(
            state.is_connected(),
            "expected connected, got {:?} msg={}",
            state.status,
            state.status_message
        );
        assert!(state.last_poll.is_some(), "connect should trigger a poll");
        assert!(!state.parameters.is_empty());
        session.refresh(&mut state);
        session.disconnect(&mut state);
        assert!(!state.is_connected());
    }

    #[test]
    fn refresh_while_disconnected_sets_message() {
        let mut session = AppSession::from_shell_defaults();
        let mut state = ShellState::new(true);
        session.refresh(&mut state);
        assert_eq!(state.status_message, strings::MSG_NOT_CONNECTED);
    }

    #[test]
    fn confirmed_write_updates_value_and_active_for_cyclic() {
        let catalog = Catalog::official();
        let param = catalog
            .parameters
            .iter()
            .find(|p| {
                p.access == Access::ReadWrite
                    && p.write_group_id.is_none()
                    && p.persistence_class == PersistenceClass::Cyclic
                    && p.group == "Heizkreis 1"
            })
            .expect("cyclic RW HK1 param");

        let mut session = AppSession::from_shell_defaults();
        let mut state = ShellState::new(true);
        session.connect(&mut state);
        state.select_group("Heizkreis 1");

        session.write_register(&mut state, &param.param_id, 25.0, false);
        assert!(state.active_entries.is_empty());
        assert!(
            state.status_message.contains("nicht bestätigt")
                || state.status_message.contains("bestätigt")
        );

        session.write_register(&mut state, &param.param_id, 25.0, true);
        assert!(
            !state.active_entries.is_empty(),
            "cyclic write should enter Active"
        );
        let row = state
            .parameters
            .iter()
            .find(|r| r.param_id == param.param_id)
            .expect("row");
        assert_eq!(row.quality, Some(ValueQuality::Good));
        assert!(row.display.is_some());

        session.clear_active_setpoint(&mut state, &param.param_id);
        assert!(state.active_entries.is_empty());
    }

    #[test]
    fn read_only_write_rejected() {
        let catalog = Catalog::official();
        let param = catalog
            .parameters
            .iter()
            .find(|p| p.access == Access::Read && p.group == "Allgemein")
            .expect("RO param");
        let mut session = AppSession::from_shell_defaults();
        let mut state = ShellState::new(true);
        session.connect(&mut state);
        session.write_register(&mut state, &param.param_id, 1.0, true);
        assert!(state.active_entries.is_empty());
        assert!(
            state.status_message.contains("nicht schreibbar")
                || state.status_message.contains("Schreib")
        );
    }

    #[test]
    fn write_group_confirm_path() {
        let catalog = Catalog::official();
        let group = catalog.by_write_group("buffer.request_temps");
        assert!(group.len() >= 2);
        let values: Vec<(String, f64)> = group.iter().map(|p| (p.param_id.clone(), 30.0)).collect();

        let mut session = AppSession::from_shell_defaults();
        let mut state = ShellState::new(true);
        session.connect(&mut state);

        // Cancel semantics at UI layer: confirmed=false must not activate.
        session.write_register_group(&mut state, "buffer.request_temps", &values, false);
        assert!(state.active_entries.is_empty());

        session.write_register_group(&mut state, "buffer.request_temps", &values, true);
        assert!(
            state
                .active_entries
                .iter()
                .any(|e| e.key == "buffer.request_temps" || e.label.contains("buffer")),
            "group should appear in Active: {:?}",
            state.active_entries
        );
    }
}
