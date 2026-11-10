//! Root window: connection | groups | parameter list | active | status + confirm dialog.

use std::time::Duration;

use gpui::{
    div, prelude::*, px, size, App, Application, Bounds, ClickEvent, Context, FocusHandle,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SharedString, Styled,
    Timer, TitlebarOptions, Window, WindowBounds, WindowOptions,
};

use crate::session::AppSession;
use crate::state::{PendingConfirm, ShellState};
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
    pub fn danger() -> Rgba {
        rgb(0xc4_5c5c)
    }
    pub fn field_bg() -> Rgba {
        rgb(0x15_202e)
    }
    pub fn warn() -> Rgba {
        rgb(0xc4_9a3c)
    }
}

#[derive(Clone, PartialEq, Eq)]
enum FocusField {
    Host,
    Port,
    Unit,
    Filter,
    Draft(String),
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

    fn on_request_write(&mut self, param_id: String, cx: &mut Context<Self>) {
        if let Err(msg) = self.state.request_write(&param_id) {
            self.state.status_message = msg;
        }
        cx.notify();
    }

    fn on_confirm_pending(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let pending = self.state.pending.clone();
        match pending {
            Some(PendingConfirm::WriteSingle {
                param_id,
                display_value,
                ..
            }) => {
                self.session
                    .write_register(&mut self.state, &param_id, display_value, true);
            }
            Some(PendingConfirm::WriteGroup {
                write_group_id,
                values,
                ..
            }) => {
                self.session
                    .write_register_group(&mut self.state, &write_group_id, &values, true);
            }
            Some(PendingConfirm::ClearAllActive) => {
                self.session
                    .clear_all_active_setpoints(&mut self.state, true);
                self.state.pending = None;
            }
            None => {}
        }
        cx.notify();
    }

    fn on_cancel_pending(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.state.cancel_pending();
        cx.notify();
    }

    fn on_clear_active(&mut self, key: String, cx: &mut Context<Self>) {
        self.session.clear_active_setpoint(&mut self.state, &key);
        cx.notify();
    }

    fn on_clear_all_active(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.state.request_clear_all_active();
        cx.notify();
    }

    fn focused_text_mut(&mut self) -> Option<&mut String> {
        match &self.field_focus {
            Some(FocusField::Host) => Some(&mut self.state.connection.host),
            Some(FocusField::Port) => Some(&mut self.state.connection.port),
            Some(FocusField::Unit) => Some(&mut self.state.connection.unit_id),
            Some(FocusField::Filter) => Some(&mut self.state.filter_query),
            Some(FocusField::Draft(id)) => {
                let id = id.clone();
                self.state
                    .parameters
                    .iter_mut()
                    .find(|r| r.param_id == id)
                    .map(|r| &mut r.draft)
            }
            None => None,
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.field_focus.clone() else {
            return;
        };
        let key = event.keystroke.key.as_str();
        if key == "escape" {
            if self.state.pending.is_some() {
                self.state.cancel_pending();
                cx.notify();
            }
            return;
        }
        if key == "enter" {
            match &field {
                FocusField::Host | FocusField::Port | FocusField::Unit => {
                    self.session.connect(&mut self.state);
                }
                FocusField::Draft(id) => {
                    let id = id.clone();
                    let _ = self.on_request_write(id, cx);
                    return;
                }
                FocusField::Filter => {}
            }
            cx.notify();
            return;
        }
        if key == "backspace" {
            if let Some(t) = self.focused_text_mut() {
                t.pop();
                cx.notify();
            }
            return;
        }
        if key.chars().count() != 1 {
            return;
        }
        let ch = key.chars().next().unwrap();
        match &field {
            FocusField::Host => {
                if ch.is_ascii_graphic() || ch == '.' {
                    self.state.connection.host.push(ch);
                    cx.notify();
                }
            }
            FocusField::Port | FocusField::Unit => {
                if ch.is_ascii_digit() {
                    if let Some(t) = self.focused_text_mut() {
                        t.push(ch);
                        cx.notify();
                    }
                }
            }
            FocusField::Filter => {
                if !ch.is_control() {
                    self.state.filter_query.push(ch);
                    cx.notify();
                }
            }
            FocusField::Draft(_) => {
                if ch.is_ascii_digit() || ch == '.' || ch == '-' {
                    if let Some(t) = self.focused_text_mut() {
                        t.push(ch);
                        cx.notify();
                    }
                }
            }
        }
    }

    fn btn(
        id: impl Into<SharedString>,
        label: &'static str,
        enabled: bool,
        danger: bool,
        on_click: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = id.into();
        let bg = if !enabled {
            theme::panel_alt()
        } else if danger {
            theme::danger()
        } else {
            theme::accent()
        };
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(bg)
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

    fn text_field(
        &self,
        id: impl Into<SharedString>,
        label: &'static str,
        value: &str,
        which: FocusField,
        min_w: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.field_focus.as_ref() == Some(&which);
        let which_click = which.clone();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_xs().text_color(theme::muted()).child(label))
            .child(
                div()
                    .id(id.into())
                    .px_2()
                    .py_1()
                    .min_w(px(min_w))
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
                        this.field_focus = Some(which_click.clone());
                        if let FocusField::Draft(ref id) = which_click {
                            this.state.edit_focus = Some(id.clone());
                        } else {
                            this.state.edit_focus = None;
                        }
                        window.focus(&this.focus);
                        cx.notify();
                    })),
            )
    }

    fn confirm_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match &self.state.pending {
            Some(PendingConfirm::ClearAllActive) => strings::CONFIRM_CLEAR_ALL_TITLE,
            _ => strings::CONFIRM_TITLE,
        };
        let detail = self
            .state
            .pending_summary()
            .unwrap_or_else(|| strings::STATUS_NONE.to_string());

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000_88))
            .child(
                div()
                    .w(px(420.))
                    .p_4()
                    .rounded_md()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::panel())
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(div().text_sm().text_color(theme::muted()).child(detail))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .justify_end()
                            .child(Self::btn(
                                "dlg-cancel",
                                strings::BTN_CANCEL,
                                true,
                                false,
                                Self::on_cancel_pending,
                                cx,
                            ))
                            .child(Self::btn(
                                "dlg-ok",
                                strings::BTN_CONFIRM,
                                true,
                                true,
                                Self::on_confirm_pending,
                                cx,
                            )),
                    ),
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
        let filter = self.state.filter_query.clone();
        let rows: Vec<_> = self
            .state
            .filtered_parameters()
            .into_iter()
            .cloned()
            .collect();
        let active_entries = self.state.active_entries.clone();
        let active_empty = active_entries.is_empty();
        let show_dialog = self.state.pending.is_some();

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
            .relative()
            .child(
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
                            .child(self.text_field(
                                "host",
                                strings::LABEL_HOST,
                                &host,
                                FocusField::Host,
                                140.,
                                cx,
                            ))
                            .child(self.text_field(
                                "port",
                                strings::LABEL_PORT,
                                &port,
                                FocusField::Port,
                                70.,
                                cx,
                            ))
                            .child(self.text_field(
                                "unit",
                                strings::LABEL_UNIT,
                                &unit,
                                FocusField::Unit,
                                60.,
                                cx,
                            ))
                            .child(Self::btn(
                                "btn-connect",
                                strings::BTN_CONNECT,
                                !connected,
                                false,
                                Self::on_connect,
                                cx,
                            ))
                            .child(Self::btn(
                                "btn-disconnect",
                                strings::BTN_DISCONNECT,
                                connected,
                                false,
                                Self::on_disconnect,
                                cx,
                            ))
                            .child(Self::btn(
                                "btn-refresh",
                                strings::BTN_REFRESH,
                                connected,
                                false,
                                Self::on_refresh,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .child(
                        div()
                            .w(px(180.))
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
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .p_3()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_end()
                                    .justify_between()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
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
                                                    .child(format!(
                                                        "{}: {selected}",
                                                        strings::MAIN_GROUP_HINT
                                                    )),
                                            ),
                                    )
                                    .child(self.text_field(
                                        "filter",
                                        strings::FILTER_LABEL,
                                        &filter,
                                        FocusField::Filter,
                                        180.,
                                        cx,
                                    )),
                            )
                            .child(
                                // header
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(theme::muted())
                                    .border_b_1()
                                    .border_color(theme::border())
                                    .child(div().w(px(150.)).child(strings::COL_NAME))
                                    .child(div().w(px(48.)).child(strings::COL_ADDR))
                                    .child(div().w(px(52.)).child(strings::COL_TYPE))
                                    .child(div().w(px(44.)).child(strings::COL_SCALE))
                                    .child(div().w(px(48.)).child(strings::COL_UNIT))
                                    .child(div().w(px(56.)).child(strings::COL_RAW))
                                    .child(div().w(px(90.)).child(strings::COL_VALUE))
                                    .child(div().w(px(36.)).child(strings::COL_ACCESS))
                                    .child(div().w(px(64.)).child(strings::COL_QUALITY))
                                    .child(div().w(px(72.)).child(strings::COL_EDIT))
                                    .child(div().w(px(80.)).child("")),
                            )
                            .child(
                                div()
                                    .id("param-list")
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .overflow_scroll()
                                    .children(rows.into_iter().map(|row| {
                                        let id = row.param_id.clone();
                                        let id_write = id.clone();
                                        let q_color = match row.quality {
                                            Some(lambda_core::ValueQuality::Good) => {
                                                theme::accent()
                                            }
                                            Some(lambda_core::ValueQuality::Stale) => theme::warn(),
                                            Some(lambda_core::ValueQuality::Error) => {
                                                theme::danger()
                                            }
                                            None => theme::muted(),
                                        };
                                        div()
                                            .id(SharedString::from(format!("row-{id}")))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_2()
                                            .px_2()
                                            .py_1()
                                            .border_b_1()
                                            .border_color(theme::border())
                                            .text_xs()
                                            .child(
                                                div()
                                                    .w(px(150.))
                                                    .whitespace_nowrap()
                                                    .overflow_hidden()
                                                    .child(row.label_de.clone()),
                                            )
                                            .child(
                                                div().w(px(48.)).child(format!("{}", row.address)),
                                            )
                                            .child(div().w(px(52.)).child(row.type_label.clone()))
                                            .child(div().w(px(44.)).child(format!("{}", row.scale)))
                                            .child(div().w(px(48.)).child(row.unit.clone()))
                                            .child(div().w(px(56.)).child(match row.raw {
                                                Some(r) => r.to_string(),
                                                None => strings::STATUS_NONE.to_string(),
                                            }))
                                            .child(div().w(px(90.)).child(row.display_text()))
                                            .child(div().w(px(36.)).child(row.access_label()))
                                            .child(
                                                div()
                                                    .w(px(64.))
                                                    .text_color(q_color)
                                                    .child(row.quality_label()),
                                            )
                                            .child(div().w(px(72.)).when(row.writable, |d| {
                                                let draft = row.draft.clone();
                                                let focus = FocusField::Draft(id.clone());
                                                d.child(self.text_field(
                                                    SharedString::from(format!("draft-{id}")),
                                                    "",
                                                    &draft,
                                                    focus,
                                                    64.,
                                                    cx,
                                                ))
                                            }))
                                            .child(div().w(px(80.)).when(row.writable, |d| {
                                                d.child(
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "write-{id_write}"
                                                        )))
                                                        .px_2()
                                                        .py_0p5()
                                                        .rounded_sm()
                                                        .bg(if connected {
                                                            theme::accent()
                                                        } else {
                                                            theme::panel_alt()
                                                        })
                                                        .when(connected, |b| {
                                                            b.cursor_pointer().on_click(
                                                                cx.listener(
                                                                    move |this, _, _, cx| {
                                                                        this.on_request_write(
                                                                            id_write.clone(),
                                                                            cx,
                                                                        );
                                                                    },
                                                                ),
                                                            )
                                                        })
                                                        .child(strings::BTN_WRITE),
                                                )
                                            }))
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .w(px(240.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_3()
                            .border_l_1()
                            .border_color(theme::border())
                            .bg(theme::panel())
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::muted())
                                            .child(strings::ACTIVE_SECTION),
                                    )
                                    .child(Self::btn(
                                        "btn-clear-all",
                                        strings::BTN_CLEAR_ALL_ACTIVE,
                                        !active_empty,
                                        true,
                                        Self::on_clear_all_active,
                                        cx,
                                    )),
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
                            .child(
                                div()
                                    .id("active-list")
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .overflow_scroll()
                                    .children(active_entries.into_iter().map(|entry| {
                                        let key = entry.key.clone();
                                        div()
                                            .id(SharedString::from(format!("active-{key}")))
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(theme::panel_alt())
                                            .child(div().text_sm().child(format!(
                                                "{} = {}",
                                                entry.label, entry.display_value
                                            )))
                                            .when_some(entry.last_error.clone(), |d, err| {
                                                d.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme::danger())
                                                        .child(err),
                                                )
                                            })
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!("clear-{key}")))
                                                    .text_xs()
                                                    .text_color(theme::accent())
                                                    .cursor_pointer()
                                                    .child(strings::BTN_CLEAR_ACTIVE)
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.on_clear_active(key.clone(), cx);
                                                        },
                                                    )),
                                            )
                                    })),
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
            .when(show_dialog, |d| d.child(self.confirm_overlay(cx)))
    }
}

/// Launch the GPUI application (blocks).
pub fn run_gui() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from(strings::WINDOW_TITLE)),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(960.), px(640.))),
                app_id: Some("lambda-modbus-gui".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| RootView::new(window, cx)),
        )
        .expect("Fenster konnte nicht geöffnet werden");
        cx.activate(true);
    });
}
