//! Root window: pose | estimate | marker map | detection | sim controls.

use std::time::Duration;

use agv_qr_core::{MarkerMap, SimEngine};
use gpui::{
    div, prelude::*, px, size, App, Application, Bounds, ClickEvent, Context, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SharedString, Styled, Timer, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};

use crate::strings;

mod theme {
    use gpui::{rgb, Rgba};

    pub fn bg() -> Rgba {
        rgb(0x1a_2332)
    }
    pub fn panel() -> Rgba {
        rgb(0x24_3044)
    }
    pub fn panel_alt() -> Rgba {
        rgb(0x2c_3b52)
    }
    pub fn border() -> Rgba {
        rgb(0x3d_4f66)
    }
    pub fn text() -> Rgba {
        rgb(0xe8_eef6)
    }
    pub fn muted() -> Rgba {
        rgb(0x9a_abbf)
    }
    pub fn accent() -> Rgba {
        rgb(0x3d_9b8f)
    }
    pub fn accent_dim() -> Rgba {
        rgb(0x2f_7a71)
    }
    pub fn warn() -> Rgba {
        rgb(0xc4_9a3c)
    }
}

struct RootView {
    engine: SimEngine,
}

impl RootView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title(strings::WINDOW_TITLE);

        cx.spawn(async move |this, cx| loop {
            Timer::after(Duration::from_millis(50)).await;
            let ok = this
                .update(cx, |this, cx| {
                    this.engine.tick();
                    cx.notify();
                })
                .is_ok();
            if !ok {
                break;
            }
        })
        .detach();

        let mut engine = SimEngine::lab_default();
        engine.reset();
        Self { engine }
    }

    fn on_start(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.engine.start();
        cx.notify();
    }

    fn on_stop(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.engine.stop();
        cx.notify();
    }

    fn on_reset(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.engine.reset();
        cx.notify();
    }

    fn panel(title: &'static str) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::accent())
                    .child(title),
            )
    }

    fn kv_row(label: &'static str, value: String) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .gap_3()
            .child(div().text_xs().text_color(theme::muted()).child(label))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::text())
                    .child(value),
            )
    }

    fn btn(
        label: &'static str,
        primary: bool,
        id: &'static str,
        on_click: impl Fn(&mut RootView, &ClickEvent, &mut Window, &mut Context<RootView>) + 'static,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let bg = if primary {
            theme::accent()
        } else {
            theme::panel_alt()
        };
        div()
            .id(SharedString::from(id))
            .px_3()
            .py_1p5()
            .rounded_md()
            .bg(bg)
            .border_1()
            .border_color(theme::border())
            .text_sm()
            .text_color(theme::text())
            .cursor_pointer()
            .hover(|s| s.bg(theme::accent_dim()))
            .on_click(cx.listener(move |this, ev, window, cx| {
                on_click(this, ev, window, cx);
            }))
            .child(label)
    }

    fn map_table(map: &MarkerMap) -> impl IntoElement {
        let header = div()
            .flex()
            .flex_row()
            .gap_2()
            .px_2()
            .py_1()
            .bg(theme::panel_alt())
            .rounded_sm()
            .text_xs()
            .text_color(theme::muted())
            .child(div().w(px(48.)).child(strings::COL_ID))
            .child(div().w(px(72.)).child(strings::COL_X))
            .child(div().w(px(72.)).child(strings::COL_Y))
            .child(div().w(px(72.)).child(strings::COL_YAW))
            .child(div().w(px(80.)).child(strings::COL_SIZE));

        let rows = map.iter().map(|(id, m)| {
            div()
                .flex()
                .flex_row()
                .gap_2()
                .px_2()
                .py_1()
                .border_b_1()
                .border_color(theme::border())
                .text_xs()
                .text_color(theme::text())
                .child(div().w(px(48.)).child(format!("{id}")))
                .child(div().w(px(72.)).child(format!("{:.2}", m.x_m)))
                .child(div().w(px(72.)).child(format!("{:.2}", m.y_m)))
                .child(
                    div()
                        .w(px(72.))
                        .child(format!("{:.1}", m.yaw_rad.to_degrees())),
                )
                .child(div().w(px(80.)).child(format!("{:.3}", m.size_m)))
        });

        div().flex().flex_col().gap_1().child(header).children(rows)
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.engine.state();
        let running = s.running;
        let run_label = if running {
            strings::STATUS_RUNNING
        } else {
            strings::STATUS_STOPPED
        };

        let est_x = s
            .estimated
            .as_ref()
            .map(|e| format!("{:.3} {}", e.x_m, strings::UNIT_M))
            .unwrap_or_else(|| strings::STATUS_NONE.into());
        let est_y = s
            .estimated
            .as_ref()
            .map(|e| format!("{:.3} {}", e.y_m, strings::UNIT_M))
            .unwrap_or_else(|| strings::STATUS_NONE.into());
        let est_th = s
            .estimated
            .as_ref()
            .map(|e| format!("{:.1} {}", e.heading_deg(), strings::UNIT_DEG))
            .unwrap_or_else(|| strings::STATUS_NONE.into());
        let est_q = s
            .estimated
            .as_ref()
            .map(|e| e.quality.as_str_de().to_string())
            .unwrap_or_else(|| strings::STATUS_NONE.into());

        let marker_str = s
            .detection
            .marker_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| strings::STATUS_NONE.into());
        let age_str = if s.detection.age_s.is_finite() {
            format!("{:.2} {}", s.detection.age_s, strings::UNIT_S)
        } else {
            strings::STATUS_NONE.into()
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::bg())
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .justify_between()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme::border())
                    .bg(theme::panel())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .child(strings::APP_HEADING),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::muted())
                                    .child(strings::APP_SUBHEADING),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::muted())
                            .child(strings::HINT_SIM),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .gap_3()
                    .p_3()
                    .child(
                        Self::panel(strings::SECTION_POSE)
                            .flex_1()
                            .child(Self::kv_row(
                                strings::LABEL_X,
                                format!("{:.3} {}", s.true_x_m, strings::UNIT_M),
                            ))
                            .child(Self::kv_row(
                                strings::LABEL_Y,
                                format!("{:.3} {}", s.true_y_m, strings::UNIT_M),
                            ))
                            .child(Self::kv_row(
                                strings::LABEL_THETA,
                                format!(
                                    "{:.1} {}",
                                    s.true_theta_rad.to_degrees(),
                                    strings::UNIT_DEG
                                ),
                            ))
                            .child(Self::kv_row(
                                strings::LABEL_TIME,
                                format!("{:.2} {}", s.time_s, strings::UNIT_S),
                            ))
                            .child(Self::kv_row(strings::LABEL_RUNNING, run_label.into())),
                    )
                    .child(
                        Self::panel(strings::SECTION_ESTIMATE)
                            .flex_1()
                            .child(Self::kv_row(strings::LABEL_X, est_x))
                            .child(Self::kv_row(strings::LABEL_Y, est_y))
                            .child(Self::kv_row(strings::LABEL_THETA, est_th))
                            .child(Self::kv_row(strings::LABEL_QUALITY, est_q)),
                    )
                    .child(
                        Self::panel(strings::SECTION_DETECT)
                            .flex_1()
                            .child(Self::kv_row(
                                strings::LABEL_QUALITY,
                                s.detection.label_de().into(),
                            ))
                            .child(Self::kv_row(
                                strings::LABEL_RMSE,
                                format!(
                                    "{:.2} {}",
                                    s.detection.reprojection_rmse_px,
                                    strings::UNIT_PX
                                ),
                            ))
                            .child(Self::kv_row(
                                strings::LABEL_CORNERS,
                                format!("{}", s.detection.corner_count),
                            ))
                            .child(Self::kv_row(strings::LABEL_MARKER, marker_str))
                            .child(Self::kv_row(strings::LABEL_AGE, age_str)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .px_3()
                    .pb_3()
                    .child(
                        Self::panel(strings::SECTION_MAP)
                            .flex_1()
                            .child(Self::kv_row(
                                strings::LABEL_MAP_VERSION,
                                s.map_version.clone(),
                            ))
                            .child(Self::kv_row(
                                strings::LABEL_MARKER_COUNT,
                                format!("{}", s.marker_count),
                            ))
                            .child(
                                div()
                                    .id("marker-map-scroll")
                                    .mt_2()
                                    .max_h(px(220.))
                                    .overflow_scroll()
                                    .child(Self::map_table(self.engine.marker_map())),
                            ),
                    )
                    .child(
                        Self::panel(strings::SECTION_CONTROLS)
                            .w(px(260.))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(Self::btn(
                                        strings::BTN_START,
                                        true,
                                        "btn-start",
                                        |this, e, w, cx| this.on_start(e, w, cx),
                                        cx,
                                    ))
                                    .child(Self::btn(
                                        strings::BTN_STOP,
                                        false,
                                        "btn-stop",
                                        |this, e, w, cx| this.on_stop(e, w, cx),
                                        cx,
                                    ))
                                    .child(Self::btn(
                                        strings::BTN_RESET,
                                        false,
                                        "btn-reset",
                                        |this, e, w, cx| this.on_reset(e, w, cx),
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .mt_2()
                                    .text_xs()
                                    .text_color(if running {
                                        theme::accent()
                                    } else {
                                        theme::warn()
                                    })
                                    .child(format!("{}: {}", strings::LABEL_RUNNING, run_label)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(theme::border())
                    .bg(theme::panel())
                    .text_xs()
                    .text_color(theme::muted())
                    .child(strings::FOOTER_BACKEND)
                    .child(format!("{}: {}", strings::LABEL_MAP_VERSION, s.map_version))
                    .child(format!("{}: {}", strings::LABEL_RUNNING, run_label)),
            )
    }
}

/// Launch the GPUI application (blocks).
pub fn run_gui() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from(strings::WINDOW_TITLE)),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(880.), px(560.))),
                app_id: Some("agv-qr-gui".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| RootView::new(window, cx)),
        )
        .expect("Fenster konnte nicht geöffnet werden");
        cx.activate(true);
    });
}
