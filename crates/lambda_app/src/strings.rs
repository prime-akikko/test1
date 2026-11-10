//! German user-facing copy for the GPUI shell.

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
pub const MAIN_GROUP_HINT: &str = "Gewählte Gruppe";
pub const FILTER_LABEL: &str = "Suche";
pub const FILTER_PLACEHOLDER: &str = "Name, ID oder Adresse…";

pub const COL_NAME: &str = "Name";
pub const COL_ADDR: &str = "Adr.";
pub const COL_TYPE: &str = "Typ";
pub const COL_SCALE: &str = "Skala";
pub const COL_UNIT: &str = "Einheit";
pub const COL_RAW: &str = "Roh";
pub const COL_VALUE: &str = "Wert";
pub const COL_ACCESS: &str = "R/W";
pub const COL_QUALITY: &str = "Qualität";
pub const COL_EDIT: &str = "Neu";
pub const BTN_WRITE: &str = "Schreiben";

pub const QUALITY_GOOD: &str = "Gut";
pub const QUALITY_STALE: &str = "Veraltet";
pub const QUALITY_ERROR: &str = "Fehler";

pub const CONFIRM_TITLE: &str = "Wirklich schreiben?";
pub const CONFIRM_CLEAR_ALL_TITLE: &str = "Alle aktiven Sollwerte freigeben?";
pub const CONFIRM_CLEAR_ALL_DETAIL: &str =
    "Zyklisches Nachschreiben aller aktiven Sollwerte wird beendet.";
pub const CONFIRM_GROUP_WRITE: &str = "Schreibgruppe";
pub const BTN_CONFIRM: &str = "Bestätigen";
pub const BTN_CANCEL: &str = "Abbrechen";

pub const ACTIVE_SECTION: &str = "Aktive Sollwerte";
pub const ACTIVE_EMPTY: &str = "Keine aktiven Sollwerte.";
pub const ACTIVE_HINT: &str =
    "Nach bestätigtem Schreiben erscheinen hier zyklische Sollwerte (60 s).";
pub const BTN_CLEAR_ACTIVE: &str = "Freigeben";
pub const BTN_CLEAR_ALL_ACTIVE: &str = "Alle freigeben";

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
pub const MSG_INVALID_VALUE: &str = "Ungültiger Wert.";
pub const MSG_OUT_OF_RANGE: &str = "Außerhalb des zulässigen Bereichs";
pub const MSG_NOT_WRITABLE: &str = "Parameter ist nur lesbar.";
pub const MSG_WRITE_OK: &str = "Schreiben erfolgreich.";
pub const MSG_WRITE_FAIL: &str = "Schreiben fehlgeschlagen";
pub const MSG_CYCLIC_WRITE_FAIL: &str = "Zyklisches Schreiben fehlgeschlagen";
pub const MSG_WRITE_CANCELLED: &str = "Schreiben abgebrochen.";
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
