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
    button, caption, card, field, heading, notice, para, pill, segmented, subheading, toggle, ButtonKind, Tone,
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
                    None => HookStatus::Unreadable("Couldn't find the Clawdmeter program itself.".into()),
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
                        v.device.error =
                            Some("The device didn't answer. Unplug it, plug it back in and try again.".into())
                    }
                    Err(e) => v.device.error = Some(e.to_string()),
                }
            },
        );
    }

    fn add_account(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add Claude config folder".into()),
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
            self.hook_msg = Some((Tone::Ok, "Already up to date.".into()));
        } else {
            self.preview = Some(HookPreview { action, plans });
        }
    }

    fn apply_hooks(&mut self) {
        let Some(p) = self.preview.take() else { return };
        let mut done = 0;
        for plan in &p.plans {
            if let Err(e) = plan.apply() {
                let what = tilde(&plan.path.display().to_string(), &self.paths.home);
                self.hook_msg = Some((Tone::Bad, format!("Couldn't write {what}. {}", errors::io_message(&e))));
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
                    format!("Hooks set up in {accounts}. The old file is saved as settings.json.bak.")
                }
                HookAction::Remove => {
                    format!("Hooks removed from {accounts}. The old file is saved as settings.json.bak.")
                }
            },
        ));
        self.reload();
    }

    fn send_test_alert(&mut self) {
        let path = self.paths.attention_file();
        let body = serde_json::json!({"m": "Test alert from Clawdmeter settings", "s": "clawdmeter-test"});
        self.hook_msg = Some(
            match std::fs::create_dir_all(&self.paths.state_dir).and_then(|_| std::fs::write(&path, body.to_string())) {
                Ok(()) if self.snap.is_none() => {
                    (Tone::Warn, "Alert queued, but Clawdmeter isn't running, so nothing will show it.".into())
                }
                Ok(()) => (Tone::Ok, "Test alert sent. Clawd should come find you; tap the device to dismiss.".into()),
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
            (None, None) => "Board not identified yet".into(),
        };
        let update = snap.as_ref().and_then(|s| s.update_available().map(str::to_owned));
        let (link_tone, link): (Tone, SharedString) = match snap.as_ref() {
            None => (Tone::Plain, "Clawdmeter isn't running".into()),
            Some(s) => match s.link.as_str() {
                "connected" => (Tone::Ok, "Connected over Bluetooth".into()),
                "no_adapter" => (Tone::Bad, "Bluetooth is off".into()),
                "connecting" => (Tone::Accent, "Connecting…".into()),
                _ => (Tone::Plain, "Looking for it over Bluetooth".into()),
            },
        };
        let usb: SharedString = match (&self.device.port, self.device.probing) {
            (_, true) => "Checking USB…".into(),
            (Some(p), _) => format!("On USB at {p}").into(),
            (None, _) if board.is_none() => "Plug it in over USB to identify it".into(),
            (None, _) => "Not on USB".into(),
        };
        let firmware: SharedString = match (&ver, &update) {
            (Some(v), Some(u)) => {
                format!("Firmware v{} · v{} is out", v.trim_start_matches('v'), u.trim_start_matches('v')).into()
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
                format!("A still Clawd and the meters, no animation. Currently {}.", if *eff { "on" } else { "off" })
            }
            (None, Some(_)) if self.device.probing => "Asking the device…".into(),
            (None, Some(_)) => "This firmware is too old for minimal mode. Update it first.".into(),
            (None, None) => {
                "A still Clawd and the meters, no animation. Changed over USB, so plug the device in.".into()
            }
        };
        pane = pane.child(field(
            t,
            "Minimal display",
            minimal_help,
            segmented(
                t,
                "minimal",
                &[("auto".to_string(), "Auto"), ("on".to_string(), "On"), ("off".to_string(), "Off")],
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
            "Shows the time at the top of the screen. Auto follows this computer's 12/24-hour setting.",
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
                "Beep when Claude needs you",
                "Plays on boards with a speaker. You can mute it for a while from the tray or the Overview.",
                toggle(t, "beep", beep, true, move |_, _, cx| {
                    let _ = me1.update(cx, |v, cx| {
                        v.save(|s, _| s.beep = !beep);
                        cx.notify();
                    });
                }),
            ))
            .child(field(
                t,
                "Chime when the session resets",
                "One short chime when your 5-hour window rolls over.",
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
            "Looks for new firmware on GitHub once a day and tells you here and in the tray.",
            toggle(t, "update-check", on, true, move |_, _, cx| {
                let _ = me.update(cx, |v, cx| {
                    v.save(|s, _| s.update_check = !on);
                    cx.notify();
                });
            }),
        ));
        let p = self.paths.clone();
        pane.child(field(
            t,
            "Forget this device",
            "Drops the remembered address and picks up whichever paired Clawdmeter is around. Use it after swapping boards.",
            button(t, "forget", "Forget", ButtonKind::Small, true, move |_, _, _| command(&p, AppCommand::Forget)),
        ))
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
                "Add folder…",
                ButtonKind::Small,
                true,
                move |_, _, cx| {
                    let _ = me.update(cx, |v, cx| v.add_account(cx));
                },
            )))
            .child(para(
                t,
                "Each Claude Code config folder is an account: the CLAUDE_CONFIG_DIR you run claude with. \
                 Clawdmeter checks all of them; the device shows whichever you used most recently.",
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
                (Some(e), _) => (e.message.clone(), Some(pill(t, "Problem", Tone::Bad))),
                (None, true) => ("Shown on the device".to_string(), Some(pill(t, "On device", Tone::Ok))),
                (None, false) if self.snap.is_some() => ("Checked, not shown".to_string(), None),
                (None, false) => ("Not checked while Clawdmeter is stopped".to_string(), None),
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
                            .child(div().text_size(px(12.5)).text_color(t.mute).child(meta)),
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
                "Hooks let Clawd react while you work: he notices prompts, keeps up with tools, and comes to find \
                 you when Claude Code is waiting. Each account gets five small entries in its settings.json that \
                 record only the event type and session ID. Without them Clawdmeter still works; it just checks \
                 every 2 minutes.",
            ))
        });
        let busy = self.preview.is_some();
        let mut list = div().flex().flex_col().rounded(px(R_M)).border_1().border_color(t.line);
        for (i, row) in self.hooks.iter().enumerate() {
            let (meta, chip, action): (String, (&str, Tone), Option<(&str, HookAction)>) = match &row.status {
                HookStatus::Installed => (
                    format!("All {} hooks set up", HOOKS.len()),
                    ("Installed", Tone::Ok),
                    Some(("Remove", HookAction::Remove)),
                ),
                HookStatus::Missing => (
                    "Not set up. Hooks you added yourself stay as they are.".into(),
                    ("Not installed", Tone::Plain),
                    Some(("Install", HookAction::Install)),
                ),
                HookStatus::Legacy => (
                    "Uses the old attention-hook.sh script. Updating switches to the built-in hook.".into(),
                    ("Old script", Tone::Warn),
                    Some(("Update", HookAction::Install)),
                ),
                HookStatus::Stale(old) => (
                    format!("Points at {}, not this copy of Clawdmeter.", tilde(old, &self.paths.home)),
                    ("Out of date", Tone::Warn),
                    Some(("Update", HookAction::Install)),
                ),
                HookStatus::Partial => (
                    "Some hooks are missing.".into(),
                    ("Incomplete", Tone::Warn),
                    Some(("Update", HookAction::Install)),
                ),
                HookStatus::Unreadable(e) => (e.clone(), ("Can't read", Tone::Bad), None),
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
                (HookAction::Install, 1) => "Review the change to 1 file".to_string(),
                (HookAction::Install, n) => format!("Review the changes to {n} files"),
                (HookAction::Remove, _) => "Review the removal".to_string(),
            };
            let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
            let apply = match p.action {
                HookAction::Install => "Save changes",
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
            pane =
                pane.child(caption(t, "Nothing is written until you save. The old file is kept as settings.json.bak."));
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
            let label = if any_old { format!("Update {accounts}") } else { format!("Install in {accounts}") };
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
                            "Send a test alert",
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
            "Starts Clawdmeter in the tray when you log in.",
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
            (PollChoice::Every(120), "Every 2 min"),
            (PollChoice::Every(300), "Every 5 min"),
        ];
        let custom;
        if let PollChoice::Every(n) = poll {
            if n != 120 && n != 300 {
                custom = format!("Every {n} s");
                options.push((poll, custom.as_str()));
            }
        }
        let me = cx.entity().downgrade();
        pane = pane.child(field(
            t,
            "Usage checks",
            "Automatic checks every 3 minutes while you work and every 15 minutes otherwise (every 5 minutes \
             without hooks); other accounts every 10 minutes. Claude Code's usage service is rate-limited, so an \
             account is never checked twice within 2 minutes. Checks never start a session window.",
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
        let hb = self.settings.heartbeat_interval;
        let me = cx.entity().downgrade();
        pane = pane.child(field(
            t,
            "Keep-alive",
            "Resends the last numbers so the device doesn't mark them stale between checks.",
            segmented(t, "hb", &[(60u64, "60 s"), (30u64, "30 s")], &hb, true, move |n, _, cx| {
                let _ = me.update(cx, |v, cx| {
                    v.save(|s, _| s.heartbeat_interval = n);
                    cx.notify();
                });
            }),
        ));
        let claude = self.snap.as_ref().and_then(|s| s.claude.clone());
        pane = pane.child(field(
            t,
            "Claude Code CLI",
            match &claude {
                Some(c) => format!("Using {}", tilde(c, &self.paths.home)),
                None if self.snap.is_some() => "Not found. Set claude_path in the config file.".into(),
                None => "Shown while Clawdmeter is running.".into(),
            },
            div(),
        ));
        let log = self.paths.state_dir.join("daemon.log");
        pane = pane.child(field(
            t,
            "Log file",
            tilde(&log.display().to_string(), &self.paths.home),
            button(t, "open-log", "Open", ButtonKind::Small, log.exists(), move |_, _, cx| cx.open_with_system(&log)),
        ));
        let cfgp = self.paths.config_file.clone();
        pane = pane.child(field(
            t,
            "Config file",
            tilde(&cfgp.display().to_string(), &self.paths.home),
            button(t, "open-cfg", "Open", ButtonKind::Small, cfgp.exists(), move |_, _, cx| cx.open_with_system(&cfgp)),
        ));
        let p = self.paths.clone();
        pane = pane.child(field(
            t,
            "Quit Clawdmeter",
            "Stops the background part and closes this window. The device keeps its last numbers until they go stale.",
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
                "The old background service is still installed",
                format!(
                    "{} talks to the device too, and the two will fight over it.",
                    tilde(&legacy.describe(), &self.paths.home)
                ),
                button(t, "legacy", "Turn it off", ButtonKind::Primary, true, move |_, _, cx| {
                    autostart::disable_legacy(&home);
                    let _ = me.update(cx, |v, cx| {
                        v.reload();
                        cx.notify();
                    });
                }),
            ));
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
        click(cx, "poll-2"); // Every 5 min
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
