//! Root window layout: connection | groups | main | active | status (German).

use std::time::Duration;

use gpui::{
    div, prelude::*, px, size, App, Application, Bounds, ClickEvent, Context, FocusHandle,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SharedString, Styled,
    Timer, TitlebarOptions, Window, WindowBounds, WindowOptions,
};

use crate::session::AppSession;
use crate::state::ShellState;
use crate::strings;

/// Visual theme — industrial cool slate (avoid purple / cream AI defaults).
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
    pub fn danger() -> Rgba {
        rgb(0xc4_5c5c)
    }
    pub fn field_bg() -> Rgba {
        rgb(0x15_202e)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusField {
    Host,
    Port,
    Unit,
}

struct RootView {
    state: ShellState,
    session: AppSession,
    focus: FocusHandle,
    field_focus: Option<FocusField>,
}

impl RootView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        window.set_window_title(strings::WINDOW_TITLE);

        // Poll timer pump — core interval is 5s; tick often enough to notice due times.
        cx.spawn(async move |this, cx| loop {
            Timer::after(Duration::from_millis(250)).await;
            let ok = this
                .update(cx, |this, cx| {
                    this.session.tick(&mut this.state);
                    cx.notify();
                })
                .is_ok();
            if !ok {
                break;
            }
        })
        .detach();

        Self {
            state: ShellState::new(cfg!(feature = "mock")),
            session: AppSession::from_shell_defaults(),
            focus,
            field_focus: None,
        }
    }

    fn on_connect(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.session.connect(&mut self.state);
        cx.notify();
    }

    fn on_disconnect(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.session.disconnect(&mut self.state);
        cx.notify();
    }

    fn on_refresh(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.session.refresh(&mut self.state);
        cx.notify();
    }

    fn on_select_group(&mut self, group: String, cx: &mut Context<Self>) {
        self.state.select_group(group);
        self.session.refresh_selected_group(&mut self.state);
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.field_focus else {
            return;
        };
        let target = match field {
            FocusField::Host => &mut self.state.connection.host,
            FocusField::Port => &mut self.state.connection.port,
            FocusField::Unit => &mut self.state.connection.unit_id,
        };
        let key = event.keystroke.key.as_str();
        if key == "backspace" {
            target.pop();
            cx.notify();
            return;
        }
        if key == "enter" {
            self.session.connect(&mut self.state);
            cx.notify();
            return;
        }
        // Single printable character (ignore modifiers-only / named keys).
        if key.chars().count() == 1 {
            let ch = key.chars().next().unwrap();
            if field == FocusField::Host {
                if ch.is_ascii_graphic() || ch == '.' {
                    target.push(ch);
                    cx.notify();
                }
            } else if ch.is_ascii_digit() {
                target.push(ch);
                cx.notify();
            }
        }
    }

    fn btn(
        id: impl Into<SharedString>,
        label: &'static str,
        enabled: bool,
        on_click: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = id.into();
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(if enabled {
                theme::accent()
            } else {
                theme::panel_alt()
            })
            .text_color(theme::text())
            .text_sm()
            .when(enabled, |d| {
                d.cursor_pointer()
                    .hover(|d| d.bg(theme::accent_dim()))
                    .on_click(cx.listener(on_click))
            })
            .when(!enabled, |d| d.opacity(0.45))
            .child(label)
    }

    fn field(
        &self,
        id: &'static str,
        label: &'static str,
        value: &str,
        which: FocusField,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.field_focus == Some(which);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_xs().text_color(theme::muted()).child(label))
            .child(
                div()
                    .id(id)
                    .px_2()
                    .py_1()
                    .min_w(px(120.))
                    .rounded_sm()
                    .border_1()
                    .border_color(if active {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(theme::field_bg())
                    .text_color(theme::text())
                    .text_sm()
                    .cursor_text()
                    .child(if value.is_empty() {
                        SharedString::from(" ")
                    } else {
                        SharedString::from(value.to_string())
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.field_focus = Some(which);
                        window.focus(&this.focus);
                        cx.notify();
                    })),
            )
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.state.is_connected();
        let groups = self.state.groups.clone();
        let selected = self.state.selected_group.clone();
        let status_label = self.state.status_label();
        let poll_text = self.state.poll_status_text();
        let cyclic_text = self.state.cyclic_status_text();
        let message = self.state.status_message.clone();
        let host = self.state.connection.host.clone();
        let port = self.state.connection.port.clone();
        let unit = self.state.connection.unit_id.clone();
        let backend = self.state.backend_label();
        let active_empty = self.state.active_labels.is_empty();

        div()
            .id("root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .text_color(theme::text())
            .font_family("Segoe UI")
            .child(
                // Header / brand
                div()
                    .flex()
                    .flex_col()
                    .px_4()
                    .py_3()
                    .bg(theme::panel())
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
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
                // Connection bar
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .bg(theme::panel_alt())
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::muted())
                            .child(strings::CONNECTION_SECTION),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_end()
                            .gap_3()
                            .child(self.field(
                                "host",
                                strings::LABEL_HOST,
                                &host,
                                FocusField::Host,
                                cx,
                            ))
                            .child(self.field(
                                "port",
                                strings::LABEL_PORT,
                                &port,
                                FocusField::Port,
                                cx,
                            ))
                            .child(self.field(
                                "unit",
                                strings::LABEL_UNIT,
                                &unit,
                                FocusField::Unit,
                                cx,
                            ))
                            .child(Self::btn(
                                "btn-connect",
                                strings::BTN_CONNECT,
                                !connected,
                                Self::on_connect,
                                cx,
                            ))
                            .child(Self::btn(
                                "btn-disconnect",
                                strings::BTN_DISCONNECT,
                                connected,
                                Self::on_disconnect,
                                cx,
                            ))
                            .child(Self::btn(
                                "btn-refresh",
                                strings::BTN_REFRESH,
                                connected,
                                Self::on_refresh,
                                cx,
                            )),
                    ),
            )
            .child(
                // Body: groups | main | active
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .child(
                        // Group navigation
                        div()
                            .w(px(200.))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_3()
                            .border_r_1()
                            .border_color(theme::border())
                            .bg(theme::panel())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::muted())
                                    .mb_2()
                                    .child(strings::GROUPS_SECTION),
                            )
                            .children(groups.into_iter().map(|group| {
                                let is_sel = group == selected;
                                let g2 = group.clone();
                                div()
                                    .id(SharedString::from(format!("grp-{group}")))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .bg(if is_sel {
                                        theme::accent_dim()
                                    } else {
                                        theme::panel()
                                    })
                                    .hover(|d| d.bg(theme::panel_alt()))
                                    .text_sm()
                                    .child(group)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.on_select_group(g2.clone(), cx);
                                    }))
                            })),
                    )
                    .child(
                        // Main parameter placeholder
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .p_4()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme::muted())
                                    .child(strings::MAIN_SECTION),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("{}: {selected}", strings::MAIN_GROUP_HINT)),
                            )
                            .child(
                                div()
                                    .mt_4()
                                    .p_4()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(theme::border())
                                    .bg(theme::panel())
                                    .text_color(theme::muted())
                                    .child(strings::MAIN_PLACEHOLDER),
                            ),
                    )
                    .child(
                        // Active setpoints panel
                        div()
                            .w(px(260.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_3()
                            .border_l_1()
                            .border_color(theme::border())
                            .bg(theme::panel())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::muted())
                                    .child(strings::ACTIVE_SECTION),
                            )
                            .when(active_empty, |d| {
                                d.child(
                                    div()
                                        .text_sm()
                                        .text_color(theme::muted())
                                        .child(strings::ACTIVE_EMPTY),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme::muted())
                                        .child(strings::ACTIVE_HINT),
                                )
                            })
                            .children(self.state.active_labels.iter().cloned().map(|label| {
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(theme::panel_alt())
                                    .text_sm()
                                    .child(label)
                            })),
                    ),
            )
            .child(
                // Status bar
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
                    .child(format!("{}: {status_label}", strings::STATUS_CONNECTION))
                    .child(format!("{}: {poll_text}", strings::STATUS_POLL))
                    .child(format!("{}: {cyclic_text}", strings::STATUS_CYCLIC))
                    .child(backend)
                    .child(
                        div()
                            .flex_1()
                            .text_color(if connected {
                                theme::accent()
                            } else {
                                theme::danger()
                            })
                            .child(message),
                    ),
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
                window_min_size: Some(size(px(800.), px(560.))),
                app_id: Some("lambda-modbus-gui".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| RootView::new(window, cx)),
        )
        .expect("Fenster konnte nicht geöffnet werden");
        cx.activate(true);
    });
}
