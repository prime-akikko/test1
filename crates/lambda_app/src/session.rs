//! Mock-first backend session for the shell: drives [`PollScheduler`] and mirrors events.
//!
//! Cyclic Active-Setpoint rewrites are shown as an empty panel in M6; full write UI is M7.
//! When the `mock` feature is off, connect uses the real TCP client (local Windows path).

use std::time::Duration;

use lambda_core::{
    defaults, ConnectionStatus, Event, PollConfig, PollScheduler, DEFAULT_POLL_INTERVAL,
};

use crate::state::ShellState;
use crate::strings;

/// Owns the Modbus-backed poll scheduler used by the desktop shell.
pub struct AppSession {
    inner: SessionInner,
}

enum SessionInner {
    #[cfg(feature = "mock")]
    Mock(PollScheduler<lambda_core::QueuedTransport<lambda_mock::MockSlave>>),
    #[cfg(not(feature = "mock"))]
    Live(PollScheduler<lambda_core::TcpModbusClient>),
}

impl AppSession {
    /// Build a session. With default app features this is the in-memory mock.
    pub fn new(unit_id: u8) -> Self {
        let config = PollConfig {
            unit_id,
            poll_interval: DEFAULT_POLL_INTERVAL,
        };
        #[cfg(feature = "mock")]
        {
            let slave = lambda_core::new_mock_transport(unit_id);
            let transport = lambda_core::QueuedTransport::new(slave);
            Self {
                inner: SessionInner::Mock(PollScheduler::new(transport, config)),
            }
        }
        #[cfg(not(feature = "mock"))]
        {
            let client = lambda_core::TcpModbusClient::new(lambda_core::ClientConfig {
                unit_id,
                ..Default::default()
            });
            Self {
                inner: SessionInner::Live(PollScheduler::new(client, config)),
            }
        }
    }

    pub fn from_shell_defaults() -> Self {
        Self::new(defaults::UNIT_ID)
    }

    pub fn status(&self) -> &ConnectionStatus {
        match &self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock(p) => p.status(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.status(),
        }
    }

    pub fn poll_interval(&self) -> Duration {
        match &self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock(p) => p.poll_interval(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.poll_interval(),
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

        // Rebuild session if unit id changed (mock/TCP config carries unit).
        if unit_id != self.unit_id() {
            *self = Self::new(unit_id);
        }

        let result = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock(p) => p.connect(&host, port),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.connect(&host, port),
        };
        self.drain_into(state);
        if let Err(err) = result {
            state.status_message = format!("{}: {err}", strings::STATUS_ERROR_PREFIX);
        }
    }

    pub fn disconnect(&mut self, state: &mut ShellState) {
        match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock(p) => p.disconnect(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.disconnect(),
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
            SessionInner::Mock(p) => p.read_all(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.read_all(),
        };
        self.drain_into(state);
        if !ran {
            state.status_message = strings::MSG_NOT_CONNECTED.to_string();
        }
    }

    /// Focus-group hint: optional targeted read (still M6 — main list is placeholder).
    pub fn refresh_selected_group(&mut self, state: &mut ShellState) {
        if !matches!(self.status(), ConnectionStatus::Connected) {
            return;
        }
        let group = state.selected_group.clone();
        let _ = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock(p) => p.read_group(&group),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.read_group(&group),
        };
        self.drain_into(state);
    }

    /// Advance the poll timer if due.
    pub fn tick(&mut self, state: &mut ShellState) -> bool {
        let ran = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock(p) => p.tick(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.tick(),
        };
        self.drain_into(state);
        ran
    }

    pub fn drain_into(&mut self, state: &mut ShellState) {
        let events: Vec<Event> = match &mut self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock(p) => p.drain_events(),
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.drain_events(),
        };
        state.apply_events(events);
    }

    fn unit_id(&self) -> u8 {
        match &self.inner {
            #[cfg(feature = "mock")]
            SessionInner::Mock(p) => p.config().unit_id,
            #[cfg(not(feature = "mock"))]
            SessionInner::Live(p) => p.config().unit_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        session.refresh(&mut state);
        assert!(state.last_poll.is_some());
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
}
