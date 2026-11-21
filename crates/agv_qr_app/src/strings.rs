//! German user-facing copy for the AGV QR test GUI.

pub const WINDOW_TITLE: &str = "AGV QR — Bodenmarken-Lokalisation (Test)";
pub const APP_HEADING: &str = "AGV QR Lokalisation";
pub const APP_SUBHEADING: &str = "Test-GUI · Simulation · keine Live-Kamera";

pub const SECTION_POSE: &str = "Simulierte Pose";
pub const SECTION_ESTIMATE: &str = "Geschätzte Pose";
pub const SECTION_MAP: &str = "Markerkarte";
pub const SECTION_DETECT: &str = "Letzte Erkennung";
pub const SECTION_CONTROLS: &str = "Steuerung";

pub const LABEL_X: &str = "x";
pub const LABEL_Y: &str = "y";
pub const LABEL_THETA: &str = "θ";
pub const LABEL_TIME: &str = "Zeit";
pub const LABEL_QUALITY: &str = "Qualität";
pub const LABEL_RMSE: &str = "Reprojektion RMSE";
pub const LABEL_CORNERS: &str = "Ecken";
pub const LABEL_MARKER: &str = "Marker-ID";
pub const LABEL_AGE: &str = "Alter Fix";
pub const LABEL_MAP_VERSION: &str = "Kartenversion";
pub const LABEL_MARKER_COUNT: &str = "Anzahl Marker";
pub const LABEL_RUNNING: &str = "Simulation";

pub const UNIT_M: &str = "m";
pub const UNIT_DEG: &str = "°";
pub const UNIT_S: &str = "s";
pub const UNIT_PX: &str = "px";

pub const BTN_START: &str = "Start";
pub const BTN_STOP: &str = "Stopp";
pub const BTN_RESET: &str = "Zurücksetzen";

pub const STATUS_RUNNING: &str = "läuft";
pub const STATUS_STOPPED: &str = "gestoppt";
pub const STATUS_NONE: &str = "—";

pub const COL_ID: &str = "ID";
pub const COL_X: &str = "x [m]";
pub const COL_Y: &str = "y [m]";
pub const COL_YAW: &str = "Yaw [°]";
pub const COL_SIZE: &str = "Größe [m]";

pub const HINT_SIM: &str =
    "Mock-Simulation der AGV-Pose über Lab-Raster 3×3. Vision und Kamera sind Stub.";
pub const HEADLESS_DONE: &str = "Headless-Smoke bereit (GUI-Feature aus).";
pub const FOOTER_BACKEND: &str = "Sim-Backend · Stub-Vision";
