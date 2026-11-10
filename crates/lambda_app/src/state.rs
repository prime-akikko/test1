//! UI view-model: maps core [`Event`]s → shell state (testable without GPUI).

use lambda_core::{
    defaults, ConnectionStatus, Event, PollTrigger, WriteSource, ACTIVE_SETPOINTS_FILE_NAME,
};

use crate::strings;

/// Snapshot of connection fields edited in the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionFields {
    pub host: String,
    pub port: String,
    pub unit_id: String,
}

impl Default for ConnectionFields {
    fn default() -> Self {
        Self {
            host: defaults::HOST.to_string(),
            port: defaults::PORT.to_string(),
            unit_id: defaults::UNIT_ID.to_string(),
        }
    }
}

impl ConnectionFields {
    pub fn parse_port(&self) -> Result<u16, &'static str> {
        self.port
            .trim()
            .parse::<u16>()
            .map_err(|_| strings::MSG_INVALID_PORT)
    }

    pub fn parse_unit_id(&self) -> Result<u8, &'static str> {
        self.unit_id
            .trim()
            .parse::<u8>()
            .map_err(|_| strings::MSG_INVALID_UNIT)
    }
}

/// Last poll / cyclic summary for the status bar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CycleSummary {
    pub ok_count: u32,
    pub err_count: u32,
    pub label: String,
}

/// Shell UI state (M6). Parameter rows arrive in M7.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellState {
    pub connection: ConnectionFields,
    pub status: ConnectionStatus,
    pub selected_group: String,
    pub groups: Vec<String>,
    pub poll_busy: bool,
    pub last_poll: Option<CycleSummary>,
    pub last_cyclic: Option<CycleSummary>,
    pub active_count: usize,
    pub status_message: String,
    pub mock_backend: bool,
    /// Placeholder list for Active panel (empty until M7 writes).
    pub active_labels: Vec<String>,
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new(cfg!(feature = "mock"))
    }
}

impl ShellState {
    pub fn new(mock_backend: bool) -> Self {
        let groups: Vec<String> = strings::NAV_GROUPS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let selected_group = groups
            .first()
            .cloned()
            .unwrap_or_else(|| "Allgemein".into());
        Self {
            connection: ConnectionFields::default(),
            status: ConnectionStatus::Disconnected,
            selected_group,
            groups,
            poll_busy: false,
            last_poll: None,
            last_cyclic: None,
            active_count: 0,
            status_message: if mock_backend {
                strings::MSG_MOCK_NOTE.to_string()
            } else {
                String::new()
            },
            mock_backend,
            active_labels: Vec::new(),
        }
    }

    pub fn status_label(&self) -> String {
        match &self.status {
            ConnectionStatus::Disconnected => strings::STATUS_DISCONNECTED.to_string(),
            ConnectionStatus::Connecting => strings::STATUS_CONNECTING.to_string(),
            ConnectionStatus::Connected => strings::STATUS_CONNECTED.to_string(),
            ConnectionStatus::Reconnecting => strings::STATUS_RECONNECTING.to_string(),
            ConnectionStatus::Error { message } => {
                format!("{}: {message}", strings::STATUS_ERROR_PREFIX)
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.status, ConnectionStatus::Connected)
    }

    pub fn select_group(&mut self, group: impl Into<String>) {
        let group = group.into();
        if self.groups.iter().any(|g| g == &group) {
            self.selected_group = group;
        }
    }

    pub fn poll_status_text(&self) -> String {
        if self.poll_busy {
            return strings::STATUS_POLLING.to_string();
        }
        match &self.last_poll {
            None => strings::STATUS_NONE.to_string(),
            Some(s) => {
                let kind = if s.err_count == 0 {
                    strings::STATUS_POLL_OK
                } else {
                    strings::STATUS_POLL_PARTIAL
                };
                format!("{kind} · {} ok / {} Fehler", s.ok_count, s.err_count)
            }
        }
    }

    pub fn cyclic_status_text(&self) -> String {
        match &self.last_cyclic {
            None => {
                if self.active_count == 0 {
                    strings::STATUS_NONE.to_string()
                } else {
                    format!("{} aktiv", self.active_count)
                }
            }
            Some(s) => format!(
                "{} aktiv · {} ok / {} Fehler",
                self.active_count, s.ok_count, s.err_count
            ),
        }
    }

    pub fn backend_label(&self) -> &'static str {
        if self.mock_backend {
            strings::STATUS_BACKEND_MOCK
        } else {
            strings::STATUS_BACKEND_LIVE
        }
    }

    /// Apply a core event. Returns `true` if UI should redraw.
    pub fn apply_event(&mut self, event: &Event) -> bool {
        match event {
            Event::ConnectionChanged { status } => {
                self.status = status.clone();
                match status {
                    ConnectionStatus::Connected => {
                        self.status_message = strings::MSG_CONNECT_OK.to_string();
                    }
                    ConnectionStatus::Disconnected => {
                        self.status_message = strings::MSG_DISCONNECT_OK.to_string();
                        self.poll_busy = false;
                    }
                    ConnectionStatus::Error { message } => {
                        self.status_message =
                            format!("{}: {message}", strings::STATUS_ERROR_PREFIX);
                        self.poll_busy = false;
                    }
                    ConnectionStatus::Connecting | ConnectionStatus::Reconnecting => {}
                }
                true
            }
            Event::PollStarted { trigger } => {
                self.poll_busy = true;
                if matches!(trigger, PollTrigger::Manual) {
                    self.status_message = strings::MSG_REFRESH_OK.to_string();
                }
                true
            }
            Event::PollCompleted {
                ok_count,
                err_count,
            } => {
                self.poll_busy = false;
                self.last_poll = Some(CycleSummary {
                    ok_count: *ok_count,
                    err_count: *err_count,
                    label: "Poll".into(),
                });
                true
            }
            Event::CyclicWriteStarted { count } => {
                self.active_count = (*count).max(self.active_count as u32) as usize;
                true
            }
            Event::CyclicWriteCompleted {
                ok_count,
                err_count,
            } => {
                self.last_cyclic = Some(CycleSummary {
                    ok_count: *ok_count,
                    err_count: *err_count,
                    label: "Cyclic".into(),
                });
                true
            }
            Event::ActiveSetpointsChanged => {
                // Detailed entries arrive via session mirror; count may be refreshed by caller.
                true
            }
            Event::WriteSucceeded { source, .. } => {
                if matches!(source, WriteSource::User) {
                    self.status_message = "Schreiben erfolgreich.".into();
                }
                true
            }
            Event::WriteFailed {
                message, source, ..
            } => {
                let prefix = match source {
                    WriteSource::User => "Schreiben fehlgeschlagen",
                    WriteSource::Cyclic => "Zyklisches Schreiben fehlgeschlagen",
                };
                self.status_message = format!("{prefix}: {message}");
                true
            }
            Event::WriteRejected { reason } => {
                self.status_message = reason.clone();
                true
            }
            Event::ParameterUpdated { .. } | Event::ParameterError { .. } => {
                // Parameter rows are M7; still accept events so wiring is live.
                false
            }
        }
    }

    pub fn apply_events<I>(&mut self, events: I) -> bool
    where
        I: IntoIterator<Item = Event>,
    {
        let mut changed = false;
        for event in events {
            changed |= self.apply_event(&event);
        }
        changed
    }

    /// Documentation helper — persistence file name for later M8.
    pub fn active_setpoints_file_name() -> &'static str {
        ACTIVE_SETPOINTS_FILE_NAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_core::PollTrigger;

    #[test]
    fn defaults_match_locked_product() {
        let s = ShellState::new(true);
        assert_eq!(s.connection.host, "10.0.0.6");
        assert_eq!(s.connection.port, "502");
        assert_eq!(s.connection.unit_id, "1");
        assert!(s.groups.iter().any(|g| g == "Heizkreis 1"));
        assert!(s.groups.iter().any(|g| g == "Heizkreis 2"));
        assert_eq!(s.groups.len(), 8);
    }

    #[test]
    fn connection_event_updates_status_de() {
        let mut s = ShellState::new(true);
        s.apply_event(&Event::ConnectionChanged {
            status: ConnectionStatus::Connected,
        });
        assert_eq!(s.status_label(), strings::STATUS_CONNECTED);
        assert!(s.is_connected());
    }

    #[test]
    fn poll_events_drive_status_bar() {
        let mut s = ShellState::new(true);
        s.apply_event(&Event::PollStarted {
            trigger: PollTrigger::Manual,
        });
        assert!(s.poll_busy);
        assert_eq!(s.poll_status_text(), strings::STATUS_POLLING);
        s.apply_event(&Event::PollCompleted {
            ok_count: 10,
            err_count: 1,
        });
        assert!(!s.poll_busy);
        assert!(s.poll_status_text().contains("10"));
        assert!(s.poll_status_text().contains("Teilfehler"));
    }

    #[test]
    fn select_group_only_known() {
        let mut s = ShellState::new(false);
        s.select_group("Heizkreis 2");
        assert_eq!(s.selected_group, "Heizkreis 2");
        s.select_group("Unbekannt");
        assert_eq!(s.selected_group, "Heizkreis 2");
    }

    #[test]
    fn parse_connection_fields() {
        let mut f = ConnectionFields::default();
        assert_eq!(f.parse_port().unwrap(), 502);
        assert_eq!(f.parse_unit_id().unwrap(), 1);
        f.port = "x".into();
        assert!(f.parse_port().is_err());
    }
}
