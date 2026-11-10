//! German user-facing copy for the M6 shell (and later UI milestones).

/// Window / app title.
pub const WINDOW_TITLE: &str = "Lambda Modbus — EU13L";

pub const APP_HEADING: &str = "Lambda Modbus";
pub const APP_SUBHEADING: &str = "EU13L · Modbus TCP · offizielle Registermap";

pub const CONNECTION_SECTION: &str = "Verbindung";
pub const LABEL_HOST: &str = "IP / Host";
pub const LABEL_PORT: &str = "Port";
pub const LABEL_UNIT: &str = "Unit-ID";
pub const BTN_CONNECT: &str = "Verbinden";
pub const BTN_DISCONNECT: &str = "Trennen";
pub const BTN_REFRESH: &str = "Jetzt aktualisieren";

pub const GROUPS_SECTION: &str = "Gruppen";
pub const MAIN_SECTION: &str = "Parameter";
pub const MAIN_PLACEHOLDER: &str =
    "Parameterliste (Lesen/Schreiben + Bestätigungsdialog) folgt in M7.";
pub const MAIN_GROUP_HINT: &str = "Gewählte Gruppe";

pub const ACTIVE_SECTION: &str = "Aktive Sollwerte";
pub const ACTIVE_EMPTY: &str = "Keine aktiven Sollwerte.";
pub const ACTIVE_HINT: &str =
    "Nach bestätigtem Schreiben (M7) erscheinen hier zyklische Sollwerte (60 s).";
pub const BTN_CLEAR_ALL_ACTIVE: &str = "Alle freigeben";

pub const STATUS_SECTION: &str = "Status";
pub const STATUS_CONNECTION: &str = "Verbindung";
pub const STATUS_POLL: &str = "Poll";
pub const STATUS_CYCLIC: &str = "Zyklisch";
pub const STATUS_BACKEND_MOCK: &str = "Mock-Backend";
pub const STATUS_BACKEND_LIVE: &str = "TCP-Backend";
pub const STATUS_NONE: &str = "—";
pub const STATUS_POLLING: &str = "Aktualisiere…";
pub const STATUS_POLL_OK: &str = "OK";
pub const STATUS_POLL_PARTIAL: &str = "Teilfehler";

pub const STATUS_DISCONNECTED: &str = "Getrennt";
pub const STATUS_CONNECTING: &str = "Verbinden…";
pub const STATUS_CONNECTED: &str = "Verbunden";
pub const STATUS_RECONNECTING: &str = "Erneutes Verbinden…";
pub const STATUS_ERROR_PREFIX: &str = "Fehler";

pub const MSG_NOT_CONNECTED: &str = "Nicht verbunden — bitte zuerst verbinden.";
pub const MSG_CONNECT_OK: &str = "Verbindung hergestellt.";
pub const MSG_DISCONNECT_OK: &str = "Verbindung getrennt.";
pub const MSG_REFRESH_OK: &str = "Manuelle Aktualisierung gestartet.";
pub const MSG_INVALID_PORT: &str = "Ungültiger Port.";
pub const MSG_INVALID_UNIT: &str = "Ungültige Unit-ID.";
pub const MSG_MOCK_NOTE: &str = "Cloud/Dev: Mock-Backend aktiv (keine echte Wärmepumpe).";

/// Navigation groups for v1 (incl. Heizkreis 1 / 2). Order matches catalog UX.
pub const NAV_GROUPS: &[&str] = &[
    "Allgemein",
    "E-Manager",
    "Wärmepumpe",
    "Warmwasser",
    "Puffer",
    "Solar",
    "Heizkreis 1",
    "Heizkreis 2",
];
