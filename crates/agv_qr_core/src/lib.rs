//! AGV QR floor-marker localization core (stub / simulation).
//!
//! Mirrors the design in `docs/agv-qr-floor-localization.md` at a lab/test level:
//! marker map store, pose output interface, vision stub, and a mock pose simulator.
//! No live camera required — Cloud/CI stays mock-first.

#![forbid(unsafe_code)]

pub mod marker_map;
pub mod pose;
pub mod sim;
pub mod vision;

pub use marker_map::{Marker, MarkerMap};
pub use pose::{PoseOutput, PoseQuality, PoseSample, PoseSink};
pub use sim::{SimConfig, SimEngine, SimState};
pub use vision::{DetectionQuality, StubVision, VisionPipeline};
