//! UI view-model: maps core [`Event`]s → shell state (testable without GPUI).

use std::collections::HashMap;

use lambda_catalog::{Access, Catalog, DataType, Parameter, PersistenceClass};
use lambda_core::{
    defaults, ActiveSetpoint, ConnectionStatus, Event, PollTrigger, ValueQuality, WriteSource,
    ACTIVE_SETPOINTS_FILE_NAME,
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

/// One catalog parameter row in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamRow {
    pub param_id: String,
    pub label_de: String,
    pub address: u16,
    pub type_label: String,
    pub scale: f64,
    pub unit: String,
    pub writable: bool,
    pub write_group_id: Option<String>,
    pub persistence_cyclic: bool,
    pub range_min: Option<f64>,
    pub range_max: Option<f64>,
    pub enum_labels: Vec<(i64, String)>,
    pub raw: Option<i64>,
    pub display: Option<f64>,
    pub quality: Option<ValueQuality>,
    /// Edit buffer (display value as text). RO rows keep this empty/unused.
    pub draft: String,
}

impl ParamRow {
    fn from_catalog(p: &Parameter) -> Self {
        Self {
            param_id: p.param_id.clone(),
            label_de: p.label_de.clone(),
            address: p.address,
            type_label: data_type_label(p.data_type).to_string(),
            scale: p.scale,
            unit: p.unit.clone(),
            writable: p.access == Access::ReadWrite,
            write_group_id: p.write_group_id.clone(),
            persistence_cyclic: p.persistence_class == PersistenceClass::Cyclic,
            range_min: p.range_min,
            range_max: p.range_max,
            enum_labels: p
                .enum_values
                .as_ref()
                .map(|v| {
                    v.iter()
                        .map(|e| (e.raw, e.label_de.clone()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            raw: None,
            display: None,
            quality: None,
            draft: String::new(),
        }
    }

    pub fn access_label(&self) -> &'static str {
        if self.writable {
            "R/W"
        } else {
            "R"
        }
    }

    pub fn quality_label(&self) -> &'static str {
        match self.quality {
            Some(ValueQuality::Good) => strings::QUALITY_GOOD,
            Some(ValueQuality::Stale) => strings::QUALITY_STALE,
            Some(ValueQuality::Error) => strings::QUALITY_ERROR,
            None => strings::STATUS_NONE,
        }
    }

    pub fn enum_display(&self) -> Option<String> {
        let raw = self.raw?;
        let label = self
            .enum_labels
            .iter()
            .find(|(r, _)| *r == raw)
            .map(|(_, l)| l.as_str())?;
        Some(format!("{label} ({raw})"))
    }

    pub fn display_text(&self) -> String {
        if let Some(enum_txt) = self.enum_display() {
            return enum_txt;
        }
        match self.display {
            Some(v) => format!("{v}"),
            None => strings::STATUS_NONE.to_string(),
        }
    }
}

fn data_type_label(dt: DataType) -> &'static str {
    match dt {
        DataType::Int16 => "INT16",
        DataType::Uint16 => "UINT16",
        DataType::Int32 => "INT32",
    }
}

/// Pending confirmation before any Modbus write / clear-all.
#[derive(Debug, Clone, PartialEq)]
pub enum PendingConfirm {
    WriteSingle {
        param_id: String,
        label: String,
        display_value: f64,
    },
    WriteGroup {
        write_group_id: String,
        label: String,
        values: Vec<(String, f64)>,
    },
    ClearAllActive,
}

/// Active setpoint row for the side panel.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveRow {
    pub key: String,
    pub label: String,
    pub display_value: f64,
    pub last_write_at: Option<String>,
    pub last_error: Option<String>,
}

/// Shell UI state.
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
    /// @deprecated prefer [`Self::active_entries`]; kept for M6 compatibility in labels.
    pub active_labels: Vec<String>,
    pub active_entries: Vec<ActiveRow>,
    /// All parameters for the currently selected group.
    pub parameters: Vec<ParamRow>,
    pub filter_query: String,
    /// Which draft field is focused in the GUI (`param_id`).
    pub edit_focus: Option<String>,
    pub pending: Option<PendingConfirm>,
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
        let mut state = Self {
            connection: ConnectionFields::default(),
            status: ConnectionStatus::Disconnected,
            selected_group: selected_group.clone(),
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
            active_entries: Vec::new(),
            parameters: Vec::new(),
            filter_query: String::new(),
            edit_focus: None,
            pending: None,
        };
        state.reload_group_parameters();
        state
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
            self.reload_group_parameters();
            self.edit_focus = None;
        }
    }

    fn reload_group_parameters(&mut self) {
        // Preserve live values across group switches via a temp map of previous rows.
        let prev: HashMap<String, ParamRow> = self
            .parameters
            .drain(..)
            .map(|r| (r.param_id.clone(), r))
            .collect();
        let catalog = Catalog::official();
        let mut rows: Vec<ParamRow> = catalog
            .by_group(&self.selected_group)
            .into_iter()
            .map(|p| {
                let mut row = ParamRow::from_catalog(p);
                if let Some(old) = prev.get(&row.param_id) {
                    row.raw = old.raw;
                    row.display = old.display;
                    row.quality = old.quality;
                    if old.writable && !old.draft.is_empty() {
                        row.draft = old.draft.clone();
                    } else if let Some(d) = row.display {
                        row.draft = format!("{d}");
                    }
                }
                row
            })
            .collect();
        rows.sort_by_key(|r| r.address);
        self.parameters = rows;
    }

    /// Visible rows after filter (search in name / id / address).
    pub fn filtered_parameters(&self) -> Vec<&ParamRow> {
        let q = self.filter_query.trim().to_lowercase();
        self.parameters
            .iter()
            .filter(|r| {
                if q.is_empty() {
                    return true;
                }
                r.label_de.to_lowercase().contains(&q)
                    || r.param_id.to_lowercase().contains(&q)
                    || r.address.to_string().contains(&q)
            })
            .collect()
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

    pub fn sync_active_from(&mut self, entries: &[ActiveSetpoint]) {
        self.active_count = entries.len();
        self.active_entries = entries
            .iter()
            .map(|e| {
                let label = e
                    .write_group_id
                    .clone()
                    .unwrap_or_else(|| e.param_id.clone());
                ActiveRow {
                    key: e.key().to_string(),
                    label,
                    display_value: e.display_value,
                    last_write_at: e.last_write_at.clone(),
                    last_error: e.last_error.clone(),
                }
            })
            .collect();
        self.active_labels = self
            .active_entries
            .iter()
            .map(|a| {
                if let Some(err) = &a.last_error {
                    format!("{} = {} ({err})", a.label, a.display_value)
                } else {
                    format!("{} = {}", a.label, a.display_value)
                }
            })
            .collect();
    }

    fn upsert_param_value(
        &mut self,
        param_id: &str,
        raw: Option<i64>,
        display: Option<f64>,
        quality: ValueQuality,
    ) {
        if let Some(row) = self.parameters.iter_mut().find(|r| r.param_id == param_id) {
            if let Some(r) = raw {
                row.raw = Some(r);
            }
            if let Some(d) = display {
                row.display = Some(d);
                let editing = row.writable && self.edit_focus.as_deref() == Some(param_id);
                if !editing {
                    row.draft = format!("{d}");
                }
            }
            row.quality = Some(quality);
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
                        for row in &mut self.parameters {
                            if row.quality == Some(ValueQuality::Good) {
                                row.quality = Some(ValueQuality::Stale);
                            }
                        }
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
            Event::ParameterUpdated {
                param_id,
                raw,
                display,
                quality,
            } => {
                self.upsert_param_value(param_id, Some(*raw), Some(*display), *quality);
                true
            }
            Event::ParameterError { param_id, message } => {
                if let Some(row) = self.parameters.iter_mut().find(|r| r.param_id == *param_id) {
                    row.quality = Some(ValueQuality::Error);
                }
                self.status_message = format!("{}: {message}", strings::STATUS_ERROR_PREFIX);
                true
            }
            Event::CyclicWriteStarted { count } => {
                self.active_count = (*count as usize).max(self.active_count);
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
            Event::ActiveSetpointsChanged => true,
            Event::WriteSucceeded { source, .. } => {
                if matches!(source, WriteSource::User) {
                    self.status_message = strings::MSG_WRITE_OK.to_string();
                    self.pending = None;
                }
                true
            }
            Event::WriteFailed {
                message, source, ..
            } => {
                let prefix = match source {
                    WriteSource::User => strings::MSG_WRITE_FAIL,
                    WriteSource::Cyclic => strings::MSG_CYCLIC_WRITE_FAIL,
                };
                self.status_message = format!("{prefix}: {message}");
                true
            }
            Event::WriteRejected { reason } => {
                self.status_message = reason.clone();
                true
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

    /// Validate draft and open confirm dialog. Returns error message on failure.
    pub fn request_write(&mut self, param_id: &str) -> Result<(), String> {
        let row = self
            .parameters
            .iter()
            .find(|r| r.param_id == param_id)
            .ok_or_else(|| format!("Parameter nicht gefunden: {param_id}"))?;
        if !row.writable {
            return Err(strings::MSG_NOT_WRITABLE.to_string());
        }
        if !self.is_connected() {
            return Err(strings::MSG_NOT_CONNECTED.to_string());
        }
        let display_value = row
            .draft
            .trim()
            .parse::<f64>()
            .map_err(|_| strings::MSG_INVALID_VALUE.to_string())?;
        if let (Some(min), Some(max)) = (row.range_min, row.range_max) {
            if display_value < min || display_value > max {
                return Err(format!(
                    "{}: {display_value} nicht in [{min}, {max}]",
                    strings::MSG_OUT_OF_RANGE
                ));
            }
        }

        if let Some(group_id) = row.write_group_id.clone() {
            let values = self.collect_group_drafts(&group_id)?;
            self.pending = Some(PendingConfirm::WriteGroup {
                write_group_id: group_id.clone(),
                label: format!("{} ({group_id})", strings::CONFIRM_GROUP_WRITE),
                values,
            });
        } else {
            self.pending = Some(PendingConfirm::WriteSingle {
                param_id: row.param_id.clone(),
                label: row.label_de.clone(),
                display_value,
            });
        }
        Ok(())
    }

    fn collect_group_drafts(&self, write_group_id: &str) -> Result<Vec<(String, f64)>, String> {
        let catalog = Catalog::official();
        let group = catalog.by_write_group(write_group_id);
        if group.is_empty() {
            return Err(format!("Unbekannte Schreibgruppe: {write_group_id}"));
        }
        let mut values = Vec::new();
        for p in group {
            // Prefer draft from loaded rows; fall back to catalog live display if in another group.
            let display =
                if let Some(row) = self.parameters.iter().find(|r| r.param_id == p.param_id) {
                    row.draft
                        .trim()
                        .parse::<f64>()
                        .or_else(|_| row.display.ok_or(()))
                        .map_err(|_| format!("{}: {}", strings::MSG_INVALID_VALUE, p.label_de))?
                } else {
                    // Parameter of the group not in current view — require we have a display from events.
                    // Load draft from a temporary ParamRow defaults: use 0.0 only if encode allows — better reject.
                    return Err(format!(
                        "Gruppenparameter {} nicht in aktueller Ansicht — bitte Gruppe „{}“ wählen",
                        p.param_id, p.group
                    ));
                };
            if let (Some(min), Some(max)) = (p.range_min, p.range_max) {
                if display < min || display > max {
                    return Err(format!(
                        "{} ({}): {display} nicht in [{min}, {max}]",
                        strings::MSG_OUT_OF_RANGE,
                        p.label_de
                    ));
                }
            }
            values.push((p.param_id.clone(), display));
        }
        Ok(values)
    }

    pub fn request_clear_all_active(&mut self) {
        if self.active_entries.is_empty() {
            self.status_message = strings::ACTIVE_EMPTY.to_string();
            return;
        }
        self.pending = Some(PendingConfirm::ClearAllActive);
    }

    pub fn cancel_pending(&mut self) {
        self.pending = None;
        self.status_message = strings::MSG_WRITE_CANCELLED.to_string();
    }

    pub fn pending_summary(&self) -> Option<String> {
        match &self.pending {
            Some(PendingConfirm::WriteSingle {
                label,
                display_value,
                ..
            }) => Some(format!("{label} → {display_value}")),
            Some(PendingConfirm::WriteGroup { label, values, .. }) => {
                Some(format!("{label} ({} Werte)", values.len()))
            }
            Some(PendingConfirm::ClearAllActive) => {
                Some(strings::CONFIRM_CLEAR_ALL_DETAIL.to_string())
            }
            None => None,
        }
    }

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
        assert!(!s.parameters.is_empty());
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
        assert!(s.parameters.iter().all(|p| {
            Catalog::official()
                .get(&p.param_id)
                .map(|c| c.group == "Heizkreis 2")
                .unwrap_or(false)
        }));
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

    #[test]
    fn request_write_ro_rejected() {
        let mut s = ShellState::new(true);
        s.status = ConnectionStatus::Connected;
        s.select_group("Allgemein");
        let ro = s
            .parameters
            .iter()
            .find(|p| !p.writable)
            .expect("RO")
            .param_id
            .clone();
        assert!(s.request_write(&ro).is_err());
        assert!(s.pending.is_none());
    }

    #[test]
    fn request_write_opens_confirm_for_rw() {
        let mut s = ShellState::new(true);
        s.status = ConnectionStatus::Connected;
        s.select_group("Heizkreis 1");
        let (id, _) = s
            .parameters
            .iter()
            .find(|p| p.writable && p.write_group_id.is_none())
            .map(|p| (p.param_id.clone(), p.draft.clone()))
            .expect("RW");
        if let Some(row) = s.parameters.iter_mut().find(|p| p.param_id == id) {
            row.draft = "25".into();
        }
        s.request_write(&id).unwrap();
        assert!(matches!(
            s.pending,
            Some(PendingConfirm::WriteSingle { .. })
        ));
        s.cancel_pending();
        assert!(s.pending.is_none());
    }

    #[test]
    fn parameter_updated_fills_row() {
        let mut s = ShellState::new(true);
        s.select_group("Allgemein");
        let id = s.parameters[0].param_id.clone();
        s.apply_event(&Event::ParameterUpdated {
            param_id: id.clone(),
            raw: 210,
            display: 21.0,
            quality: ValueQuality::Good,
        });
        let row = s.parameters.iter().find(|p| p.param_id == id).unwrap();
        assert_eq!(row.display, Some(21.0));
        assert_eq!(row.quality, Some(ValueQuality::Good));
    }
}
