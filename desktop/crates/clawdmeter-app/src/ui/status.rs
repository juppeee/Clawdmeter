//! The tray menu as a window, for desktops with no tray (GNOME without the
//! AppIndicator extension). Closing it quits the app.

use clawdmeter_core::{Paths, Settings};
use gpui::{div, prelude::*, px, Context, Window};

use super::theme::{Theme, FONT_MONO, R_PILL};
use super::widgets::{button, clawd, dot, para, toggle, ButtonKind, Tone};
use super::{chrome, command, live_status, not_running_text, REFRESH};
use crate::art::TrayState;
use crate::ipc::{AppCommand, StatusSnapshot};
use crate::{ago, tilde};

/// The status window.
pub struct StatusView {
    paths: Paths,
    snap: Option<StatusSnapshot>,
    beep_enabled: bool,
}

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
        let mut v = StatusView { paths, snap: None, beep_enabled: false };
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
    let frac = pct.unwrap_or(0).min(100) as f32 / 100.;
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().flex().justify_between().child(div().text_color(t.dim).child(label)).child(
            div().font_family(FONT_MONO).child(match (pct, note) {
                (_, Some(n)) => n,
                (Some(p), None) => format!("{p}%"),
                (None, None) => "–".into(),
            }),
        ))
        .child(
            div()
                .h(px(8.))
                .rounded(px(R_PILL))
                .bg(t.track)
                .child(div().h_full().w(gpui::relative(frac)).rounded(px(R_PILL)).bg(t.accent)),
        )
}

impl Render for StatusView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::for_appearance(window.appearance());
        let paths = self.paths.clone();
        let body = match &self.snap {
            None => {
                div().p(px(20.)).flex().flex_col().gap(px(16.)).child(clawd(3.)).child(para(&t, not_running_text()))
            }
            Some(s) => {
                let state = s.tray_state();
                let label = if s.link == "no_adapter" { "Bluetooth off" } else { state.label() };
                let account = match (&s.account, s.age(clawdmeter_core::unix_now())) {
                    (Some(a), Some(age)) => format!("{} · updated {}", tilde(a, &paths.home), ago(age)),
                    (Some(a), None) => tilde(a, &paths.home),
                    (None, _) if s.logged_out => "Run claude to log in again".into(),
                    (None, _) => "No data yet".into(),
                };
                let session_note = (s.session_pct.is_some() && !s.window_started).then(|| "not started".to_string());
                let muted = s.beep_muted;
                let update_label = match s.update_available() {
                    Some(v) => format!("Update firmware to {v}…"),
                    None => "Update firmware…".into(),
                };
                let (p1, p2, p3, p4, p5, p6) =
                    (paths.clone(), paths.clone(), paths.clone(), paths.clone(), paths.clone(), paths.clone());
                div()
                    .p(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div().flex().items_center().gap(px(14.)).child(clawd(3.)).child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(dot(&t, tone(state)))
                                        .child(div().text_size(px(15.)).child(format!("Clawdmeter · {label}"))),
                                )
                                .child(div().text_size(px(12.5)).text_color(t.mute).child(account)),
                        ),
                    )
                    .child(meter(&t, "Session", s.session_pct, session_note))
                    .child(meter(&t, "Weekly", s.week_pct, None))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_color(t.dim).child("Mute attention beep"))
                            .child(toggle(&t, "mute", muted, self.beep_enabled, move |_, _, _| {
                                command(&p1, AppCommand::Mute(!muted))
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(8.))
                            .child(button(&t, "refresh", "Refresh now", ButtonKind::Normal, true, move |_, _, _| {
                                command(&p2, AppCommand::Refresh)
                            }))
                            .child(button(&t, "update", update_label, ButtonKind::Normal, true, move |_, _, _| {
                                command(&p3, AppCommand::Open("flasher".into()))
                            }))
                            .child(button(
                                &t,
                                "flash",
                                "Flash a new device…",
                                ButtonKind::Normal,
                                true,
                                move |_, _, _| command(&p4, AppCommand::Open("flasher".into())),
                            ))
                            .child(button(&t, "settings", "Settings…", ButtonKind::Normal, true, move |_, _, _| {
                                command(&p5, AppCommand::Open("settings".into()))
                            })),
                    )
                    .child(div().flex().justify_end().child(button(
                        &t,
                        "quit",
                        "Quit (stops the daemon)",
                        ButtonKind::Small,
                        true,
                        move |_, window, _| {
                            command(&p6, AppCommand::Quit);
                            window.remove_window();
                        },
                    )))
            }
        };
        let _ = cx;
        chrome(&t, "Clawdmeter", window, body)
    }
}
