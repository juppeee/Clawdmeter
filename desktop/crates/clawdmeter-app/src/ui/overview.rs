//! Overview: what the tray menu shows, as a page. The link, the live
//! meters, a refresh button, and a nudge when new firmware is out.

use clawdmeter_core::Paths;
use gpui::{div, prelude::*, px, Context, EventEmitter, SharedString, Window};

use super::theme::{Theme, FONT_DISPLAY, FONT_MONO};
use super::widgets::{bar, button, card, clawd, dot, icon_button, notice, tone_color, usage_tone, ButtonKind, Tone};
use super::{command, live_status, not_running_text, Navigate, REFRESH};
use crate::art::TrayState;
use crate::ipc::{AppCommand, StatusSnapshot};
use crate::{ago, tilde, Page};

/// The overview page.
pub struct OverviewView {
    paths: Paths,
    snap: Option<StatusSnapshot>,
    /// When the refresh button was pressed, until that check has run.
    refreshing: Option<i64>,
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
        let mut v = OverviewView { paths, snap: None, refreshing: None };
        v.reload(clawdmeter_core::unix_now());
        v
    }

    fn reload(&mut self, now: i64) {
        self.snap = live_status(&self.paths);
        if let Some(at) = self.refreshing {
            let done = match &self.snap {
                None => true,
                // The check ran (fresh numbers or a new error) or never will.
                Some(s) => {
                    s.read_at.is_some_and(|r| r >= at)
                        || (!s.polling && now - at > REFRESH_SETTLE_SECS)
                        || now - at > REFRESH_GIVE_UP_SECS
                }
            };
            if done {
                self.refreshing = None;
            }
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let before = self.snap.clone();
        self.reload(clawdmeter_core::unix_now());
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

/// After a refresh press, how long to wait for the poll to start.
const REFRESH_SETTLE_SECS: i64 = 3;
/// A `/usage` poll times out well before this.
const REFRESH_GIVE_UP_SECS: i64 = 60;

/// "1 min 40 s" for a wait.
fn wait_text(secs: i64) -> String {
    match secs.max(1) {
        s if s < 60 => format!("{s} s"),
        s if s < 3600 && s % 60 == 0 => format!("{} min", s / 60),
        s if s < 3600 => format!("{} min {} s", s / 60, s % 60),
        s => format!("{} h {} min", s / 3600, (s % 3600) / 60),
    }
}

/// What the refresh button can do right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshState {
    /// A check of the shown account is running.
    Checking,
    /// The account was checked too recently, or is backing off after a
    /// rate limit: a press would only queue a check for then.
    Wait(i64),
    /// A press checks right away.
    Ready,
}

/// The refresh button's state for `s`; `pressed` is a press not yet served.
pub fn refresh_state(s: &StatusSnapshot, pressed: bool, now: i64) -> RefreshState {
    if s.polling || pressed {
        return RefreshState::Checking;
    }
    match s.refresh_at {
        Some(t) if t > now => RefreshState::Wait(t - now),
        _ => RefreshState::Ready,
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
        ("no_adapter", _) => "Bluetooth unavailable",
        (_, TrayState::Connected) => "Connected",
        (_, TrayState::Stale) => "Usage out of date",
        (_, TrayState::Searching) => "Looking for Clawdmeter",
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
            None => (Tone::Plain, "Not running", "Start Clawdmeter to update usage.".to_string()),
            Some(s) => {
                let sub = match (&s.account, s.age(now)) {
                    _ if s.logged_out => "Open Claude Code in a terminal and sign in.".into(),
                    (Some(a), Some(age)) => format!("{} · updated {}", tilde(a, &home), ago(age)),
                    (Some(a), None) => tilde(a, &home),
                    (None, _) => "No usage data yet".into(),
                };
                (state_tone(s.tray_state()), state_title(s), sub)
            }
        };
        let refresh = self.snap.as_ref().map(|s| self.refresh_control(&t, s, now, cx));
        let header = div()
            .flex()
            .items_start()
            .gap(px(20.))
            .child(clawd(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
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
            )
            .children(refresh);
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
                notice(&t, Tone::Accent, format!("Firmware v{v} is available. Connect USB to update.")).child(
                    div().flex().pl(px(18.)).child(button(
                        &t,
                        "go-update",
                        "Update firmware",
                        ButtonKind::Primary,
                        true,
                        cx.listener(|_, _, _, cx| cx.emit(Navigate(Page::Flash))),
                    )),
                ),
            );
        }
        page
    }
}

impl OverviewView {
    /// The refresh icon at the top right, with a quiet note when a press
    /// can't check right away.
    fn refresh_control(&self, t: &Theme, s: &StatusSnapshot, now: i64, cx: &mut Context<Self>) -> impl IntoElement {
        let state = refresh_state(s, self.refreshing.is_some(), now);
        let note: Option<SharedString> = match &state {
            RefreshState::Checking => Some("Checking…".into()),
            RefreshState::Wait(secs) => Some(format!("Can check again in {}", wait_text(*secs)).into()),
            RefreshState::Ready => None,
        };
        let checking = state == RefreshState::Checking;
        let button =
            icon_button(t, "refresh", "refresh-cw", "Refresh usage", state == RefreshState::Ready, checking, {
                cx.listener(|v, _, _, cx| {
                    command(&v.paths, AppCommand::Refresh);
                    v.refreshing = Some(clawdmeter_core::unix_now());
                    cx.notify();
                })
            });
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .when_some(note, |d, n| d.child(div().text_size(px(12.5)).text_color(t.mute).child(n)))
            .child(button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_follows_the_gap_and_backoff() {
        let s = StatusSnapshot { refresh_at: Some(1_100), ..Default::default() };
        assert_eq!(refresh_state(&s, false, 1_000), RefreshState::Wait(100));
        assert_eq!(refresh_state(&s, false, 1_100), RefreshState::Ready);
        assert_eq!(refresh_state(&s, true, 1_000), RefreshState::Checking);
        let polling = StatusSnapshot { polling: true, ..Default::default() };
        assert_eq!(refresh_state(&polling, false, 1_000), RefreshState::Checking);
        assert_eq!(refresh_state(&StatusSnapshot::default(), false, 0), RefreshState::Ready);
    }

    #[test]
    fn waits_read_naturally() {
        assert_eq!(wait_text(0), "1 s");
        assert_eq!(wait_text(45), "45 s");
        assert_eq!(wait_text(100), "1 min 40 s");
        assert_eq!(wait_text(600), "10 min");
        assert_eq!(wait_text(3_900), "1 h 5 min");
    }
}
