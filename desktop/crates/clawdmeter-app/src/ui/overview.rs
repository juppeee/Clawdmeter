//! Overview: what the tray menu shows, as a page. The link, the live
//! meters, refresh and mute, and a nudge when new firmware is out.

use clawdmeter_core::{Paths, Settings};
use gpui::{div, prelude::*, px, Context, EventEmitter, Window};

use super::theme::{Theme, FONT_DISPLAY, FONT_MONO};
use super::widgets::{bar, button, card, clawd, dot, field, notice, toggle, tone_color, usage_tone, ButtonKind, Tone};
use super::{command, live_status, not_running_text, Navigate, REFRESH};
use crate::art::TrayState;
use crate::ipc::{AppCommand, StatusSnapshot};
use crate::{ago, tilde, Page};

/// The overview page.
pub struct OverviewView {
    paths: Paths,
    snap: Option<StatusSnapshot>,
    beep_enabled: bool,
}

impl EventEmitter<Navigate> for OverviewView {}

impl OverviewView {
    /// Build and start following the tray process's status.
    pub fn new(paths: Paths, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(REFRESH).await;
            if this.update(cx, |v, cx| v.refresh(cx)).is_err() {
                break;
            }
        })
        .detach();
        let mut v = OverviewView { paths, snap: None, beep_enabled: false };
        v.reload();
        v
    }

    fn reload(&mut self) {
        self.snap = live_status(&self.paths);
        self.beep_enabled = Settings::load(&self.paths).map(|s| s.beep).unwrap_or(false);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let before = self.snap.clone();
        self.reload();
        // "updated N s ago" ticks, so redraw while running.
        if self.snap.is_some() || before.is_some() {
            cx.notify();
        }
    }

    /// New firmware is out for the connected device.
    pub fn update_available(&self) -> bool {
        self.snap.as_ref().is_some_and(|s| s.update_available().is_some())
    }
}

/// The dot colour for a tray state.
pub fn state_tone(state: TrayState) -> Tone {
    match state {
        TrayState::Connected => Tone::Ok,
        TrayState::Stale => Tone::Warn,
        TrayState::Searching => Tone::Plain,
        TrayState::AuthError => Tone::Bad,
        TrayState::Attention => Tone::Accent,
    }
}

/// One headline for the link and data state.
pub fn state_title(s: &StatusSnapshot) -> &'static str {
    match (s.link.as_str(), s.tray_state()) {
        ("no_adapter", _) => "Bluetooth is off",
        (_, TrayState::Connected) => "Connected",
        (_, TrayState::Stale) => "Waiting for fresh numbers",
        (_, TrayState::Searching) => "Looking for your Clawdmeter",
        (_, TrayState::AuthError) => "Signed out of Claude Code",
        (_, TrayState::Attention) => "Claude needs you",
    }
}

fn meter(t: &Theme, label: &'static str, pct: Option<u32>, note: Option<&'static str>) -> impl IntoElement {
    let value = match (pct, note) {
        (_, Some(n)) => n.to_owned(),
        (Some(p), None) => format!("{p}%"),
        (None, None) => "–".into(),
    };
    let color = match (pct, note) {
        (Some(p), None) => tone_color(t, usage_tone(p)),
        _ => t.track,
    };
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(div().text_size(px(14.)).text_color(t.dim).child(label))
                .child(div().font_family(FONT_MONO).text_size(px(20.)).text_color(t.text).child(value)),
        )
        .child(bar(t, pct.unwrap_or(0).min(100) as f32 / 100., color))
}

impl Render for OverviewView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::for_appearance(window.appearance());
        let now = clawdmeter_core::unix_now();
        let home = self.paths.home.clone();
        let (tone, title, sub) = match &self.snap {
            None => (Tone::Plain, "Not running", "No live usage until Clawdmeter starts.".to_string()),
            Some(s) => {
                let sub = match (&s.account, s.age(now)) {
                    _ if s.logged_out => "Run claude in a terminal to sign in again.".into(),
                    (Some(a), Some(age)) => format!("{} · updated {}", tilde(a, &home), ago(age)),
                    (Some(a), None) => tilde(a, &home),
                    (None, _) => "No usage yet".into(),
                };
                (state_tone(s.tray_state()), state_title(s), sub)
            }
        };
        let header = div().flex().items_center().gap(px(20.)).child(clawd(4.)).child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(dot(&t, tone))
                        .child(div().font_family(FONT_DISPLAY).text_size(px(24.)).child(title)),
                )
                .child(div().text_size(px(13.)).text_color(t.mute).truncate().child(sub)),
        );
        let mut page = div().flex().flex_col().gap(px(20.)).child(header);
        let Some(s) = self.snap.clone() else {
            return page.child(notice(&t, Tone::Plain, not_running_text()));
        };

        let session_note = (s.session_pct.is_some() && !s.window_started).then_some("Not started");
        page = page.child(
            card(&t)
                .p(px(20.))
                .gap(px(20.))
                .child(meter(&t, "Session", s.session_pct, session_note))
                .child(meter(&t, "Weekly", s.week_pct, None)),
        );
        if let Some(v) = s.update_available() {
            let v = v.trim_start_matches('v').to_owned();
            page = page.child(
                notice(&t, Tone::Accent, format!("Firmware v{v} is out. Updating takes about a minute over USB."))
                    .child(div().flex().pl(px(18.)).child(button(
                        &t,
                        "go-update",
                        "Update firmware",
                        ButtonKind::Primary,
                        true,
                        cx.listener(|_, _, _, cx| cx.emit(Navigate(Page::Flash))),
                    ))),
            );
        }
        let (p1, p2) = (self.paths.clone(), self.paths.clone());
        let muted = s.beep_muted;
        let beep_help = if self.beep_enabled {
            "Silences the attention beep until Clawdmeter restarts."
        } else {
            "The beep is off on the Device page, so there's nothing to mute."
        };
        page.child(
            div()
                .flex()
                .flex_col()
                .child(field(
                    &t,
                    "Refresh now",
                    "Reads usage right away instead of waiting for the next check.",
                    button(&t, "refresh", "Refresh", ButtonKind::Small, true, move |_, _, _| {
                        command(&p1, AppCommand::Refresh)
                    }),
                ))
                .child(field(
                    &t,
                    "Mute the attention beep",
                    beep_help,
                    toggle(&t, "mute", muted, self.beep_enabled, move |_, _, _| command(&p2, AppCommand::Mute(!muted))),
                )),
        )
    }
}
