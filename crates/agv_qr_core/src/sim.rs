//! Mock AGV pose simulator — no live camera; drives stub vision + pose sink.

use crate::marker_map::MarkerMap;
use crate::pose::{PoseOutput, PoseSampleOwned, PoseSink};
use crate::vision::{DetectionQuality, StubVision, VisionPipeline};

/// Simulation control / kinematics config.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimConfig {
    /// Seconds per tick (UI / headless).
    pub dt_s: f64,
    /// Forward speed m/s while running.
    pub speed_m_s: f64,
    /// Yaw rate rad/s (gentle arc through the grid).
    pub yaw_rate_rad_s: f64,
    /// Camera FOV radius on floor for stub detection (m).
    pub view_radius_m: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            dt_s: 0.05,
            speed_m_s: 0.25,
            yaw_rate_rad_s: 0.15,
            view_radius_m: 0.45,
        }
    }
}

/// Public snapshot for the GUI / headless smoke.
#[derive(Debug, Clone, PartialEq)]
pub struct SimState {
    pub running: bool,
    pub true_x_m: f64,
    pub true_y_m: f64,
    pub true_theta_rad: f64,
    pub time_s: f64,
    pub estimated: Option<PoseSampleOwned>,
    pub detection: DetectionQuality,
    pub map_version: String,
    pub marker_count: usize,
}

/// Runs a simple unicycle through the lab marker grid with stub vision.
pub struct SimEngine {
    pub config: SimConfig,
    pub map: MarkerMap,
    vision: StubVision,
    sink: PoseSink,
    running: bool,
    true_x: f64,
    true_y: f64,
    true_theta: f64,
    time_s: f64,
    last_detection: DetectionQuality,
}

impl SimEngine {
    pub fn new(map: MarkerMap, config: SimConfig) -> Self {
        Self {
            config,
            map,
            vision: StubVision::default(),
            sink: PoseSink::default(),
            running: false,
            true_x: 0.0,
            true_y: 0.0,
            true_theta: 0.0,
            time_s: 0.0,
            last_detection: DetectionQuality::default(),
        }
    }

    pub fn lab_default() -> Self {
        Self::new(MarkerMap::lab_grid_3x3(), SimConfig::default())
    }

    pub fn start(&mut self) {
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn reset(&mut self) {
        self.running = false;
        self.true_x = 0.0;
        self.true_y = 0.0;
        self.true_theta = 0.0;
        self.time_s = 0.0;
        self.vision = StubVision::default();
        self.sink = PoseSink::default();
        self.last_detection = DetectionQuality::default();
        self.publish_once();
    }

    /// Advance one tick if running; always refreshes estimated pose.
    pub fn tick(&mut self) {
        if self.running {
            let dt = self.config.dt_s;
            self.true_theta += self.config.yaw_rate_rad_s * dt;
            self.true_x += self.config.speed_m_s * self.true_theta.cos() * dt;
            self.true_y += self.config.speed_m_s * self.true_theta.sin() * dt;
            // Soft clamp to stay near the 3×3 grid.
            self.true_x = self.true_x.clamp(-0.5, 2.5);
            self.true_y = self.true_y.clamp(-0.5, 2.5);
            self.time_s += dt;
        }
        self.publish_once();
    }

    fn publish_once(&mut self) {
        let (sample, quality) = self.vision.estimate(
            &self.map,
            self.true_x,
            self.true_y,
            self.true_theta,
            self.time_s,
            self.config.view_radius_m,
        );
        self.last_detection = quality;
        let owned = PoseSampleOwned::from_sample(&sample, quality);
        self.sink.last = Some(owned);
        self.sink.count = self.sink.count.saturating_add(1);
        // Also exercise the PoseOutput trait path.
        let mut discard = PoseSink::default();
        discard.publish(&sample);
    }

    pub fn state(&self) -> SimState {
        SimState {
            running: self.running,
            true_x_m: self.true_x,
            true_y_m: self.true_y,
            true_theta_rad: self.true_theta,
            time_s: self.time_s,
            estimated: self.sink.last.clone(),
            detection: self.last_detection,
            map_version: self.map.version.clone(),
            marker_count: self.map.len(),
        }
    }

    pub fn marker_map(&self) -> &MarkerMap {
        &self.map
    }

    pub fn publish_count(&self) -> u64 {
        self.sink.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pose::PoseQuality;

    #[test]
    fn start_tick_moves_and_publishes() {
        let mut eng = SimEngine::lab_default();
        eng.reset();
        assert!(!eng.is_running());
        eng.start();
        for _ in 0..20 {
            eng.tick();
        }
        let s = eng.state();
        assert!(s.running);
        assert!(s.time_s > 0.0);
        assert!(eng.publish_count() > 0);
        let est = s.estimated.unwrap();
        // Near origin after short run — should have seen marker 1.
        assert!(matches!(
            est.quality,
            PoseQuality::Good | PoseQuality::Degraded
        ));
    }

    #[test]
    fn reset_clears_pose() {
        let mut eng = SimEngine::lab_default();
        eng.start();
        for _ in 0..50 {
            eng.tick();
        }
        eng.reset();
        let s = eng.state();
        assert!(!s.running);
        assert!((s.true_x_m).abs() < 1e-9);
        assert!((s.true_y_m).abs() < 1e-9);
        assert!((s.true_theta_rad).abs() < 1e-9);
    }
}
