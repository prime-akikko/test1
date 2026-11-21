//! Root window: connection | groups | parameter list | active | status + confirm dialog.
//!
//! Keyboard: Tab / Shift-Tab cycle [`FocusTarget`]s; write-confirm dialog traps focus.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    actions, div, prelude::*, px, size, App, Application, Bounds, ClickEvent, Context, FocusHandle,
    FontWeight, InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, ParentElement,
    SharedString, Styled, Timer, TitlebarOptions, Window, WindowBounds, WindowOptions,
};

use crate::focus::{self, FocusTarget};
use crate::persist::{self, BootstrappedApp};
use crate::session::AppSession;
use crate::state::{PendingConfirm, ShellState};
use crate::strings;

actions!(lambda_shell, [FocusNext, FocusPrev]);

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
    /// High-contrast keyboard focus ring.
    pub fn focus_ring() -> Rgba {
        rgb(0x7e_c8_ff)
    }
}

struct RootView {
    state: ShellState,
    session: AppSession,
    data_dir: PathBuf,
    focus: FocusHandle,
    ui_focus: Option<FocusTarget>,
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

        let boot = BootstrappedApp::load_default(cfg!(feature = "mock"));
        Self {
            state: boot.state,
            session: boot.session,
            data_dir: boot.data_dir,
            focus,
            ui_focus: None,
        }
    }

    fn persist_config(&mut self) {
        if let Err(err) = persist::save_connection_settings(&self.data_dir, &self.state) {
            self.state.status_message = format!("{}: {err}", strings::STATUS_ERROR_PREFIX);
        }
    }

    fn persist_prefs(&mut self) {
        if let Err(err) = persist::save_ui_prefs(&self.data_dir, &self.state) {
            self.state.status_message = format!("{}: {err}", strings::STATUS_ERROR_PREFIX);
        }
    }

    fn set_focus(
        &mut self,
        target: Option<FocusTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ui_focus = target;
        self.state.edit_focus = self
            .ui_focus
            .as_ref()
            .and_then(|t| t.draft_param_id().map(str::to_string));
        window.focus(&self.focus);
        cx.notify();
    }

    fn move_focus(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let order = focus::focus_order(&self.state);
        let next = focus::cycle_focus(self.ui_focus.as_ref(), &order, forward);
        self.set_focus(next, window, cx);
    }

    fn on_focus_next(&mut self, _: &FocusNext, window: &mut Window, cx: &mut Context<Self>) {
        self.move_focus(true, window, cx);
    }

    fn on_focus_prev(&mut self, _: &FocusPrev, window: &mut Window, cx: &mut Context<Self>) {
        self.move_focus(false, window, cx);
    }

    fn on_select_group(&mut self, group: String, cx: &mut Context<Self>) {
        self.state.select_group(group);
        self.persist_prefs();
        self.session.refresh_selected_group(&mut self.state);
        // Drop stale draft/write focus from the previous group.
        if matches!(
            self.ui_focus,
            Some(FocusTarget::Draft(_)) | Some(FocusTarget::Write(_))
        ) {
            self.ui_focus = None;
            self.state.edit_focus = None;
        }
        cx.notify();
    }

    fn on_request_write(&mut self, param_id: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(msg) = self.state.request_write(&param_id) {
            self.state.status_message = msg;
        } else {
            self.set_focus(Some(FocusTarget::DialogCancel), window, cx);
            return;
        }
        cx.notify();
    }

    fn on_confirm_pending(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
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
        if self.state.pending.is_none() {
            self.set_focus(None, window, cx);
        } else {
            cx.notify();
        }
    }

    fn on_clear_active(&mut self, key: String, cx: &mut Context<Self>) {
        self.session.clear_active_setpoint(&mut self.state, &key);
        cx.notify();
    }

    fn focused_text_mut(&mut self) -> Option<&mut String> {
        match &self.ui_focus {
            Some(FocusTarget::Host) => Some(&mut self.state.connection.host),
            Some(FocusTarget::Port) => Some(&mut self.state.connection.port),
            Some(FocusTarget::Unit) => Some(&mut self.state.connection.unit_id),
            Some(FocusTarget::Filter) => Some(&mut self.state.filter_query),
            Some(FocusTarget::Draft(id)) => {
                let id = id.clone();
                self.state
                    .parameters
                    .iter_mut()
                    .find(|r| r.param_id == id)
                    .map(|r| &mut r.draft)
            }
            _ => None,
        }
    }

    fn activate_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.ui_focus.clone() else {
            return;
        };
        let connected = self.state.is_connected();
        match target {
            FocusTarget::Host | FocusTarget::Port | FocusTarget::Unit => {
                self.persist_config();
                self.session.connect(&mut self.state);
                cx.notify();
            }
            FocusTarget::Connect if !connected => {
                self.persist_config();
                self.session.connect(&mut self.state);
                cx.notify();
            }
            FocusTarget::Disconnect if connected => {
                self.session.disconnect(&mut self.state);
                cx.notify();
            }
            FocusTarget::Refresh if connected => {
                self.session.refresh(&mut self.state);
                cx.notify();
            }
            FocusTarget::Group(group) => {
                self.on_select_group(group, cx);
            }
            FocusTarget::Draft(id) => {
                self.on_request_write(id, window, cx);
            }
            FocusTarget::Write(id) if connected => {
                self.on_request_write(id, window, cx);
            }
            FocusTarget::ClearAllActive if !self.state.active_entries.is_empty() => {
                self.state.request_clear_all_active();
                if self.state.pending.is_some() {
                    self.set_focus(Some(FocusTarget::DialogCancel), window, cx);
                } else {
                    cx.notify();
                }
            }
            FocusTarget::ClearActive(key) => {
                self.on_clear_active(key, cx);
            }
            FocusTarget::DialogCancel => {
                self.state.cancel_pending();
                self.set_focus(None, window, cx);
            }
            FocusTarget::DialogConfirm => {
                self.on_confirm_pending(&ClickEvent::default(), window, cx);
            }
            _ => {}
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();

        // Tab / Shift-Tab are handled via KeyBinding → FocusNext / FocusPrev actions
        // (see run_gui). Do not also handle them here or focus would move twice.
        if key == "tab" {
            return;
        }

        if key == "escape" {
            if self.state.pending.is_some() {
                self.state.cancel_pending();
                self.set_focus(None, window, cx);
            }
            return;
        }

        if key == "enter" {
            self.activate_focused(window, cx);
            return;
        }

        let Some(field) = self.ui_focus.clone() else {
            return;
        };
        if !field.is_text_field() {
            return;
        }

        if key == "backspace" {
            let is_filter = matches!(field, FocusTarget::Filter);
            if let Some(t) = self.focused_text_mut() {
                t.pop();
                if is_filter {
                    self.persist_prefs();
                }
                cx.notify();
            }
            return;
        }
        if key.chars().count() != 1 {
            return;
        }
        let ch = key.chars().next().unwrap();
        match &field {
            FocusTarget::Host => {
                if ch.is_ascii_graphic() || ch == '.' {
                    self.state.connection.host.push(ch);
                    cx.notify();
                }
            }
            FocusTarget::Port | FocusTarget::Unit => {
                if ch.is_ascii_digit() {
                    if let Some(t) = self.focused_text_mut() {
                        t.push(ch);
                        cx.notify();
                    }
                }
            }
            FocusTarget::Filter => {
                if !ch.is_control() {
                    self.state.filter_query.push(ch);
                    self.persist_prefs();
                    cx.notify();
                }
            }
            FocusTarget::Draft(_) => {
                if ch.is_ascii_digit() || ch == '.' || ch == '-' {
                    if let Some(t) = self.focused_text_mut() {
                        t.push(ch);
                        cx.notify();
                    }
                }
            }
            _ => {}
        }
    }

    fn is_focused(&self, target: &FocusTarget) -> bool {
        self.ui_focus.as_ref() == Some(target)
    }

    fn text_field(
        &self,
        id: impl Into<SharedString>,
        label: &'static str,
        value: &str,
        which: FocusTarget,
        min_w: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.is_focused(&which);
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
                        theme::focus_ring()
                    } else {
                        theme::border()
                    })
                    .when(active, |d| d.border_2().border_color(theme::focus_ring()))
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
                        this.set_focus(Some(which_click.clone()), window, cx);
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
        let cancel_focused = self.is_focused(&FocusTarget::DialogCancel);
        let confirm_focused = self.is_focused(&FocusTarget::DialogConfirm);

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000_88))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
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
                            .child(dialog_btn(
                                "dlg-cancel",
                                strings::BTN_CANCEL,
                                false,
                                cancel_focused,
                                FocusTarget::DialogCancel,
                                cx,
                            ))
                            .child(dialog_btn(
                                "dlg-ok",
                                strings::BTN_CONFIRM,
                                true,
                                confirm_focused,
                                FocusTarget::DialogConfirm,
                                cx,
                            )),
                    ),
            )
    }
}

fn focusable_btn(
    id: impl Into<SharedString>,
    label: &'static str,
    enabled: bool,
    danger: bool,
    focused: bool,
    target: FocusTarget,
    on_click: impl Fn(&mut RootView, &ClickEvent, &mut Window, &mut Context<RootView>) + 'static,
    cx: &mut Context<RootView>,
) -> impl IntoElement {
    let id = id.into();
    let bg = if !enabled {
        theme::panel_alt()
    } else if danger {
        theme::danger()
    } else {
        theme::accent()
    };
    let target_focus = target.clone();
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(if focused {
            theme::focus_ring()
        } else {
            theme::border()
        })
        .when(focused, |d| d.border_2().border_color(theme::focus_ring()))
        .bg(bg)
        .text_color(theme::text())
        .text_sm()
        .on_click(cx.listener(move |this, ev, window, cx| {
            this.set_focus(Some(target_focus.clone()), window, cx);
            if enabled {
                on_click(this, ev, window, cx);
            }
        }))
        .when(enabled, |d| {
            d.cursor_pointer().hover(|d| d.bg(theme::accent_dim()))
        })
        .when(!enabled, |d| d.opacity(0.45))
        .child(label)
}

fn dialog_btn(
    id: impl Into<SharedString>,
    label: &'static str,
    danger: bool,
    focused: bool,
    target: FocusTarget,
    cx: &mut Context<RootView>,
) -> impl IntoElement {
    let target_click = target.clone();
    focusable_btn(
        id,
        label,
        true,
        danger,
        focused,
        target,
        move |this, _, window, cx| match target_click {
            FocusTarget::DialogCancel => {
                this.state.cancel_pending();
                this.set_focus(None, window, cx);
            }
            FocusTarget::DialogConfirm => {
                this.on_confirm_pending(&ClickEvent::default(), window, cx);
            }
            _ => {}
        },
        cx,
    )
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep dialog focus trapped if pending just appeared via non-keyboard path.
        if self.state.pending.is_some() {
            let on_dialog = self.ui_focus.as_ref().is_some_and(|t| t.is_dialog());
            if !on_dialog {
                self.ui_focus = Some(FocusTarget::DialogCancel);
                self.state.edit_focus = None;
                window.focus(&self.focus);
            }
        } else if self.ui_focus.as_ref().is_some_and(|t| t.is_dialog()) {
            self.ui_focus = None;
        }

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

        let host_focus = FocusTarget::Host;
        let port_focus = FocusTarget::Port;
        let unit_focus = FocusTarget::Unit;
        let filter_focus = FocusTarget::Filter;

        div()
            .id("root")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::on_focus_next))
            .on_action(cx.listener(Self::on_focus_prev))
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
                                host_focus,
                                140.,
                                cx,
                            ))
                            .child(self.text_field(
                                "port",
                                strings::LABEL_PORT,
                                &port,
                                port_focus,
                                70.,
                                cx,
                            ))
                            .child(self.text_field(
                                "unit",
                                strings::LABEL_UNIT,
                                &unit,
                                unit_focus,
                                60.,
                                cx,
                            ))
                            .child(focusable_btn(
                                "btn-connect",
                                strings::BTN_CONNECT,
                                !connected,
                                false,
                                self.is_focused(&FocusTarget::Connect),
                                FocusTarget::Connect,
                                |this, _, _, cx| {
                                    this.persist_config();
                                    this.session.connect(&mut this.state);
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(focusable_btn(
                                "btn-disconnect",
                                strings::BTN_DISCONNECT,
                                connected,
                                false,
                                self.is_focused(&FocusTarget::Disconnect),
                                FocusTarget::Disconnect,
                                |this, _, _, cx| {
                                    this.session.disconnect(&mut this.state);
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(focusable_btn(
                                "btn-refresh",
                                strings::BTN_REFRESH,
                                connected,
                                false,
                                self.is_focused(&FocusTarget::Refresh),
                                FocusTarget::Refresh,
                                |this, _, _, cx| {
                                    this.session.refresh(&mut this.state);
                                    cx.notify();
                                },
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
                                let target = FocusTarget::Group(group.clone());
                                let focused = self.is_focused(&target);
                                div()
                                    .id(SharedString::from(format!("grp-{group}")))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(if focused {
                                        theme::focus_ring()
                                    } else {
                                        theme::border()
                                    })
                                    .when(focused, |d| {
                                        d.border_2().border_color(theme::focus_ring())
                                    })
                                    .bg(if is_sel {
                                        theme::accent_dim()
                                    } else {
                                        theme::panel()
                                    })
                                    .hover(|d| d.bg(theme::panel_alt()))
                                    .text_sm()
                                    .child(group)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.set_focus(
                                            Some(FocusTarget::Group(g2.clone())),
                                            window,
                                            cx,
                                        );
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
                                        filter_focus,
                                        180.,
                                        cx,
                                    )),
                            )
                            .child(
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
                                        let draft_target = FocusTarget::Draft(id.clone());
                                        let write_target = FocusTarget::Write(id_write.clone());
                                        let write_focused = self.is_focused(&write_target);
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
                                                d.child(self.text_field(
                                                    SharedString::from(format!("draft-{id}")),
                                                    "",
                                                    &draft,
                                                    draft_target,
                                                    64.,
                                                    cx,
                                                ))
                                            }))
                                            .child(div().w(px(80.)).when(row.writable, |d| {
                                                let id_write2 = id_write.clone();
                                                d.child(
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "write-{id_write}"
                                                        )))
                                                        .px_2()
                                                        .py_0p5()
                                                        .rounded_sm()
                                                        .border_1()
                                                        .border_color(if write_focused {
                                                            theme::focus_ring()
                                                        } else {
                                                            theme::border()
                                                        })
                                                        .when(write_focused, |b| {
                                                            b.border_2()
                                                                .border_color(theme::focus_ring())
                                                        })
                                                        .bg(if connected {
                                                            theme::accent()
                                                        } else {
                                                            theme::panel_alt()
                                                        })
                                                        .cursor_pointer()
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.set_focus(
                                                                    Some(FocusTarget::Write(
                                                                        id_write2.clone(),
                                                                    )),
                                                                    window,
                                                                    cx,
                                                                );
                                                                if this.state.is_connected() {
                                                                    this.on_request_write(
                                                                        id_write2.clone(),
                                                                        window,
                                                                        cx,
                                                                    );
                                                                }
                                                            },
                                                        ))
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
                                    .child(focusable_btn(
                                        "btn-clear-all",
                                        strings::BTN_CLEAR_ALL_ACTIVE,
                                        !active_empty,
                                        true,
                                        self.is_focused(&FocusTarget::ClearAllActive),
                                        FocusTarget::ClearAllActive,
                                        |this, _, window, cx| {
                                            this.state.request_clear_all_active();
                                            if this.state.pending.is_some() {
                                                this.set_focus(
                                                    Some(FocusTarget::DialogCancel),
                                                    window,
                                                    cx,
                                                );
                                            } else {
                                                cx.notify();
                                            }
                                        },
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
                                        let target = FocusTarget::ClearActive(key.clone());
                                        let focused = self.is_focused(&target);
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
                                                    .border_1()
                                                    .border_color(if focused {
                                                        theme::focus_ring()
                                                    } else {
                                                        theme::border()
                                                    })
                                                    .when(focused, |d| {
                                                        d.border_2()
                                                            .border_color(theme::focus_ring())
                                                    })
                                                    .rounded_sm()
                                                    .px_1()
                                                    .child(strings::BTN_CLEAR_ACTIVE)
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.set_focus(
                                                                Some(FocusTarget::ClearActive(
                                                                    key.clone(),
                                                                )),
                                                                window,
                                                                cx,
                                                            );
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
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrev, None),
        ]);

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
