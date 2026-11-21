//! AGV QR floor localization test GUI (simulation).
//!
//! - Always available: German strings, headless sim smoke.
//! - Feature `gui`: GPUI window ([`ui`]).

#![forbid(unsafe_code)]

pub mod strings;

#[cfg(feature = "gui")]
pub mod ui;

use agv_qr_core::SimEngine;

/// Run a short headless simulation smoke (Cloud/CI — no camera, no GPUI).
pub fn run_headless_smoke() {
    println!("{}", strings::WINDOW_TITLE);
    println!("{}", strings::APP_SUBHEADING);

    let mut eng = SimEngine::lab_default();
    eng.reset();
    eng.start();
    for _ in 0..40 {
        eng.tick();
    }
    let s = eng.state();
    println!("Karte: {} ({} Marker)", s.map_version, s.marker_count);
    println!(
        "Wahr-Pose: x={:.3} m  y={:.3} m  θ={:.1}°  t={:.2} s",
        s.true_x_m,
        s.true_y_m,
        s.true_theta_rad.to_degrees(),
        s.time_s
    );
    if let Some(est) = &s.estimated {
        println!(
            "Schätzung: x={:.3} m  y={:.3} m  θ={:.1}°  Qualität={}",
            est.x_m,
            est.y_m,
            est.heading_deg(),
            est.quality.as_str_de()
        );
        println!(
            "Erkennung: {}  RMSE={:.2} px  Ecken={}  Marker={:?}",
            s.detection.label_de(),
            s.detection.reprojection_rmse_px,
            s.detection.corner_count,
            s.detection.marker_id
        );
    }
    eng.stop();
    println!("{}", strings::HEADLESS_DONE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_smoke_runs() {
        run_headless_smoke();
    }
}
