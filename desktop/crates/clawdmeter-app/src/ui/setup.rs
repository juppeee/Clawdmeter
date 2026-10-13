//! The first-run setup wizard: Welcome → Your board → Flash → Pair → Hooks
//! → Done. It fills the window, every step can be skipped, and finishing or
//! skipping it records `onboarding_done` ([`crate::onboarding`]). The board
//! and flash steps are the flasher; the hooks step is the Hooks page.

use clawdmeter_core::Paths;
use gpui::{div, prelude::*, px, Context, Entity, EventEmitter, SharedString, Window};

use super::flasher::{FlashEvent, FlashMode, FlasherView, Step as FlashStep};
use super::settings::SettingsView;
use super::theme::{Theme, FONT_DISPLAY, R_PILL};
use super::widgets::{button, card, clawd, dot, notice, para, pill, ButtonKind, Tone};
use super::{live_status, REFRESH};
use crate::Page;

/// The wizard's steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// What Clawdmeter is.
    Welcome,
    /// The flasher: plug in and identify, then flash.
    Device,
    /// Bluetooth pairing.
    Pair,
    /// Claude Code hooks.
    Hooks,
    /// Summary.
    Done,
}

const LABELS: [&str; 6] = ["Welcome", "Your board", "Flash", "Pair", "Hooks", "Done"];

/// What happened to the board in this run of the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardOutcome {
    Flashed,
    UpToDate,
    Skipped,
}

/// The wizard tells the window it's done (finished or skipped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupEvent {
    /// Show the app's pages.
    Finished,
}

/// The setup wizard.
pub struct SetupView {
    paths: Paths,
    step: Step,
    flasher: Entity<FlasherView>,
    settings: Entity<SettingsView>,
    board: Option<BoardOutcome>,
    connected: bool,
    running: bool,
}

impl EventEmitter<SetupEvent> for SetupView {}

impl SetupView {
    /// Build around the window's flasher and settings views.
    pub fn new(
        paths: Paths,
        flasher: Entity<FlasherView>,
        settings: Entity<SettingsView>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&flasher, |v, _, e: &FlashEvent, cx| {
            if v.step == Step::Device {
                v.board = Some(match e {
                    FlashEvent::Flashed => BoardOutcome::Flashed,
                    FlashEvent::UpToDate => BoardOutcome::UpToDate,
                });
                v.goto(Step::Pair, cx);
            }
        })
        .detach();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(REFRESH).await;
            if this.update(cx, |v, cx| v.tick(cx)).is_err() {
                break;
            }
        })
        .detach();
        SetupView { paths, step: Step::Welcome, flasher, settings, board: None, connected: false, running: false }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let s = live_status(&self.paths);
        let (running, connected) = (s.is_some(), s.is_some_and(|s| s.link == "connected"));
        if (running, connected) != (self.running, self.connected) {
            self.running = running;
            self.connected = connected;
            cx.notify();
        }
    }

    /// Start from the welcome step.
    pub fn restart(&mut self, cx: &mut Context<Self>) {
        self.board = None;
        self.flasher.update(cx, |f, cx| {
            f.restart();
            f.set_mode(FlashMode::Setup, cx);
        });
        self.goto(Step::Welcome, cx);
    }

    /// The window is leaving the wizard for a page: put the borrowed views
    /// back and count setup as done.
    pub fn leave(&mut self, cx: &mut Context<Self>) {
        self.flasher.update(cx, |f, cx| {
            f.set_mode(FlashMode::Page, cx);
            f.set_active(false, cx);
        });
        if let Err(e) = crate::onboarding::mark_done(&self.paths) {
            log::warn!("couldn't record that setup is done: {e}");
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        self.leave(cx);
        cx.emit(SetupEvent::Finished);
    }

    /// Show a step.
    pub fn goto(&mut self, step: Step, cx: &mut Context<Self>) {
        self.step = step;
        self.flasher.update(cx, |f, cx| f.set_active(step == Step::Device, cx));
        // The Hooks page shows inside the hooks step; otherwise the settings
        // view stays hidden (and off USB).
        let page = if step == Step::Hooks { Page::Hooks } else { Page::Setup };
        self.settings.update(cx, |s, cx| s.show(page, true, cx));
        self.tick(cx);
        cx.notify();
    }

    fn index(&self, cx: &Context<Self>) -> usize {
        match self.step {
            Step::Welcome => 0,
            Step::Device => match self.flasher.read(cx).step() {
                FlashStep::Connect | FlashStep::Identify => 1,
                _ => 2,
            },
            Step::Pair => 3,
            Step::Hooks => 4,
            Step::Done => 5,
        }
    }

    fn progress(&self, t: &Theme, cx: &Context<Self>) -> impl IntoElement {
        let at = self.index(cx);
        let mut row = div().flex().items_center().justify_center().gap(px(4.)).flex_wrap();
        for (i, label) in LABELS.iter().enumerate() {
            let (done, on) = (i < at, i == at);
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(R_PILL))
                    .when(on, |d| d.bg(t.panel))
                    .text_size(px(12.5))
                    .text_color(if on || done { t.text } else { t.mute })
                    .child(div().size(px(7.)).rounded(px(R_PILL)).bg(if on {
                        t.accent
                    } else if done {
                        t.ok
                    } else {
                        t.track
                    }))
                    .child(*label),
            );
        }
        row
    }

    fn title(text: &'static str) -> impl IntoElement {
        div().font_family(FONT_DISPLAY).text_size(px(26.)).text_center().child(text)
    }

    fn welcome(&self, t: &Theme) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.))
            .pt(px(24.))
            .child(clawd(7.))
            .child(Self::title("Meet Clawdmeter"))
            .child(div().max_w(px(460.)).text_center().child(para(
                t,
                "A small screen for your desk that shows your Claude Code usage. Clawd lives on it, reacts while \
                 you work, and comes to find you when a session is waiting on you.",
            )))
            .child(div().max_w(px(460.)).text_center().child(para(
                t,
                "Setup takes a few minutes: put the firmware on your board, pair it over Bluetooth, and let \
                 Claude Code tell Clawd what's going on.",
            )))
    }

    fn pair(&self, t: &Theme) -> gpui::Div {
        let how = if cfg!(target_os = "macos") {
            "Open System Settings → Bluetooth and click Connect next to Clawdmeter."
        } else if cfg!(windows) {
            "Open Settings → Bluetooth & devices → Add device → Bluetooth, and choose Clawdmeter."
        } else {
            "Open your desktop's Bluetooth settings and pair Clawdmeter. From a terminal: bluetoothctl, then \
             scan on, pair <address> and trust <address>."
        };
        let status = match (self.running, self.connected) {
            (_, true) => pill(t, "Connected", Tone::Ok).into_any_element(),
            (true, false) => pill(t, "Waiting for Clawdmeter to connect", Tone::Accent).into_any_element(),
            (false, false) => notice(
                t,
                Tone::Plain,
                "Clawdmeter's background part isn't running, so this step can't see the connection. Pair it \
                 anyway; the app finds it once it runs.",
            )
            .w(px(520.))
            .into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.))
            .pt(px(12.))
            .child(Self::title("Pair it over Bluetooth"))
            .child(div().max_w(px(500.)).text_center().child(para(
                t,
                "Your computer sees the device as a Bluetooth keyboard; that's also what makes its buttons work. \
                 Pair it once and the app finds it on its own after that.",
            )))
            .child(card(t).w(px(520.)).child(div().text_size(px(14.)).line_height(px(21.)).child(how)).child(
                div().text_size(px(12.5)).text_color(t.mute).line_height(px(18.)).child(
                    "Paired with another computer before? Hold its PWR button for 3 seconds and let go to make \
                         it pairable again.",
                ),
            ))
            .child(status)
    }

    fn hooks(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let (pending, installed) = {
            let s = self.settings.read(cx);
            (s.pending_hooks(), s.installed_hooks())
        };
        let mut d =
            div().flex().flex_col().gap(px(16.)).pt(px(12.)).child(Self::title("Let Clawd follow along")).child(
                div().flex().justify_center().child(div().max_w(px(520.)).text_center().child(para(
                    t,
                    "Claude Code hooks tell Clawd when you send a prompt and when a session is waiting on you. They \
                 record only the event type and session ID.",
                ))),
            );
        if pending == 0 && installed > 0 {
            let accounts =
                if installed == 1 { "your account".to_string() } else { format!("all {installed} accounts") };
            d = d.child(div().flex().justify_center().child(pill(
                t,
                SharedString::from(format!("Already set up in {accounts}")),
                Tone::Ok,
            )));
        } else {
            d = d.child(self.settings.clone());
        }
        d
    }

    fn done(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let board = match self.board {
            Some(BoardOutcome::Flashed) => ("Board flashed with the latest firmware", Tone::Ok),
            Some(BoardOutcome::UpToDate) => ("Board already on the latest firmware", Tone::Ok),
            Some(BoardOutcome::Skipped) | None => ("Board not flashed (the Flash page does it any time)", Tone::Plain),
        };
        let link = if self.connected {
            ("Connected over Bluetooth", Tone::Ok)
        } else {
            ("Not connected yet (pair it whenever you're ready)", Tone::Plain)
        };
        let installed = self.settings.read(cx).installed_hooks();
        let hooks = if installed > 0 {
            ("Claude Code hooks set up", Tone::Ok)
        } else {
            ("No hooks yet (the Hooks page adds them)", Tone::Plain)
        };
        let row = |(text, tone): (&'static str, Tone)| {
            div().flex().items_center().gap(px(10.)).child(dot(t, tone)).child(div().text_size(px(14.)).child(text))
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.))
            .pt(px(24.))
            .child(clawd(6.))
            .child(Self::title("You're all set"))
            .child(card(t).min_w(px(380.)).gap(px(12.)).child(row(board)).child(row(link)).child(row(hooks)))
            .child(div().max_w(px(460.)).text_center().child(para(
                t,
                "Clawdmeter keeps running in the tray. Open this window from there to see your usage, update \
                 the firmware or change settings.",
            )))
    }
}

impl Render for SetupView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::for_appearance(window.appearance());
        let body = match self.step {
            Step::Welcome => self.welcome(&t),
            Step::Device => div().child(self.flasher.clone()),
            Step::Pair => self.pair(&t),
            Step::Hooks => self.hooks(&t, cx),
            Step::Done => self.done(&t, cx),
        };
        let back = match self.step {
            Step::Welcome => None,
            Step::Device => Some(Step::Welcome),
            Step::Pair => Some(Step::Device),
            Step::Hooks => Some(Step::Pair),
            Step::Done => Some(Step::Hooks),
        };
        // Nothing walks away from a board mid-write.
        let busy = self.step == Step::Device && self.flasher.read(cx).flashing();
        let mut right = div().flex().items_center().gap(px(8.));
        if let Some(b) = back {
            right = right.child(button(
                &t,
                "setup-back",
                "Back",
                ButtonKind::Small,
                !busy,
                cx.listener(move |v, _, _, cx| {
                    // On the board steps, Back walks the flasher back first.
                    if v.step == Step::Device
                        && v.flasher.update(cx, |f, cx| {
                            let moved = f.back();
                            cx.notify();
                            moved
                        })
                    {
                        cx.notify();
                        return;
                    }
                    if b == Step::Device {
                        v.flasher.update(cx, |f, _| f.restart());
                    }
                    v.goto(b, cx);
                }),
            ));
        }
        right = match self.step {
            Step::Welcome => right.child(button(
                &t,
                "setup-next",
                "Get started",
                ButtonKind::Primary,
                true,
                cx.listener(|v, _, _, cx| v.goto(Step::Device, cx)),
            )),
            // The flasher has the step's primary action.
            Step::Device => right.child(button(
                &t,
                "setup-next",
                "Skip this step",
                ButtonKind::Normal,
                !busy,
                cx.listener(|v, _, _, cx| {
                    v.board = Some(BoardOutcome::Skipped);
                    v.goto(Step::Pair, cx);
                }),
            )),
            Step::Pair | Step::Hooks => {
                let next = if self.step == Step::Pair { Step::Hooks } else { Step::Done };
                // While hooks wait to be installed, installing is the step's
                // primary action and moving on is the quiet one.
                let pending = self.step == Step::Hooks && self.settings.read(cx).pending_hooks() > 0;
                let (label, kind) =
                    if pending { ("Not now", ButtonKind::Normal) } else { ("Next", ButtonKind::Primary) };
                right.child(button(
                    &t,
                    "setup-next",
                    label,
                    kind,
                    true,
                    cx.listener(move |v, _, _, cx| v.goto(next, cx)),
                ))
            }
            Step::Done => right.child(button(
                &t,
                "setup-finish",
                "Finish",
                ButtonKind::Primary,
                true,
                cx.listener(|v, _, _, cx| v.finish(cx)),
            )),
        };
        let footer = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .h(px(64.))
            .px(px(24.))
            .border_t_1()
            .border_color(t.line)
            .bg(t.bg)
            .child(
                div()
                    .id("skip-setup")
                    .debug_selector(|| "skip-setup".into())
                    .text_size(px(13.))
                    .text_color(t.dim)
                    .cursor_pointer()
                    .hover(|s| s.text_color(t.text))
                    .when(self.step != Step::Done && !busy, |d| {
                        d.child("Skip setup").on_click(cx.listener(|v, _, _, cx| v.finish(cx)))
                    }),
            )
            .child(right);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_shrink_0().pt(px(18.)).pb(px(6.)).child(self.progress(&t, cx)))
            .child(div().id("setup-body").flex_1().min_h_0().overflow_y_scroll().child(
                div().flex().justify_center().child(div().w_full().max_w(px(680.)).px(px(32.)).py(px(20.)).child(body)),
            ))
            .child(footer)
    }
}
