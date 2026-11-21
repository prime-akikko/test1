//! AGV QR localization GPUI test binary.
//!
//! Default Cloud/CI: headless sim smoke, no GPUI link.
//! Windows GUI: `cargo run -p agv_qr_app --features gui`

#![forbid(unsafe_code)]

fn main() {
    #[cfg(feature = "gui")]
    {
        agv_qr_app::ui::run_gui();
    }

    #[cfg(not(feature = "gui"))]
    {
        agv_qr_app::run_headless_smoke();
    }
}
