//! The GPUI windows. Each runs in its own `clawdmeter --window <kind>`
//! process (see [`crate::ipc`]) and exits when its window closes.

pub mod flasher;
pub mod settings;
pub mod status;
pub mod theme;
pub mod widgets;

use std::borrow::Cow;
use std::time::Duration;

use clawdmeter_core::{unix_now, Paths};
use gpui::{
    div, prelude::*, px, size, App, Application, Bounds, Decorations, MouseButton, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowDecorations, WindowOptions,
};

use crate::ipc::{self, AppCommand, StatusSnapshot};
use crate::WindowKind;
use theme::{Theme, FONT_BODY};

/// The three faces the firmware fonts are generated from (spec §01).
const FONTS: [&[u8]; 3] = [
    include_bytes!("../../../../../assets/TiemposText-400-Regular.otf"),
    include_bytes!("../../../../../assets/StyreneB-Regular.otf"),
    include_bytes!("../../../../../assets/DejaVuSansMono.ttf"),
];

/// How often windows re-read the tray process's status.
pub const REFRESH: Duration = Duration::from_millis(500);

/// Open one window and run until it closes.
/// `tab` picks the settings tab to open on (`device`, `accounts`, `hooks`,
/// `alerts`, `background`).
pub fn run(kind: WindowKind, paths: Paths, tab: Option<String>) -> i32 {
    Application::new().run(move |cx: &mut App| {
        let tab = tab.clone();
        if let Err(e) = cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()) {
            log::warn!("bundled fonts not registered ({e}); using system fonts");
        }
        // macOS keeps apps alive with no windows; this one is the window.
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let (title, w, h) = match kind {
            WindowKind::Settings => ("Clawdmeter settings", 780., 600.),
            WindowKind::Flasher => ("Flash your Clawdmeter", 620., 560.),
            WindowKind::Status => ("Clawdmeter", 380., 470.),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(w), px(h)), cx))),
            titlebar: Some(TitlebarOptions { title: Some(title.into()), ..Default::default() }),
            app_id: Some("clawdmeter".into()),
            is_resizable: true,
            window_min_size: Some(size(px(w * 0.8), px(h * 0.8))),
            window_decorations: Some(WindowDecorations::Server),
            ..Default::default()
        };
        let p = paths.clone();
        let opened = match kind {
            WindowKind::Settings => cx
                .open_window(options, move |w, cx| cx.new(|cx| settings::SettingsView::new(p, tab.as_deref(), w, cx)))
                .map(|_| ()),
            WindowKind::Flasher => {
                cx.open_window(options, move |w, cx| cx.new(|cx| flasher::FlasherView::new(p, w, cx))).map(|_| ())
            }
            WindowKind::Status => {
                cx.open_window(options, move |w, cx| cx.new(|cx| status::StatusView::new(p, w, cx))).map(|_| ())
            }
        };
        if let Err(e) = opened {
            log::error!("couldn't open the window: {e}");
            cx.quit();
        }
        cx.activate(true);
    });
    0
}

/// The window frame: the system's title bar when the compositor draws one,
/// otherwise (GNOME on Wayland) a minimal one of our own with a close button.
pub fn chrome(t: &Theme, title: &'static str, window: &Window, body: impl IntoElement) -> impl IntoElement {
    let client = matches!(window.window_decorations(), Decorations::Client { .. });
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(t.surface)
        .text_color(t.text)
        .font_family(FONT_BODY)
        .text_size(px(14.))
        .when(client, |d| {
            d.child(
                div()
                    .id("titlebar")
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .h(px(38.))
                    .pl(px(16.))
                    .pr(px(6.))
                    .border_b_1()
                    .border_color(t.line)
                    .bg(t.bg)
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(div().text_size(px(13.)).text_color(t.dim).child(title))
                    .child(
                        div()
                            .id("close")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(28.))
                            .rounded(px(theme::R_S))
                            .cursor_pointer()
                            .text_color(t.dim)
                            .hover(|s| s.bg(t.panel))
                            .child("✕")
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            )
        })
        .child(div().flex_1().min_h_0().child(body))
}

/// Read the latest status (None when the tray process isn't running).
pub fn live_status(paths: &Paths) -> Option<StatusSnapshot> {
    ipc::read_status(paths).filter(|s| s.alive(unix_now()))
}

/// Ask the tray process to do something; if it isn't running, do what we
/// can from here (open windows ourselves).
pub fn command(paths: &Paths, cmd: AppCommand) {
    if live_status(paths).is_none() {
        if let AppCommand::Open(w) = &cmd {
            if let Some(k) = WindowKind::parse(w) {
                let _ = crate::tray::spawn_window(k);
            }
            return;
        }
    }
    if let Err(e) = ipc::send_command(paths, &cmd) {
        log::warn!("command {}: {e}", cmd.encode());
    }
}

/// Text for "the daemon isn't running" banners.
pub fn not_running_text() -> SharedString {
    "The Clawdmeter tray app isn't running, so live status is unavailable. Start `clawdmeter` to bring it back.".into()
}

/// Run blocking work off the UI thread, then hand the result to the view.
pub fn run_blocking<V: 'static, R: Send + 'static>(
    cx: &mut gpui::Context<V>,
    work: impl FnOnce() -> R + Send + 'static,
    done: impl FnOnce(&mut V, R, &mut gpui::Context<V>) + 'static,
) {
    let task = cx.background_executor().spawn(async move { work() });
    cx.spawn(async move |this, cx| {
        let r = task.await;
        let _ = this.update(cx, |v, cx| {
            done(v, r, cx);
            cx.notify();
        });
    })
    .detach();
}
