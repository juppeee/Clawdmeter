//! The settings pages (spec §03): Device, Accounts, Hooks and Settings,
//! one view behind four sidebar entries. The setup wizard borrows the Hooks
//! page.
//!
//! It edits the same config file the daemon reads (through
//! `ConfigFile`/`Settings`, which keep comments, unknown keys and hand-written
//! values), so a hand-edited config and the GUI never disagree. The daemon
//! re-reads the file every few seconds.

use std::path::PathBuf;
use std::time::Duration;

use clawdmeter_core::autostart::{self, LaunchMode};
use clawdmeter_core::config::{ClockMode, ConfigFile};
use clawdmeter_core::{Paths, Settings};
use clawdmeter_flash::device::{self, FirmwareInfo};
use gpui::{div, img, prelude::*, px, Context, EventEmitter, PathPromptOptions, SharedString, Window};

use super::theme::{Theme, FONT_DISPLAY, FONT_MONO, R_M, R_S};
use super::widgets::{
    button, caption, card, field, heading, icon, notice, para, pill, segmented, subheading, toggle, ButtonKind, Tone,
};
use super::{command, live_status, not_running_text, run_blocking, Navigate, REFRESH};
use crate::diff::{line_diff, with_context, DiffLine};
use crate::errors;
use crate::hooks::install::{self as hooks, HookPlan, HookStatus, HOOKS};
use crate::ipc::{self, AppCommand, FirmwareRecord, StatusSnapshot};
use crate::tilde;
use crate::{boards, Page};

struct HookRow {
    dir: PathBuf,
    status: HookStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HookAction {
    Install,
    Remove,
}

struct HookPreview {
    action: HookAction,
    plans: Vec<HookPlan>,
}

#[derive(Default)]
struct DeviceState {
    probing: bool,
    port: Option<String>,
    info: Option<FirmwareInfo>,
    /// (effective, override) from the firmware's `minimal` command.
    minimal: Option<(bool, String)>,
    busy: bool,
    error: Option<String>,
    /// Result of clearing the board's pairing over USB.
    unpair: Option<(Tone, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PollChoice {
    Auto,
    Every(u64),
}

/// The settings pages.
pub struct SettingsView {
    paths: Paths,
    page: Page,
    /// Shown inside the setup wizard (no page heading).
    embedded: bool,
    /// One of this view's pages is on screen.
    visible: bool,
    snap: Option<StatusSnapshot>,
    settings: Settings,
    config_error: Option<String>,
    hooks: Vec<HookRow>,
    preview: Option<HookPreview>,
    hook_msg: Option<(Tone, String)>,
    /// The binary hooks point at (`None`: couldn't be determined).
    hook_exe: Option<PathBuf>,
    device: DeviceState,
    /// Probe USB for a board (off in tests).
    usb: bool,
    autostart_on: bool,
    /// The old daemon's login entry (systemd unit, LaunchAgent or Run value).
    legacy: Option<autostart::LegacyEntry>,
    notice: Option<(Tone, String)>,
    /// The Settings page's Advanced section is open.
    advanced: bool,
    ticks: u32,
}

impl EventEmitter<Navigate> for SettingsView {}

impl SettingsView {
    /// Build and load the config. USB is probed while the Device page is up.
    pub fn new(paths: Paths, page: Page, cx: &mut Context<Self>) -> Self {
        Self::with_usb(paths, page, true, cx)
    }

    /// [`SettingsView::new`]; `usb: false` never touches serial ports (tests).
    pub fn with_usb(paths: Paths, page: Page, usb: bool, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(REFRESH).await;
            if this.update(cx, |v, cx| v.tick(cx)).is_err() {
                break;
            }
        })
        .detach();
        let mut v = SettingsView {
            settings: Settings::from_config(&ConfigFile::default(), &paths, None),
            paths,
            page,
            embedded: false,
            visible: false,
            snap: None,
            config_error: None,
            hooks: Vec::new(),
            preview: None,
            hook_msg: None,
            hook_exe: hooks::hook_binary().ok(),
            device: DeviceState::default(),
            usb,
            autostart_on: false,
            legacy: None,
            notice: None,
            advanced: false,
            ticks: 0,
        };
        v.reload();
        v.show(page, false, cx);
        v
    }

    /// Put `page` on screen (`embedded`: inside the setup wizard). Other
    /// pages hide this view.
    pub fn show(&mut self, page: Page, embedded: bool, cx: &mut Context<Self>) {
        self.visible = matches!(page, Page::Device | Page::Accounts | Page::Hooks | Page::Settings);
        if !self.visible {
            return;
        }
        self.page = page;
        self.embedded = embedded;
        self.reload();
        if page == Page::Device && self.usb && !self.device.probing && self.device.port.is_none() {
            self.probe_device(cx);
        }
        cx.notify();
    }

    /// Accounts whose hooks need installing or updating.
    pub fn pending_hooks(&self) -> usize {
        self.hooks.iter().filter(|h| h.status.needs_install()).count()
    }

    /// Open the review of installing hooks in every account that needs them
    /// (what the page's install button does).
    pub fn review_hook_install(&mut self, cx: &mut Context<Self>) {
        let pending = self.hooks.iter().filter(|h| h.status.needs_install()).map(|h| h.dir.clone()).collect();
        self.plan_hooks(HookAction::Install, pending);
        cx.notify();
    }

    /// Accounts whose hooks are set up.
    pub fn installed_hooks(&self) -> usize {
        self.hooks.iter().filter(|h| h.status == HookStatus::Installed).count()
    }

    fn reload(&mut self) {
        match Settings::load(&self.paths) {
            Ok(s) => {
                self.settings = s;
                self.config_error = None;
            }
            Err(e) => {
                self.config_error = Some(format!(
                    "Couldn't read {}: {e}",
                    tilde(&self.paths.config_file.display().to_string(), &self.paths.home)
                ))
            }
        }
        self.hooks = self
            .settings
            .config_dirs
            .iter()
            .map(|d| HookRow {
                dir: d.clone(),
                status: match &self.hook_exe {
                    Some(exe) => hooks::dir_status(d, exe),
                    None => HookStatus::Unreadable("Couldn't locate the Clawdmeter executable.".into()),
                },
            })
            .collect();
        self.autostart_on = autostart::is_enabled(&self.paths.home, LaunchMode::Tray);
        self.legacy = autostart::legacy_entry(&self.paths.home);
        self.snap = live_status(&self.paths);
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.ticks += 1;
        self.snap = live_status(&self.paths);
        // Pick up hand edits and other windows' changes every few seconds,
        // but never under an open diff preview.
        if self.ticks.is_multiple_of(6) && self.preview.is_none() {
            self.reload();
        }
        // Notice a board being plugged in (only while the Device page is up,
        // so it never competes with the flasher for the port).
        let on_device = self.visible && self.page == Page::Device;
        if self.usb && on_device && self.ticks.is_multiple_of(4) && !self.device.probing && self.device.port.is_none() {
            if let Some(p) = device::list_ports().into_iter().next() {
                self.device.port = Some(p.path);
                self.probe_device(cx);
            }
        }
        cx.notify();
    }

    /// Change settings: re-read the file, apply the change, write only the
    /// keys whose value differs.
    fn save(&mut self, change: impl FnOnce(&mut Settings, &mut ConfigFile)) {
        let path = self.paths.config_file.clone();
        let result = (|| -> std::io::Result<()> {
            let mut cfg = ConfigFile::load(&path)?;
            let mut s = Settings::from_config(&cfg, &self.paths, None);
            change(&mut s, &mut cfg);
            s.apply_to(&mut cfg, &self.paths);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            cfg.save(&path)
        })();
        match result {
            Ok(()) => self.notice = None,
            Err(e) => self.notice = Some((Tone::Bad, errors::save_failed("settings", &path, &self.paths.home, &e))),
        }
        self.reload();
    }

    fn probe_device(&mut self, cx: &mut Context<Self>) {
        self.device.probing = true;
        run_blocking(
            cx,
            || {
                let port = device::list_ports().into_iter().next().map(|p| p.path);
                let Some(port) = port else { return (None, None, None) };
                let info = device::probe_info(&port, Duration::from_millis(1500)).ok().flatten();
                let minimal = info.as_ref().and_then(|_| {
                    device::query_json(&port, "minimal", "minimal", Duration::from_millis(1200)).ok().flatten()
                });
                (Some(port), info, minimal)
            },
            |v, (port, info, minimal), _| {
                v.device.probing = false;
                v.device.port = port;
                if let Some(i) = &info {
                    let _ =
                        ipc::save_firmware_record(&v.paths, &FirmwareRecord { env: i.env.clone(), ver: i.ver.clone() });
                }
                v.device.info = info;
                v.device.minimal = minimal.map(|m| {
                    (m["minimal"].as_bool().unwrap_or(false), m["override"].as_str().unwrap_or("auto").to_owned())
                });
            },
        );
    }

    fn set_minimal(&mut self, mode: &'static str, cx: &mut Context<Self>) {
        let Some(port) = self.device.port.clone() else { return };
        self.device.busy = true;
        self.device.error = None;
        run_blocking(
            cx,
            move || device::query_json(&port, &format!("minimal {mode}"), "minimal", Duration::from_millis(1500)),
            |v, r, _| {
                v.device.busy = false;
                match r {
                    Ok(Some(m)) => {
                        v.device.minimal = Some((
                            m["minimal"].as_bool().unwrap_or(false),
                            m["override"].as_str().unwrap_or("auto").to_owned(),
                        ))
                    }
                    Ok(None) => {
                        v.device.error = Some("No response from the device. Reconnect USB and try again.".into())
                    }
                    Err(e) => v.device.error = Some(e.to_string()),
                }
            },
        );
    }

    /// Clear every Bluetooth bond on the board (firmware `unpair`), so it
    /// can pair with this or another computer again.
    fn unpair(&mut self, cx: &mut Context<Self>) {
        let Some(port) = self.device.port.clone() else { return };
        self.device.busy = true;
        self.device.unpair = None;
        run_blocking(
            cx,
            move || device::query_json(&port, "unpair", "unpair", Duration::from_millis(1500)),
            |v, r, _| {
                v.device.busy = false;
                v.device.unpair = Some(match r {
                    Ok(Some(_)) => (
                        Tone::Ok,
                        "The board's pairing is cleared. Remove Clawdmeter from this computer's Bluetooth settings, \
                         then pair it again."
                            .into(),
                    ),
                    Ok(None) => (
                        Tone::Bad,
                        "No response from the device. Reconnect USB or update the firmware, then try again.".into(),
                    ),
                    Err(e) => (Tone::Bad, e.to_string()),
                });
            },
        );
    }

    fn add_account(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add Claude Code account folder".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let _ = this.update(cx, |v, cx| {
                    for p in paths {
                        if !v.settings.config_dirs.contains(&p) {
                            v.save(|s, _| s.config_dirs.push(p));
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn plan_hooks(&mut self, action: HookAction, dirs: Vec<PathBuf>) {
        self.hook_msg = None;
        let Some(exe) = self.hook_exe.clone() else { return };
        let mut plans = Vec::new();
        for d in dirs {
            let plan = match action {
                HookAction::Install => hooks::plan_install(&d, &exe),
                HookAction::Remove => hooks::plan_uninstall(&d),
            };
            match plan {
                Ok(p) if p.changes() => plans.push(p),
                Ok(_) => {}
                Err(e) => {
                    let f = tilde(&d.join("settings.json").display().to_string(), &self.paths.home);
                    self.hook_msg = Some((Tone::Bad, format!("{f}: {e}")));
                    return;
                }
            }
        }
        if plans.is_empty() {
            self.hook_msg = Some((Tone::Ok, "No changes needed.".into()));
        } else {
            self.preview = Some(HookPreview { action, plans });
        }
    }

    fn apply_hooks(&mut self) {
        let Some(p) = self.preview.take() else { return };
        let mut done = 0;
        for plan in &p.plans {
            if let Err(e) = plan.apply() {
                // Files are written one by one: say which ones made it.
                let what = tilde(&plan.path.display().to_string(), &self.paths.home);
                let saved = match done {
                    0 => String::new(),
                    1 => " 1 other file was already saved.".into(),
                    n => format!(" {n} other files were already saved."),
                };
                self.hook_msg = Some((Tone::Bad, format!("Couldn't write {what}. {}{saved}", errors::io_message(&e))));
                self.reload();
                return;
            }
            done += 1;
        }
        let accounts = if done == 1 { "1 account".to_string() } else { format!("{done} accounts") };
        self.hook_msg = Some((
            Tone::Ok,
            match p.action {
                HookAction::Install => {
                    format!("Hooks installed in {accounts}. Existing files were backed up to settings.json.bak.")
                }
                HookAction::Remove => {
                    format!("Hooks removed from {accounts}. Existing files were backed up to settings.json.bak.")
                }
            },
        ));
        self.reload();
    }

    fn send_test_alert(&mut self) {
        let path = self.paths.attention_file();
        let body = serde_json::json!({"m": "Clawdmeter test alert", "s": "clawdmeter-test"});
        self.hook_msg = Some(
            match std::fs::create_dir_all(&self.paths.state_dir).and_then(|_| std::fs::write(&path, body.to_string())) {
                Ok(()) if self.snap.is_none() => {
                    (Tone::Warn, "Clawdmeter isn't running. Start it, then send another test alert.".into())
                }
                Ok(()) => (Tone::Ok, "Test alert queued. Check the device; tap it to dismiss.".into()),
                Err(e) => (Tone::Bad, format!("Couldn't send the test alert. {}", errors::io_message(&e))),
            },
        );
    }

    // ---------------------------------------------------------------- panes

    fn device_pane(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let snap = self.snap.clone();
        let rec = ipc::load_firmware_record(&self.paths);
        let env = self.device.info.as_ref().map(|i| i.env.clone()).or_else(|| rec.as_ref().map(|r| r.env.clone()));
        let ver = self
            .device
            .info
            .as_ref()
            .map(|i| i.ver.clone())
            .or_else(|| rec.as_ref().map(|r| r.ver.clone()))
            .or_else(|| snap.as_ref().and_then(|s| s.device_firmware.clone()));
        let board = env.as_deref().and_then(boards::board);
        let name: SharedString = match (board, &env) {
            (Some(b), _) => b.product.into(),
            (None, Some(e)) => e.clone().into(),
            (None, None) => "Board not identified".into(),
        };
        let update = snap.as_ref().and_then(|s| s.update_available().map(str::to_owned));
        let (link_tone, link): (Tone, SharedString) = match snap.as_ref() {
            None => (Tone::Plain, "Clawdmeter isn't running".into()),
            Some(s) => match s.link.as_str() {
                "connected" => (Tone::Ok, "Bluetooth connected".into()),
                "no_adapter" => (Tone::Bad, "Bluetooth unavailable".into()),
                "connecting" => (Tone::Accent, "Connecting…".into()),
                _ => (Tone::Plain, "Looking for Clawdmeter".into()),
            },
        };
        let usb: SharedString = match (&self.device.port, self.device.probing) {
            (_, true) => "Checking USB…".into(),
            (Some(p), _) => format!("USB: {p}").into(),
            (None, _) if board.is_none() => "Connect USB to identify the board.".into(),
            (None, _) => "USB not connected".into(),
        };
        let firmware: SharedString = match (&ver, &update) {
            (Some(v), Some(u)) => {
                format!("Firmware v{} · v{} available", v.trim_start_matches('v'), u.trim_start_matches('v')).into()
            }
            (Some(v), None) => format!("Firmware v{}", v.trim_start_matches('v')).into(),
            (None, _) => "Firmware version unknown".into(),
        };
        let art = div().flex().flex_shrink_0().items_center().justify_center().size(px(150.)).child(match board {
            Some(b) => img(b.illustration()).size(px(150.)).into_any_element(),
            None => super::widgets::clawd(4.).into_any_element(),
        });
        let (label, kind) = match &update {
            Some(_) => ("Update firmware", ButtonKind::Primary),
            None => ("Flash firmware…", ButtonKind::Normal),
        };
        let card = card(t).flex_row().items_center().gap(px(20.)).p(px(20.)).child(art).child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(8.))
                .child(div().font_family(FONT_DISPLAY).text_size(px(19.)).child(name))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .child(pill(t, link, link_tone))
                        .when_some(update.as_ref(), |d, _| d.child(pill(t, "Update available", Tone::Accent))),
                )
                .child(div().text_size(px(13.)).text_color(t.dim).child(firmware))
                .child(div().text_size(px(13.)).text_color(t.mute).child(usb))
                .child(div().flex().pt(px(4.)).child(button(
                    t,
                    "update-now",
                    label,
                    kind,
                    true,
                    cx.listener(|_, _, _, cx| cx.emit(Navigate(Page::Flash))),
                ))),
        );
        let mut pane = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(heading(t, "Device"))
            .child(div().pt(px(8.)).pb(px(10.)).child(card));

        // Minimal display (sent over USB).
        let minimal_sel = self.device.minimal.as_ref().map(|(_, o)| o.clone()).unwrap_or_else(|| "auto".into());
        let can_minimal = self.device.port.is_some() && self.device.minimal.is_some() && !self.device.busy;
        let me = cx.entity().downgrade();
        let minimal_help = match (&self.device.minimal, &self.device.port) {
            (Some((eff, _)), _) => {
                format!("Meters and a still Clawd. Minimal display is {}.", if *eff { "on" } else { "off" })
            }
            (None, Some(_)) if self.device.probing => "Reading display mode…".into(),
            (None, Some(_)) => "Couldn't read display mode. Reconnect USB or update the firmware.".into(),
            (None, None) => "Meters and a still Clawd. Connect USB to change this setting.".into(),
        };
        pane = pane.child(field(
            t,
            "Minimal display",
            minimal_help,
            segmented(
                t,
                "minimal",
                &[("auto".to_string(), "Board default"), ("on".to_string(), "On"), ("off".to_string(), "Off")],
                &minimal_sel,
                can_minimal,
                move |m, _, cx| {
                    let mode: &'static str = match m.as_str() {
                        "on" => "on",
                        "off" => "off",
                        _ => "auto",
                    };
                    let _ = me.update(cx, |v, cx| v.set_minimal(mode, cx));
                },
            ),
        ));
        if let Some(e) = &self.device.error {
            pane = pane.child(div().text_size(px(12.5)).text_color(t.bad).child(e.clone()));
        }

        let me = cx.entity().downgrade();
        pane = pane.child(field(
            t,
            "Clock",
            "Replaces the Usage title with a clock. Auto follows this computer's time format.",
            segmented(
                t,
                "clock",
                &[
                    (ClockMode::Off, "Off"),
                    (ClockMode::Auto, "Auto"),
                    (ClockMode::H12, "12 h"),
                    (ClockMode::H24, "24 h"),
                ],
                &self.settings.clock,
                true,
                move |m, _, cx| {
                    let _ = me.update(cx, |v, cx| {
                        v.save(|s, _| s.clock = m);
                        cx.notify();
                    });
                },
            ),
        ));
        let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
        let (beep, chime) = (self.settings.beep, self.settings.chime);
        pane = pane
            .child(field(
                t,
                "Attention beep",
                "Sound for attention alerts on supported boards. Mute from the tray until restart.",
                toggle(t, "beep", beep, true, move |_, _, cx| {
                    let _ = me1.update(cx, |v, cx| {
                        v.save(|s, _| s.beep = !beep);
                        cx.notify();
                    });
                }),
            ))
            .child(field(
                t,
                "Session reset chime",
                "Sound when a session reset is detected, on supported boards.",
                toggle(t, "chime", chime, true, move |_, _, cx| {
                    let _ = me2.update(cx, |v, cx| {
                        v.save(|s, _| s.chime = !chime);
                        cx.notify();
                    });
                }),
            ));
        let me = cx.entity().downgrade();
        let on = self.settings.update_check;
        pane = pane.child(field(
            t,
            "Check for firmware updates",
            "Checks GitHub daily. Updates appear here and in the tray.",
            toggle(t, "update-check", on, true, move |_, _, cx| {
                let _ = me.update(cx, |v, cx| {
                    v.save(|s, _| s.update_check = !on);
                    cx.notify();
                });
            }),
        ));
        let p = self.paths.clone();
        pane = pane.child(field(
            t,
            "Clear saved device",
            "Clears the app's saved address and searches for a paired Clawdmeter. Bluetooth pairing stays in place.",
            button(t, "forget", "Clear", ButtonKind::Small, true, move |_, _, _| command(&p, AppCommand::Forget)),
        ));
        let on_usb = self.device.port.is_some();
        let me = cx.entity().downgrade();
        pane = pane.child(field(
            t,
            "Clear the board's pairing",
            if on_usb {
                "Makes the board forget every computer it was paired with, so it can pair again."
            } else {
                "Makes the board forget every computer it was paired with. Connect USB to use this."
            },
            button(t, "unpair", "Clear pairing", ButtonKind::Small, on_usb && !self.device.busy, move |_, _, cx| {
                let _ = me.update(cx, |v, cx| v.unpair(cx));
            }),
        ));
        if let Some((tone, msg)) = &self.device.unpair {
            pane = pane.child(notice(t, *tone, msg.clone()));
        }
        pane
    }

    fn accounts_pane(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let me = cx.entity().downgrade();
        let mut pane = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(div().flex().items_center().justify_between().child(heading(t, "Accounts")).child(button(
                t,
                "add-dir",
                "Add account folder…",
                ButtonKind::Small,
                true,
                move |_, _, cx| {
                    let _ = me.update(cx, |v, cx| v.add_account(cx));
                },
            )))
            .child(para(
                t,
                "Add a Claude Code config folder for each account you want to monitor. The device follows the \
                 account whose session usage increased most recently.",
            ));
        let active = self.snap.as_ref().and_then(|s| s.account.clone());
        let errors: Vec<ipc::AccountError> = self.snap.as_ref().map(|s| s.errors.clone()).unwrap_or_default();
        let only_one = self.settings.config_dirs.len() <= 1;
        let mut list = div().flex().flex_col().rounded(px(R_M)).border_1().border_color(t.line);
        for (i, d) in self.settings.config_dirs.clone().into_iter().enumerate() {
            let ds = d.display().to_string();
            let err = errors.iter().find(|e| e.dir == ds).cloned();
            let is_active = active.as_deref() == Some(ds.as_str());
            let (meta, chip) = match (&err, is_active) {
                // Transient errors retry on their own; the rest need the user.
                (Some(e), _) if e.retrying => (e.message.clone(), Some(pill(t, "Retrying", Tone::Warn))),
                (Some(e), _) => (e.message.clone(), Some(pill(t, "Needs attention", Tone::Bad))),
                (None, true) => (String::new(), Some(pill(t, "On device", Tone::Ok))),
                (None, false) if self.snap.is_some() => ("Not selected for the device".to_string(), None),
                (None, false) => ("Start Clawdmeter to check usage.".to_string(), None),
            };
            let me = cx.entity().downgrade();
            let dir = d.clone();
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(14.))
                    .py(px(12.))
                    .when(i > 0, |r| r.border_t_1().border_color(t.line))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(div().font_family(FONT_MONO).text_size(px(13.)).child(tilde(&ds, &self.paths.home)))
                            .when(!meta.is_empty(), |d| {
                                d.child(div().text_size(px(12.5)).text_color(t.mute).child(meta))
                            }),
                    )
                    .when_some(chip, |r, c| r.child(c))
                    .child(button(
                        t,
                        SharedString::from(format!("rm-{i}")),
                        "Remove",
                        ButtonKind::Small,
                        !only_one,
                        move |_, _, cx| {
                            let dir = dir.clone();
                            let _ = me.update(cx, |v, cx| {
                                v.save(|s, _| s.config_dirs.retain(|x| *x != dir));
                                cx.notify();
                            });
                        },
                    )),
            );
        }
        pane = pane.child(list);
        if self.snap.is_none() {
            pane = pane.child(notice(t, Tone::Plain, not_running_text()));
        }
        pane
    }

    fn hooks_pane(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut pane = div().flex().flex_col().gap(px(14.)).when(!self.embedded, |d| {
            d.child(heading(t, "Hooks")).child(para(
                t,
                "Hooks let Clawd react to activity and show alerts. The event log stores timestamps, event types, \
                 and session IDs. Alerts also store notification or question text for the device. Usage meters work \
                 without hooks.",
            ))
        });
        let busy = self.preview.is_some();
        let mut list = div().flex().flex_col().rounded(px(R_M)).border_1().border_color(t.line);
        for (i, row) in self.hooks.iter().enumerate() {
            let (meta, chip, action): (String, (&str, Tone), Option<(&str, HookAction)>) = match &row.status {
                HookStatus::Installed => (
                    format!("{} hooks installed", HOOKS.len()),
                    ("Installed", Tone::Ok),
                    Some(("Review removal", HookAction::Remove)),
                ),
                HookStatus::Missing => (
                    "Clawdmeter hooks are not installed. Other hooks are kept.".into(),
                    ("Not installed", Tone::Plain),
                    Some(("Review install", HookAction::Install)),
                ),
                HookStatus::Legacy => (
                    "Uses attention-hook.sh. Update to the built-in Clawdmeter hooks.".into(),
                    ("Legacy hooks", Tone::Warn),
                    Some(("Review update", HookAction::Install)),
                ),
                HookStatus::Stale(old) => (
                    format!("Uses another Clawdmeter executable: {}.", tilde(old, &self.paths.home)),
                    ("Out of date", Tone::Warn),
                    Some(("Review update", HookAction::Install)),
                ),
                HookStatus::Partial => (
                    "Some hooks are missing.".into(),
                    ("Incomplete", Tone::Warn),
                    Some(("Review update", HookAction::Install)),
                ),
                HookStatus::Unreadable(e) => (e.clone(), ("Unreadable", Tone::Bad), None),
            };
            let mut r = div()
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(14.))
                .py(px(12.))
                .when(i > 0, |r| r.border_t_1().border_color(t.line))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(3.))
                        .child(
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(13.))
                                .truncate()
                                .child(tilde(&row.dir.join("settings.json").display().to_string(), &self.paths.home)),
                        )
                        .child(div().text_size(px(12.5)).text_color(t.mute).child(meta)),
                )
                .child(pill(t, chip.0, chip.1));
            if let Some((label, action)) = action {
                let me = cx.entity().downgrade();
                let dir = row.dir.clone();
                r = r.child(button(
                    t,
                    SharedString::from(format!("hook-{i}")),
                    label,
                    ButtonKind::Small,
                    !busy,
                    move |_, _, cx| {
                        let dir = dir.clone();
                        let _ = me.update(cx, |v, cx| {
                            v.plan_hooks(action, vec![dir]);
                            cx.notify();
                        });
                    },
                ));
            }
            list = list.child(r);
        }
        pane = pane.child(list);

        if let Some(p) = &self.preview {
            let n = p.plans.len();
            let title = match (p.action, n) {
                (HookAction::Install, 1) => "Review 1 file".to_string(),
                (HookAction::Install, n) => format!("Review {n} files"),
                (HookAction::Remove, _) => "Review hook removal".to_string(),
            };
            let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
            let apply = match p.action {
                HookAction::Install => "Install hooks",
                HookAction::Remove => "Remove hooks",
            };
            pane = pane.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .pt(px(6.))
                    .child(div().flex_1().child(subheading(t, title)))
                    .child(button(t, "apply-hooks", apply, ButtonKind::Primary, true, move |_, _, cx| {
                        let _ = me1.update(cx, |v, cx| {
                            v.apply_hooks();
                            cx.notify();
                        });
                    }))
                    .child(button(t, "cancel-hooks", "Cancel", ButtonKind::Normal, true, move |_, _, cx| {
                        let _ = me2.update(cx, |v, cx| {
                            v.preview = None;
                            cx.notify();
                        });
                    })),
            );
            pane = pane.child(caption(t, "Review before applying. Existing files are backed up to settings.json.bak."));
            for plan in &p.plans {
                // Diff what changes, not how the file was formatted: compare
                // against the old file pretty-printed the way it will be saved.
                let before = plan.before.as_deref().unwrap_or("");
                let pretty_before = serde_json::from_str::<serde_json::Value>(before)
                    .ok()
                    .and_then(|v| serde_json::to_string_pretty(&v).ok())
                    .map(|s| s + "\n");
                let reformatted = pretty_before.as_deref().is_some_and(|p| p.trim() != before.trim());
                let diff = with_context(line_diff(pretty_before.as_deref().unwrap_or(before), &plan.after), 2);
                let mut block = div()
                    .flex()
                    .flex_col()
                    .py(px(10.))
                    .rounded(px(R_S))
                    .bg(t.bg)
                    .border_1()
                    .border_color(t.line)
                    .font_family(FONT_MONO)
                    .text_size(px(12.))
                    .line_height(px(18.));
                for l in diff {
                    let (sign, text, fg, bg) = match l {
                        DiffLine::Same(s) => (" ", s, t.mute, None),
                        DiffLine::Added(s) => ("+", s, t.ok, Some(with_alpha(t.ok, 0.10))),
                        DiffLine::Removed(s) => ("-", s, t.bad, Some(with_alpha(t.bad, 0.10))),
                    };
                    block = block.child(
                        div()
                            .flex()
                            .px(px(12.))
                            .text_color(fg)
                            .when_some(bg, |d, bg| d.bg(bg))
                            .child(div().w(px(16.)).flex_shrink_0().child(sign))
                            .child(div().flex_1().min_w_0().child(text)),
                    );
                }
                let file = tilde(&plan.path.display().to_string(), &self.paths.home);
                let what = match (plan.before.is_none(), reformatted) {
                    (true, _) => format!("{file} (new file)"),
                    (false, true) => format!("{file} (also re-indented)"),
                    (false, false) => file,
                };
                pane = pane.child(div().flex().flex_col().gap(px(6.)).child(caption(t, what)).child(block));
            }
        } else {
            let pending: Vec<PathBuf> =
                self.hooks.iter().filter(|h| h.status.needs_install()).map(|h| h.dir.clone()).collect();
            let any_old = self.hooks.iter().any(|h| h.status.needs_install() && h.status != HookStatus::Missing);
            let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
            let n = pending.len();
            let accounts = if n == 1 { "1 account".to_string() } else { format!("{n} accounts") };
            let label = if any_old {
                format!("Review updates for {accounts}")
            } else {
                format!("Review installation in {accounts}")
            };
            pane = pane.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .when(n > 0, |d| {
                        d.child(button(t, "install-all", label, ButtonKind::Primary, true, move |_, _, cx| {
                            let pending = pending.clone();
                            let _ = me1.update(cx, |v, cx| {
                                v.plan_hooks(HookAction::Install, pending);
                                cx.notify();
                            });
                        }))
                    })
                    .when(!self.embedded, |d| {
                        d.child(button(
                            t,
                            "test-alert",
                            "Send test alert",
                            ButtonKind::Normal,
                            true,
                            move |_, _, cx| {
                                let _ = me2.update(cx, |v, cx| {
                                    v.send_test_alert();
                                    cx.notify();
                                });
                            },
                        ))
                    }),
            );
        }
        if let Some((tone, m)) = &self.hook_msg {
            pane = pane.child(notice(t, *tone, m.clone()));
        }
        pane
    }

    fn settings_pane(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let me = cx.entity().downgrade();
        let on = self.autostart_on;
        let home = self.paths.home.clone();
        let mut pane = div().flex().flex_col().gap(px(6.)).child(heading(t, "Settings")).child(field(
            t,
            "Start at login",
            "Opens Clawdmeter when you sign in.",
            toggle(t, "autostart", on, true, move |_, _, cx| {
                let home = home.clone();
                let _ = me.update(cx, |v, cx| {
                    let r = match std::env::current_exe() {
                        Ok(exe) if !on => autostart::enable(&home, &exe, LaunchMode::Tray),
                        Ok(_) => autostart::disable(&home, LaunchMode::Tray),
                        Err(e) => Err(e),
                    };
                    if let Err(e) = r {
                        v.notice =
                            Some((Tone::Bad, format!("Couldn't change Start at login. {}", errors::io_message(&e))));
                    }
                    v.reload();
                    cx.notify();
                });
            }),
        ));

        let poll = if self.settings.poll_interval_explicit {
            PollChoice::Every(self.settings.poll_interval)
        } else {
            PollChoice::Auto
        };
        let mut options = vec![
            (PollChoice::Auto, "Automatic"),
            (PollChoice::Every(120), "While working: 2 min"),
            (PollChoice::Every(300), "While working: 5 min"),
        ];
        let custom;
        if let PollChoice::Every(n) = poll {
            if n != 120 && n != 300 {
                custom = format!("While working: {n} s");
                options.push((poll, custom.as_str()));
            }
        }
        let me = cx.entity().downgrade();
        pane = pane.child(field(
            t,
            "Usage checks",
            "Checks every 3 minutes while working, then less often when quiet. Retry delays apply to all checks, \
             including manual refresh.",
            segmented(t, "poll", &options, &poll, true, move |c, _, cx| {
                let _ = me.update(cx, |v, cx| {
                    let paths = v.paths.clone();
                    v.save(|s, cfg| match c {
                        PollChoice::Auto => {
                            cfg.remove("poll_interval");
                            let fresh = Settings::from_config(cfg, &paths, None);
                            s.poll_interval = fresh.poll_interval;
                            s.poll_interval_explicit = false;
                        }
                        PollChoice::Every(n) => {
                            s.poll_interval = n;
                            cfg.set("poll_interval", &n.to_string());
                        }
                    });
                    cx.notify();
                });
            }),
        ));
        let p = self.paths.clone();
        pane = pane.child(field(
            t,
            "Quit Clawdmeter",
            "Stops usage updates and closes Clawdmeter. The device retains its last reading.",
            button(t, "quit", "Quit", ButtonKind::Small, self.snap.is_some(), move |_, window, _| {
                command(&p, AppCommand::Quit);
                window.remove_window();
            }),
        ));
        if let Some(legacy) = self.legacy.clone() {
            let me = cx.entity().downgrade();
            let home = self.paths.home.clone();
            pane = pane.child(field(
                t,
                "Legacy service detected",
                format!(
                    "Disable {} so only one Clawdmeter process connects to the device.",
                    tilde(&legacy.describe(), &self.paths.home)
                ),
                button(t, "legacy", "Disable legacy service", ButtonKind::Primary, true, move |_, _, cx| {
                    autostart::disable_legacy(&home);
                    let _ = me.update(cx, |v, cx| {
                        v.reload();
                        if let Some(still) = &v.legacy {
                            v.notice = Some((
                                Tone::Bad,
                                format!(
                                    "Couldn't disable {}. Remove it by hand.",
                                    tilde(&still.describe(), &v.paths.home)
                                ),
                            ));
                        }
                        cx.notify();
                    });
                }),
            ));
        }
        // Rarely needed: where things live and which CLI runs.
        let open = self.advanced;
        let text = t.text;
        pane = pane.child(
            div()
                .id("advanced")
                .group("advanced")
                .debug_selector(|| "advanced".into())
                .flex()
                .items_center()
                .gap(px(6.))
                .pt(px(14.))
                .pb(px(4.))
                .text_size(px(14.))
                .text_color(t.dim)
                .cursor_pointer()
                .hover(|s| s.text_color(t.text))
                .on_click(cx.listener(|v, _, _, cx| {
                    v.advanced = !v.advanced;
                    cx.notify();
                }))
                .child(
                    icon(if open { "chevron-down" } else { "chevron-right" }, 16., t.dim)
                        .group_hover("advanced", move |s| s.text_color(text)),
                )
                .child("Advanced"),
        );
        if open {
            let mut advanced = div().flex().flex_col();
            let claude = self.snap.as_ref().and_then(|s| s.claude.clone());
            advanced = advanced.child(field(
                t,
                "Claude Code executable",
                match &claude {
                    Some(c) => format!("Usage is read with {}.", tilde(c, &self.paths.home)),
                    None if self.snap.is_some() => {
                        "Claude Code wasn't found. Set claude_path in the config file.".into()
                    }
                    None => "Start Clawdmeter to detect Claude Code.".into(),
                },
                div(),
            ));
            let log = self.paths.state_dir.join("daemon.log");
            advanced = advanced.child(field(
                t,
                "Log file",
                tilde(&log.display().to_string(), &self.paths.home),
                button(t, "open-log", "Open log", ButtonKind::Small, log.exists(), move |_, _, cx| {
                    cx.open_with_system(&log)
                }),
            ));
            let cfgp = self.paths.config_file.clone();
            advanced = advanced.child(field(
                t,
                "Config file",
                tilde(&cfgp.display().to_string(), &self.paths.home),
                button(t, "open-cfg", "Open config", ButtonKind::Small, cfgp.exists(), move |_, _, cx| {
                    cx.open_with_system(&cfgp)
                }),
            ));
            pane = pane.child(advanced);
        }
        pane
    }
}

fn with_alpha(c: gpui::Rgba, a: f32) -> gpui::Hsla {
    let mut h: gpui::Hsla = c.into();
    h.a = a;
    h
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::for_appearance(window.appearance());
        let mut pane = match self.page {
            Page::Accounts => self.accounts_pane(&t, cx),
            Page::Hooks => self.hooks_pane(&t, cx),
            Page::Settings => self.settings_pane(&t, cx),
            _ => self.device_pane(&t, cx),
        };
        if let Some(e) = self.config_error.clone().or_else(|| self.notice.as_ref().map(|(_, m)| m.clone())) {
            pane = pane.child(div().pt(px(12.)).text_size(px(13.)).text_color(t.bad).child(e));
        }
        pane
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    use super::*;

    fn home(tag: &str) -> Paths {
        let root = std::env::temp_dir().join(format!("cm-settings-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::under(&root);
        std::fs::create_dir_all(&paths.state_dir).unwrap();
        std::fs::create_dir_all(root.join("acct")).unwrap();
        std::fs::write(&paths.config_file, format!("# mine\nconfig_dirs = {}\n", root.join("acct").display())).unwrap();
        paths
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        cx.run_until_parked();
        let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} isn't on screen"));
        cx.simulate_click(b.center(), Modifiers::none());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn toggles_and_choices_write_the_config(cx: &mut TestAppContext) {
        let paths = home("cfg");
        let p = paths.clone();
        let (view, cx) = cx.add_window_view(move |_, cx| SettingsView::with_usb(p, Page::Device, false, cx));
        click(cx, "chime");
        let text = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(text.starts_with("# mine\n"), "comments kept: {text}");
        assert!(text.contains("chime = on"), "{text}");

        view.update(cx, |v, cx| v.show(Page::Settings, false, cx));
        click(cx, "poll-2"); // While working: 5 min
        let text = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(text.contains("poll_interval = 300"), "{text}");
        click(cx, "poll-0"); // Automatic removes the key again.
        assert!(!std::fs::read_to_string(&paths.config_file).unwrap().contains("poll_interval"));
        let _ = std::fs::remove_dir_all(&paths.home);
    }

    #[gpui::test]
    fn hooks_install_after_preview(cx: &mut TestAppContext) {
        let paths = home("hooks");
        let settings = paths.home.join("acct/settings.json");
        let before = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"/old/attention-hook.sh event"}]}]}}"#;
        std::fs::write(&settings, before).unwrap();
        let p = paths.clone();
        let (view, cx) = cx.add_window_view(move |_, cx| SettingsView::with_usb(p, Page::Hooks, false, cx));
        view.read_with(cx, |v, _| assert_eq!(v.hooks[0].status, HookStatus::Legacy));

        click(cx, "install-all");
        // Nothing is written until the preview is confirmed.
        assert_eq!(std::fs::read_to_string(&settings).unwrap(), before);
        view.read_with(cx, |v, _| assert!(v.preview.is_some()));
        click(cx, "apply-hooks");

        let after = std::fs::read_to_string(&settings).unwrap();
        assert!(!after.contains("attention-hook.sh"), "{after}");
        assert!(after.contains(" hook clear event"), "{after}");
        assert_eq!(std::fs::read_to_string(settings.with_extension("json.bak")).unwrap(), before);
        view.read_with(cx, |v, _| {
            assert_eq!(v.hooks[0].status, HookStatus::Installed);
            assert!(v.preview.is_none());
        });
        let _ = std::fs::remove_dir_all(&paths.home);
    }
}
