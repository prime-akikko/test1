//! Pose output interface (design §2).
//!
//! Time-stamped pose sample: \((x, y, \theta)\) in map frame + quality + marker IDs.

use crate::vision::DetectionQuality;

/// Planar AGV pose in the map / world frame (z up, right-handed).
#[derive(Debug, Clone, PartialEq)]
pub struct PoseSample {
    /// Metres, map frame.
    pub x_m: f64,
    /// Metres, map frame.
    pub y_m: f64,
    /// Radians, yaw about z (CCW from +x).
    pub theta_rad: f64,
    /// Monotonic simulation / wall time in seconds.
    pub timestamp_s: f64,
    /// Marker IDs that contributed to this fix (empty = coast / odom-only).
    pub marker_ids: Vec<u32>,
    pub quality: PoseQuality,
}

/// Coarse quality flag for the UI and consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoseQuality {
    /// Absolute fix from ≥1 marker with acceptable reprojection.
    Good,
    /// Odometry coast between markers / degraded detect.
    Degraded,
    /// No usable pose (sim stopped or fail-safe).
    Lost,
}

impl PoseQuality {
    pub fn as_str_de(self) -> &'static str {
        match self {
            Self::Good => "Gut",
            Self::Degraded => "Eingeschränkt",
            Self::Lost => "Verloren",
        }
    }
}

impl PoseSample {
    pub fn heading_deg(&self) -> f64 {
        self.theta_rad.to_degrees()
    }
}

/// Sink that receives pose samples (ROS / protobuf / custom controller later).
pub trait PoseOutput {
    fn publish(&mut self, sample: &PoseSample);
}

/// In-memory sink for tests and the GPUI shell.
#[derive(Debug, Default, Clone)]
pub struct PoseSink {
    pub last: Option<PoseSampleOwned>,
    pub count: u64,
}

/// Owned pose + detection quality for UI snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct PoseSampleOwned {
    pub x_m: f64,
    pub y_m: f64,
    pub theta_rad: f64,
    pub timestamp_s: f64,
    pub marker_ids: Vec<u32>,
    pub quality: PoseQuality,
    pub detection: DetectionQuality,
}

impl PoseSampleOwned {
    pub fn from_sample(sample: &PoseSample, detection: DetectionQuality) -> Self {
        Self {
            x_m: sample.x_m,
            y_m: sample.y_m,
            theta_rad: sample.theta_rad,
            timestamp_s: sample.timestamp_s,
            marker_ids: sample.marker_ids.clone(),
            quality: sample.quality,
            detection,
        }
    }

    pub fn heading_deg(&self) -> f64 {
        self.theta_rad.to_degrees()
    }
}

impl PoseOutput for PoseSink {
    fn publish(&mut self, sample: &PoseSample) {
        self.last = Some(PoseSampleOwned::from_sample(
            sample,
            DetectionQuality::default(),
        ));
        self.count = self.count.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_records_sample() {
        let mut sink = PoseSink::default();
        let sample = PoseSample {
            x_m: 1.0,
            y_m: 2.0,
            theta_rad: 0.5,
            timestamp_s: 1.0,
            marker_ids: vec![],
            quality: PoseQuality::Good,
        };
        sink.publish(&sample);
        assert_eq!(sink.count, 1);
        let last = sink.last.as_ref().unwrap();
        assert!((last.x_m - 1.0).abs() < 1e-9);
        assert_eq!(last.quality, PoseQuality::Good);
    }
}
