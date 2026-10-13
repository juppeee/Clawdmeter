//! The tray menu as a window, for desktops with no tray (GNOME without the
//! AppIndicator extension). Closing it quits the app.

use clawdmeter_core::{Paths, Settings};
use gpui::{div, prelude::*, px, size, Context, SharedString, Window};

use super::theme::{Theme, FONT_MONO};
use super::widgets::{bar, clawd, dot, menu_row, toggle, tone_color, usage_tone, Tone};
use super::{chrome, command, live_status, REFRESH};
use crate::art::TrayState;
use crate::ipc::{AppCommand, StatusSnapshot};
use crate::{ago, tilde};

/// The status window.
pub struct StatusView {
    paths: Paths,
    snap: Option<StatusSnapshot>,
    beep_enabled: bool,
    /// Height the window was last fitted to, so it only resizes on a change.
    fitted_h: f32,
}

/// Window heights that fit the content: the header alone, or header + meters + menu.
pub(super) const HEIGHT_NOT_RUNNING: f32 = 180.;
pub(super) const HEIGHT_RUNNING: f32 = 440.;

impl StatusView {
    /// Build and start following the tray process's status.
    pub fn new(paths: Paths, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(REFRESH).await;
            if this.update(cx, |v, cx| v.refresh(cx)).is_err() {
                break;
            }
        })
        .detach();
        let mut v = StatusView { paths, snap: None, beep_enabled: false, fitted_h: 0. };
        v.reload();
        v
    }

    fn reload(&mut self) {
        self.snap = live_status(&self.paths);
        self.beep_enabled = Settings::load(&self.paths).map(|s| s.beep).unwrap_or(false);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.reload();
        cx.notify();
    }
}

fn tone(state: TrayState) -> Tone {
    match state {
        TrayState::Connected => Tone::Ok,
        TrayState::Stale => Tone::Warn,
        TrayState::Searching => Tone::Plain,
        TrayState::AuthError => Tone::Bad,
        TrayState::Attention => Tone::Accent,
    }
}

fn meter(t: &Theme, label: &'static str, pct: Option<u32>, note: Option<String>) -> impl IntoElement {
    let value = match (pct, &note) {
        (_, Some(n)) => n.clone(),
        (Some(p), None) => format!("{p}%"),
        (None, None) => "–".into(),
    };
    let color = match (pct, &note) {
        (Some(p), None) => tone_color(t, usage_tone(p)),
        _ => t.track,
    };
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(div().text_size(px(13.)).text_color(t.dim).child(label))
                .child(div().font_family(FONT_MONO).text_size(px(13.)).text_color(t.text).child(value)),
        )
        .child(bar(t, pct.unwrap_or(0).min(100) as f32 / 100., color))
}

fn divider(t: &Theme) -> impl IntoElement {
    div().h(px(1.)).w_full().bg(t.line)
}

impl Render for StatusView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fit = if self.snap.is_some() { HEIGHT_RUNNING } else { HEIGHT_NOT_RUNNING };
        if fit != self.fitted_h {
            self.fitted_h = fit;
            window.resize(size(window.bounds().size.width, px(fit)));
        }
        let t = Theme::for_appearance(window.appearance());
        let paths = self.paths.clone();
        let now = clawdmeter_core::unix_now();
        let (state, title, sub): (Option<TrayState>, String, String) = match &self.snap {
            None => (None, "Not running".into(), "No live usage until Clawdmeter starts.".into()),
            Some(s) => {
                let state = s.tray_state();
                let title = match (s.link.as_str(), state) {
                    ("no_adapter", _) => "Bluetooth is off".to_string(),
                    (_, TrayState::Connected) => "Connected".into(),
                    (_, TrayState::Stale) => "Waiting for fresh numbers".into(),
                    (_, TrayState::Searching) => "Looking for your Clawdmeter".into(),
                    (_, TrayState::AuthError) => "Signed out of Claude Code".into(),
                    (_, TrayState::Attention) => "Claude needs you".into(),
                };
                let sub = match (&s.account, s.age(now)) {
                    _ if s.logged_out => "Run claude in a terminal to sign in again.".into(),
                    (Some(a), Some(age)) => format!("{} · updated {}", tilde(a, &paths.home), ago(age)),
                    (Some(a), None) => tilde(a, &paths.home),
                    (None, _) => "No usage yet".into(),
                };
                (Some(state), title, sub)
            }
        };
        let header = div().flex().items_center().gap(px(16.)).child(clawd(3.)).child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(dot(&t, state.map_or(Tone::Plain, tone)))
                        .child(div().text_size(px(15.)).child(title)),
                )
                .child(div().text_size(px(12.5)).text_color(t.mute).truncate().child(sub)),
        );
        let mut body = div().size_full().flex().flex_col().px(px(16.)).pt(px(20.)).pb(px(10.)).gap(px(16.));
        body = body.child(div().px(px(4.)).child(header));
        let Some(s) = self.snap.clone() else {
            let (p1, p2) = (paths.clone(), paths.clone());
            let body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .child(divider(&t))
                    .child(div().pt(px(6.)).child(menu_row(
                        &t,
                        "flash",
                        "Flash a new device…",
                        None,
                        true,
                        move |_, _, _| command(&p1, AppCommand::Open("flasher".into())),
                    )))
                    .child(menu_row(&t, "settings", "Settings…", None, true, move |_, _, _| {
                        command(&p2, AppCommand::Open("settings".into()))
                    })),
            );
            return chrome(&t, "Clawdmeter", window, body);
        };
        let session_note = (s.session_pct.is_some() && !s.window_started).then(|| "Not started".to_string());
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .px(px(4.))
                .child(meter(&t, "Session", s.session_pct, session_note))
                .child(meter(&t, "Weekly", s.week_pct, None)),
        );
        let muted = s.beep_muted;
        let update_label = match s.update_available() {
            Some(v) => format!("Update firmware to v{}…", v.trim_start_matches('v')),
            None => "Update firmware…".into(),
        };
        let (p1, p2, p3, p4, p5, p6) =
            (paths.clone(), paths.clone(), paths.clone(), paths.clone(), paths.clone(), paths.clone());
        let beep_hint: SharedString = if self.beep_enabled { "".into() } else { "Beep is off".into() };
        let menu = div()
            .flex()
            .flex_col()
            .child(divider(&t))
            .child(div().py(px(6.)).child(menu_row(&t, "refresh", "Refresh now", None, true, move |_, _, _| {
                command(&p1, AppCommand::Refresh)
            })))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(34.))
                    .px(px(10.))
                    .text_size(px(14.))
                    .text_color(if self.beep_enabled { t.text } else { t.mute })
                    .child("Mute attention beep")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .when(!self.beep_enabled, |d| {
                                d.child(div().text_size(px(12.5)).text_color(t.mute).child(beep_hint))
                            })
                            .child(toggle(&t, "mute", muted, self.beep_enabled, move |_, _, _| {
                                command(&p2, AppCommand::Mute(!muted))
                            })),
                    ),
            )
            .child(div().pt(px(6.)).pb(px(6.)).child(divider(&t)))
            .child(menu_row(&t, "update", update_label, None, true, move |_, _, _| {
                command(&p3, AppCommand::Open("flasher".into()))
            }))
            .child(menu_row(&t, "flash", "Flash a new device…", None, true, move |_, _, _| {
                command(&p4, AppCommand::Open("flasher".into()))
            }))
            .child(menu_row(&t, "settings", "Settings…", None, true, move |_, _, _| {
                command(&p5, AppCommand::Open("settings".into()))
            }))
            .child(div().py(px(6.)).child(divider(&t)))
            .child(menu_row(&t, "quit", "Quit", Some("Stops the daemon".into()), true, move |_, window, _| {
                command(&p6, AppCommand::Quit);
                window.remove_window();
            }));
        let _ = cx;
        chrome(&t, "Clawdmeter", window, body.child(menu))
    }
}
