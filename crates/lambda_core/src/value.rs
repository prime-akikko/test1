//! Runtime parameter values and quality flags (architecture §7).

/// Quality of a cached parameter value after poll / offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueQuality {
    /// Last poll decoded this parameter successfully.
    Good,
    /// Connection lost or value older than last successful poll of peers.
    Stale,
    /// Last attempt for this parameter/block failed.
    Error,
}

/// Cached live value for one catalog parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterValue {
    pub param_id: String,
    pub raw: i64,
    pub display: f64,
    pub quality: ValueQuality,
}
