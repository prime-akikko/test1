//! Vision pipeline stub (detect → decode → PnP). Real camera/OpenCV later.

use crate::marker_map::{Marker, MarkerMap};
use crate::pose::{PoseQuality, PoseSample};

/// Last detection quality metrics for the UI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetectionQuality {
    /// Reprojection RMSE in pixels (sim: synthetic).
    pub reprojection_rmse_px: f64,
    /// Number of corners used (0–4).
    pub corner_count: u8,
    /// Age of last absolute fix in seconds.
    pub age_s: f64,
    /// Marker ID used, if any.
    pub marker_id: Option<u32>,
}

impl Default for DetectionQuality {
    fn default() -> Self {
        Self {
            reprojection_rmse_px: 0.0,
            corner_count: 0,
            age_s: f64::INFINITY,
            marker_id: None,
        }
    }
}

impl DetectionQuality {
    pub fn label_de(self) -> &'static str {
        if self.corner_count >= 4 && self.reprojection_rmse_px < 1.5 {
            "Gut"
        } else if self.corner_count >= 3 && self.reprojection_rmse_px < 3.0 {
            "Mittel"
        } else if self.corner_count > 0 {
            "Schwach"
        } else {
            "Keine Erkennung"
        }
    }
}

/// Trait for the onboard vision block (design §2 / §4).
pub trait VisionPipeline {
    /// Produce a pose sample + quality from the current robot pose (sim) or frame (later).
    fn estimate(
        &mut self,
        map: &MarkerMap,
        true_x: f64,
        true_y: f64,
        true_theta: f64,
        timestamp_s: f64,
        view_radius_m: f64,
    ) -> (PoseSample, DetectionQuality);
}

/// Stub vision: if a marker is within `view_radius_m`, return absolute pose ≈ truth
/// with synthetic quality; otherwise coast with degraded quality.
#[derive(Debug, Default)]
pub struct StubVision {
    last_fix_age_s: f64,
    last_marker_id: Option<u32>,
}

impl VisionPipeline for StubVision {
    fn estimate(
        &mut self,
        map: &MarkerMap,
        true_x: f64,
        true_y: f64,
        true_theta: f64,
        timestamp_s: f64,
        view_radius_m: f64,
    ) -> (PoseSample, DetectionQuality) {
        let nearest = map.nearest(true_x, true_y);
        let in_view = nearest
            .map(|m| (m.x_m - true_x).hypot(m.y_m - true_y) <= view_radius_m)
            .unwrap_or(false);

        if in_view {
            let m: &Marker = nearest.expect("in_view implies nearest");
            self.last_fix_age_s = 0.0;
            self.last_marker_id = Some(m.id);
            // Tiny synthetic noise for realism in the UI.
            let noise = 0.002 * (timestamp_s.sin());
            let sample = PoseSample {
                x_m: true_x + noise,
                y_m: true_y + noise * 0.5,
                theta_rad: true_theta,
                timestamp_s,
                marker_ids: vec![m.id],
                quality: PoseQuality::Good,
            };
            let q = DetectionQuality {
                reprojection_rmse_px: 0.35 + 0.1 * (timestamp_s * 0.7).cos().abs(),
                corner_count: 4,
                age_s: 0.0,
                marker_id: Some(m.id),
            };
            (sample, q)
        } else {
            self.last_fix_age_s += 0.05; // caller ticks ~20 Hz in sim
            let quality = if self.last_fix_age_s < 2.0 {
                PoseQuality::Degraded
            } else {
                PoseQuality::Lost
            };
            let sample = PoseSample {
                x_m: true_x,
                y_m: true_y,
                theta_rad: true_theta,
                timestamp_s,
                marker_ids: vec![],
                quality,
            };
            let q = DetectionQuality {
                reprojection_rmse_px: 0.0,
                corner_count: 0,
                age_s: self.last_fix_age_s,
                marker_id: self.last_marker_id,
            };
            (sample, q)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_detects_near_marker() {
        let map = MarkerMap::lab_grid_3x3();
        let mut vision = StubVision::default();
        let (pose, q) = vision.estimate(&map, 0.0, 0.0, 0.0, 1.0, 0.4);
        assert_eq!(pose.quality, PoseQuality::Good);
        assert_eq!(q.corner_count, 4);
        assert_eq!(q.marker_id, Some(1));
    }

    #[test]
    fn stub_degrades_far_from_markers() {
        let map = MarkerMap::lab_grid_3x3();
        let mut vision = StubVision::default();
        let (pose, q) = vision.estimate(&map, 10.0, 10.0, 0.0, 1.0, 0.4);
        assert_ne!(pose.quality, PoseQuality::Good);
        assert_eq!(q.corner_count, 0);
    }
}
