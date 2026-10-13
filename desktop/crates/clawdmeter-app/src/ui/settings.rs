//! The settings window (spec §03): one window instead of the three install
//! scripts. Tabs: Device, Accounts, Hooks, Alerts, Background.
//!
//! It edits the same config file the daemon reads (through
//! `ConfigFile`/`Settings`, which keep comments, unknown keys and hand-written
//! values), so a hand-edited config and the GUI never disagree. The daemon
//! re-reads the file every few seconds.

use std::path::{Path, PathBuf};
use std::time::Duration;

use clawdmeter_core::autostart::{self, LaunchMode, LEGACY_LAUNCH_AGENT_LABEL, LEGACY_SYSTEMD_UNIT};
use clawdmeter_core::config::{ClockMode, ConfigFile};
use clawdmeter_core::hooks::{self, HookPlan, HookStatus, HOOKS};
use clawdmeter_core::{Paths, Settings};
use clawdmeter_flash::device::{self, FirmwareInfo};
use gpui::{div, prelude::*, px, Context, PathPromptOptions, SharedString, Window};

use super::theme::{Theme, FONT_MONO, R_M, R_S};
use super::widgets::{
    board_thumb, button, card, dot, field, heading, kv, para, pill, segmented, toggle, ButtonKind, Tone,
};
use super::{chrome, command, live_status, not_running_text, run_blocking, REFRESH};
use crate::boards;
use crate::diff::{line_diff, with_context, DiffLine};
use crate::hookscript;
use crate::ipc::{self, AppCommand, FirmwareRecord, StatusSnapshot};
use crate::tilde;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Device,
    Accounts,
    Hooks,
    Alerts,
    Background,
}

const TABS: [(Tab, &str); 5] = [
    (Tab::Device, "Device"),
    (Tab::Accounts, "Accounts"),
    (Tab::Hooks, "Hooks"),
    (Tab::Alerts, "Alerts"),
    (Tab::Background, "Background"),
];

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

/// The settings window.
pub struct SettingsView {
    paths: Paths,
    tab: Tab,
    snap: Option<StatusSnapshot>,
    settings: Settings,
    config_error: Option<String>,
    hooks: Vec<HookRow>,
    preview: Option<HookPreview>,
    hook_msg: Option<(Tone, String)>,
    jq: bool,
    device: DeviceState,
    autostart_on: bool,
    legacy: Option<PathBuf>,
    notice: Option<(Tone, String)>,
    ticks: u32,
}

fn legacy_entry(home: &Path) -> Option<PathBuf> {
    let p = if cfg!(target_os = "macos") {
        home.join("Library/LaunchAgents").join(format!("{LEGACY_LAUNCH_AGENT_LABEL}.plist"))
    } else if cfg!(windows) {
        return None;
    } else {
        home.join(".config/systemd/user").join(LEGACY_SYSTEMD_UNIT)
    };
    p.exists().then_some(p)
}

impl SettingsView {
    /// Build, load the config, and start probing for a USB-attached device.
    pub fn new(paths: Paths, tab: Option<&str>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tab =
            TABS.iter().find(|(_, l)| tab.is_some_and(|t| l.eq_ignore_ascii_case(t))).map_or(Tab::Device, |(t, _)| *t);
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
            tab,
            snap: None,
            config_error: None,
            hooks: Vec::new(),
            preview: None,
            hook_msg: None,
            jq: hookscript::jq_available(),
            device: DeviceState::default(),
            autostart_on: false,
            legacy: None,
            notice: None,
            ticks: 0,
        };
        v.reload();
        v.probe_device(cx);
        v
    }

    fn reload(&mut self) {
        match Settings::load(&self.paths) {
            Ok(s) => {
                self.settings = s;
                self.config_error = None;
            }
            Err(e) => self.config_error = Some(format!("{}: {e}", self.paths.config_file.display())),
        }
        self.hooks = self
            .settings
            .config_dirs
            .iter()
            .map(|d| HookRow { dir: d.clone(), status: hooks::dir_status(d) })
            .collect();
        self.autostart_on = autostart::is_enabled(&self.paths.home, LaunchMode::Tray);
        self.legacy = legacy_entry(&self.paths.home);
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
        // Notice a board being plugged in.
        if self.ticks.is_multiple_of(4) && !self.device.probing && self.device.port.is_none() {
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
            Err(e) => self.notice = Some((Tone::Bad, format!("Couldn't save {}: {e}", path.display()))),
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
                        v.device.error = Some(
                            "The device didn't answer. Is it running Clawdmeter firmware with minimal mode?".into(),
                        )
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
        // The script itself is written when the change is applied.
        let script = (action == HookAction::Install).then(|| hookscript::script_path(&self.paths));
        let mut plans = Vec::new();
        for d in dirs {
            let plan = match &script {
                Some(s) => hooks::plan_install(&d, s),
                None => hooks::plan_uninstall(&d),
            };
            match plan {
                Ok(p) if p.changes() => plans.push(p),
                Ok(_) => {}
                Err(e) => {
                    self.hook_msg = Some((Tone::Bad, format!("{}: {e}", d.join("settings.json").display())));
                    return;
                }
            }
        }
        if plans.is_empty() {
            self.hook_msg = Some((Tone::Ok, "Nothing to change.".into()));
        } else {
            self.preview = Some(HookPreview { action, plans });
        }
    }

    fn apply_hooks(&mut self) {
        let Some(p) = self.preview.take() else { return };
        if p.action == HookAction::Install {
            if let Err(e) = hookscript::install_script(&self.paths) {
                self.hook_msg = Some((Tone::Bad, format!("Couldn't write the hook script: {e}")));
                return;
            }
        }
        let mut done = 0;
        for plan in &p.plans {
            if let Err(e) = plan.apply() {
                self.hook_msg = Some((Tone::Bad, format!("{}: {e}", plan.path.display())));
                self.reload();
                return;
            }
            done += 1;
        }
        let verb = if p.action == HookAction::Install { "Installed in" } else { "Removed from" };
        self.hook_msg = Some((
            Tone::Ok,
            format!(
                "{verb} {done} account{}. The previous settings.json is kept as settings.json.bak.",
                if done == 1 { "" } else { "s" }
            ),
        ));
        self.reload();
    }

    fn send_test_alert(&mut self) {
        let path = self.paths.attention_file();
        let body = serde_json::json!({"m": "Test alert from Clawdmeter settings", "s": "clawdmeter-test"});
        self.hook_msg = Some(
            match std::fs::create_dir_all(&self.paths.state_dir).and_then(|_| std::fs::write(&path, body.to_string())) {
                Ok(()) => {
                    (Tone::Ok, "Test alert sent. Clawd should come to fetch you; tap the device to dismiss.".into())
                }
                Err(e) => (Tone::Bad, format!("{}: {e}", path.display())),
            },
        );
    }

    // ---------------------------------------------------------------- panes

    fn device_pane(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let snap = self.snap.clone();
        let rec = ipc::load_firmware_record(&self.paths);
        let env = self.device.info.as_ref().map(|i| i.env.clone()).or_else(|| rec.as_ref().map(|r| r.env.clone()));
        let ver = self.device.info.as_ref().map(|i| i.ver.clone()).or_else(|| rec.as_ref().map(|r| r.ver.clone()));
        let board = env.as_deref().and_then(boards::board);
        let thumb = board.map(|b| b.thumb(96.)).unwrap_or((80., 80.));
        let board_text: SharedString = match (board, &env) {
            (Some(b), _) => format!("{} · {}", b.name, b.chip.trim_start_matches("ESP32-")).into(),
            (None, Some(e)) => e.clone().into(),
            (None, None) => "Unknown (plug it in over USB to identify it)".into(),
        };
        let update = snap.as_ref().and_then(|s| s.update_available().map(str::to_owned));
        let link: SharedString = match snap.as_ref() {
            None => "Daemon not running".into(),
            Some(s) => match s.link.as_str() {
                "connected" => format!("Bluetooth · {}", s.device_key.clone().unwrap_or_default()).into(),
                "no_adapter" => format!("Bluetooth unavailable ({})", s.link_detail.clone().unwrap_or_default()).into(),
                _ => "Searching for \"Clawdmeter\"…".into(),
            },
        };
        let port_text: SharedString = match (&self.device.port, self.device.probing) {
            (_, true) => "Checking USB…".into(),
            (Some(p), _) => p.clone().into(),
            (None, _) => "Not plugged in over USB".into(),
        };
        let mut fw_row = div().flex().items_center().gap(px(8.)).child(ver.clone().unwrap_or_else(|| "–".into()));
        if let Some(u) = &update {
            fw_row = fw_row.child(pill(t, format!("{u} available"), Tone::Warn));
        }

        let mut pane = div().flex().flex_col().gap(px(14.)).child(heading(t, "Device"));
        pane = pane.child(
            card(t).flex_row().items_center().gap(px(18.)).child(board_thumb(t, thumb, 104.)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .flex_1()
                    .child(kv(t, vec![("Board", board_text), ("Link", link), ("USB", port_text)]))
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(div().w(px(76.)).text_size(px(13.)).text_color(t.mute).child("Firmware"))
                            .child(div().text_size(px(13.)).child(fw_row)),
                    ),
            ),
        );
        if let Some(u) = update {
            let p = self.paths.clone();
            pane = pane.child(
                card(t)
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .border_1()
                    .border_color(t.warn)
                    .child(div().flex().flex_col().gap(px(4.)).child(format!("Firmware {u} is available")).child(
                        div().text_size(px(12.5)).text_color(t.mute).child(format!(
                            "You have {}. The flasher downloads it from GitHub Releases and checks its hash.",
                            ver.unwrap_or_else(|| "an older version".into())
                        )),
                    ))
                    .child(button(t, "update-now", "Update now", ButtonKind::Primary, true, move |_, _, _| {
                        command(&p, AppCommand::Open("flasher".into()))
                    })),
            );
        }

        // Minimal display (sent over USB).
        let minimal_sel = self.device.minimal.as_ref().map(|(_, o)| o.clone()).unwrap_or_else(|| "auto".into());
        let can_minimal = self.device.port.is_some() && self.device.minimal.is_some() && !self.device.busy;
        let me = cx.entity().downgrade();
        let minimal_help = match (&self.device.minimal, &self.device.port) {
            (Some((eff, _)), _) => format!(
                "Still Clawd, meters only, no animation. Always on for e-ink panels. Now {}.",
                if *eff { "on" } else { "off" }
            ),
            (None, Some(_)) if self.device.probing => "Asking the device…".into(),
            (None, Some(_)) => "This firmware doesn't support minimal mode yet; update it first.".into(),
            (None, None) => "Still Clawd, meters only, no animation. Plug the device in over USB to change it.".into(),
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
            "Clock on the device",
            "Shows the time in the header instead of \"Usage\". Auto follows this computer's 12/24-hour setting.",
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

        let p = self.paths.clone();
        pane = pane.child(field(
            t,
            "Forget this device",
            "Clears the cached address and looks again. Use it after swapping boards.",
            button(t, "forget", "Forget", ButtonKind::Small, true, move |_, _, _| command(&p, AppCommand::Forget)),
        ));
        let me = cx.entity().downgrade();
        let on = self.settings.update_check;
        pane = pane.child(field(
            t,
            "Check for firmware updates",
            "Once a day, against the project's latest GitHub Release. The device reports its version with the info command.",
            toggle(t, "update-check", on, true, move |_, _, cx| {
                let _ = me.update(cx, |v, cx| {
                    v.save(|s, _| s.update_check = !on);
                    cx.notify();
                });
            }),
        ));
        let (p1, p2) = (self.paths.clone(), self.paths.clone());
        pane.child(
            div()
                .flex()
                .gap(px(8.))
                .child(button(t, "check-now", "Check now", ButtonKind::Normal, true, move |_, _, _| {
                    command(&p1, AppCommand::CheckUpdate)
                }))
                .child(button(t, "flash-new", "Flash a new device…", ButtonKind::Normal, true, move |_, _, _| {
                    command(&p2, AppCommand::Open("flasher".into()))
                })),
        )
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
                "Add config folder",
                ButtonKind::Small,
                true,
                move |_, _, cx| {
                    let _ = me.update(cx, |v, cx| v.add_account(cx));
                },
            )))
            .child(para(
                t,
                "Each Claude Code config folder is one account (CLAUDE_CONFIG_DIR). All are polled with claude /usage; \
                 the device shows the one whose usage moved most recently.",
            ));
        let active = self.snap.as_ref().and_then(|s| s.account.clone());
        let errors: Vec<(String, String)> = self.snap.as_ref().map(|s| s.errors.clone()).unwrap_or_default();
        let only_one = self.settings.config_dirs.len() <= 1;
        let mut list = div().flex().flex_col().rounded(px(R_M)).border_1().border_color(t.line);
        for (i, d) in self.settings.config_dirs.clone().into_iter().enumerate() {
            let ds = d.display().to_string();
            let err = errors.iter().find(|(p, _)| *p == ds).map(|(_, e)| e.clone());
            let is_active = active.as_deref() == Some(ds.as_str());
            let (tone, meta, chip) = match (&err, is_active) {
                (Some(e), _) => (Tone::Bad, e.clone(), pill(t, "Error", Tone::Bad).into_any_element()),
                (None, true) => (
                    Tone::Ok,
                    "Polling · shown on the device".to_string(),
                    pill(t, "On device", Tone::Accent).into_any_element(),
                ),
                (None, false) => {
                    (Tone::Plain, "Polling".to_string(), pill(t, "Polling", Tone::Plain).into_any_element())
                }
            };
            let me = cx.entity().downgrade();
            let dir = d.clone();
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(14.))
                    .py(px(10.))
                    .when(i > 0, |r| r.border_t_1().border_color(t.line))
                    .child(dot(t, tone))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(div().font_family(FONT_MONO).text_size(px(13.)).child(tilde(&ds, &self.paths.home)))
                            .child(div().text_size(px(12.)).text_color(t.mute).truncate().child(meta)),
                    )
                    .child(chip)
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
            pane = pane.child(div().text_size(px(12.5)).text_color(t.mute).child(not_running_text()));
        }
        pane
    }

    fn hooks_pane(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut pane = div().flex().flex_col().gap(px(14.)).child(heading(t, "Attention hooks")).child(para(
            t,
            "Clawd notices when you send a prompt, keeps up with tools, and comes to find you when Claude Code is \
             waiting. That's five small hooks in each account's settings.json. They record the event type and session \
             only. Without them Clawdmeter still works, polling every 2 minutes.",
        ));
        if !self.jq {
            pane = pane.child(
                card(t).border_1().border_color(t.warn).child(div().text_size(px(13.)).text_color(t.warn).child(
                    "The hook script needs jq on PATH (apt install jq / brew install jq). Without it the hooks run but record nothing.",
                )),
            );
        }
        let mut list = div().flex().flex_col().rounded(px(R_M)).border_1().border_color(t.line);
        for (i, row) in self.hooks.iter().enumerate() {
            let (tone, meta, chip, label, action) = match row.status {
                HookStatus::Installed => (
                    Tone::Ok,
                    format!("{n} of {n} hooks installed", n = HOOKS.len()),
                    pill(t, "Installed", Tone::Ok),
                    "Remove",
                    HookAction::Remove,
                ),
                HookStatus::Partial => (
                    Tone::Warn,
                    "Some hooks missing or from an older version".into(),
                    pill(t, "Update", Tone::Warn),
                    "Update",
                    HookAction::Install,
                ),
                HookStatus::Missing => (
                    Tone::Plain,
                    "Not installed · other hooks are kept".into(),
                    pill(t, "Missing", Tone::Warn),
                    "Install",
                    HookAction::Install,
                ),
            };
            let me = cx.entity().downgrade();
            let dir = row.dir.clone();
            let kind = if action == HookAction::Install { ButtonKind::Primary } else { ButtonKind::Small };
            list =
                list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .px(px(14.))
                        .py(px(10.))
                        .when(i > 0, |r| r.border_t_1().border_color(t.line))
                        .child(dot(t, tone))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.))
                                .child(div().font_family(FONT_MONO).text_size(px(13.)).child(tilde(
                                    &row.dir.join("settings.json").display().to_string(),
                                    &self.paths.home,
                                )))
                                .child(div().text_size(px(12.)).text_color(t.mute).child(meta)),
                        )
                        .child(chip)
                        .child(button(
                            t,
                            SharedString::from(format!("hook-{i}")),
                            label,
                            kind,
                            self.preview.is_none(),
                            move |_, _, cx| {
                                let dir = dir.clone();
                                let _ = me.update(cx, |v, cx| {
                                    v.plan_hooks(action, vec![dir]);
                                    cx.notify();
                                });
                            },
                        )),
                );
        }
        pane = pane.child(list);

        if let Some(p) = &self.preview {
            for plan in &p.plans {
                let diff = with_context(line_diff(plan.before.as_deref().unwrap_or(""), &plan.after), 2);
                let mut block = div()
                    .flex()
                    .flex_col()
                    .p(px(12.))
                    .rounded(px(R_S))
                    .bg(t.bg)
                    .border_1()
                    .border_color(t.line)
                    .font_family(FONT_MONO)
                    .text_size(px(12.));
                for l in diff {
                    block = block.child(match l {
                        DiffLine::Same(s) => div().text_color(t.mute).child(format!("  {s}")),
                        DiffLine::Added(s) => div().text_color(t.ok).bg(with_alpha(t.ok, 0.10)).child(format!("+ {s}")),
                        DiffLine::Removed(s) => {
                            div().text_color(t.bad).bg(with_alpha(t.bad, 0.10)).child(format!("- {s}"))
                        }
                    });
                }
                let verb = if p.action == HookAction::Install { "Will change" } else { "Will remove our hooks from" };
                pane = pane
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(t.mute)
                            .child(format!("{verb} {}", tilde(&plan.path.display().to_string(), &self.paths.home))),
                    )
                    .child(block);
            }
            let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
            let n = p.plans.len();
            pane = pane.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(button(
                        t,
                        "apply-hooks",
                        format!("Write {n} settings.json{} (keeps a .bak)", if n == 1 { "" } else { " files" }),
                        ButtonKind::Primary,
                        true,
                        move |_, _, cx| {
                            let _ = me1.update(cx, |v, cx| {
                                v.apply_hooks();
                                cx.notify();
                            });
                        },
                    ))
                    .child(button(t, "cancel-hooks", "Cancel", ButtonKind::Normal, true, move |_, _, cx| {
                        let _ = me2.update(cx, |v, cx| {
                            v.preview = None;
                            cx.notify();
                        });
                    })),
            );
        } else {
            let missing: Vec<PathBuf> =
                self.hooks.iter().filter(|h| h.status != HookStatus::Installed).map(|h| h.dir.clone()).collect();
            let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
            let n = missing.len();
            pane = pane.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .when(n > 0, |d| {
                        d.child(button(
                            t,
                            "install-all",
                            format!("Install in {n} account{}", if n == 1 { "" } else { "s" }),
                            ButtonKind::Primary,
                            true,
                            move |_, _, cx| {
                                let missing = missing.clone();
                                let _ = me1.update(cx, |v, cx| {
                                    v.plan_hooks(HookAction::Install, missing);
                                    cx.notify();
                                });
                            },
                        ))
                    })
                    .child(button(t, "test-alert", "Send a test alert", ButtonKind::Normal, true, move |_, _, cx| {
                        let _ = me2.update(cx, |v, cx| {
                            v.send_test_alert();
                            cx.notify();
                        });
                    })),
            );
        }
        if let Some((tone, m)) = &self.hook_msg {
            pane =
                pane.child(div().text_size(px(13.)).text_color(super::widgets::tone_color(t, *tone)).child(m.clone()));
        }
        pane
    }

    fn alerts_pane(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
        let (beep, chime) = (self.settings.beep, self.settings.chime);
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(heading(t, "Alerts"))
            .child(field(
                t,
                "Beep when Claude needs you",
                "Off by default. Plays on boards with a speaker. The tray can mute it for a while.",
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
            ))
    }

    fn background_pane(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let me = cx.entity().downgrade();
        let on = self.autostart_on;
        let home = self.paths.home.clone();
        let mut pane = div().flex().flex_col().gap(px(6.)).child(heading(t, "Background")).child(field(
            t,
            "Start at login",
            "Windows: Run key · macOS: LaunchAgent · Linux: XDG autostart entry.",
            toggle(t, "autostart", on, true, move |_, _, cx| {
                let home = home.clone();
                let _ = me.update(cx, |v, cx| {
                    let r = match std::env::current_exe() {
                        Ok(exe) if !on => autostart::enable(&home, &exe, LaunchMode::Tray),
                        Ok(_) => autostart::disable(&home, LaunchMode::Tray),
                        Err(e) => Err(e),
                    };
                    if let Err(e) = r {
                        v.notice = Some((Tone::Bad, format!("Start at login: {e}")));
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
            "Automatic: every 60 s while you're working, every 15 min otherwise, using claude /usage. Nothing is sent \
             to the model, so checks never start a window. Without the hooks: every 2 min.",
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
            "Keep-alive every",
            "Resends the last numbers so the device doesn't go stale between checks.",
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
                Some(c) => format!("Using {c}"),
                None if self.snap.is_some() => "Not found. Set claude_path in the config file.".into(),
                None => "Shown while the tray app is running.".into(),
            },
            div(),
        ));
        let log = self.paths.state_dir.join("daemon.log");
        pane = pane.child(field(
            t,
            "Daemon log",
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
        if let Some(legacy) = self.legacy.clone() {
            let me = cx.entity().downgrade();
            let home = self.paths.home.clone();
            pane = pane.child(field(
                t,
                "The old daemon is still installed",
                format!(
                    "{} also drives the device and will fight this app for it.",
                    tilde(&legacy.display().to_string(), &self.paths.home)
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
        let pending_hooks = self.hooks.iter().filter(|h| h.status != HookStatus::Installed).count();
        let mut tabs = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .w(px(168.))
            .flex_shrink_0()
            .p(px(12.))
            .border_r_1()
            .border_color(t.line);
        for (tab, label) in TABS {
            let on = tab == self.tab;
            let me = cx.entity().downgrade();
            tabs = tabs.child(
                div()
                    .id(label)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .py(px(7.))
                    .rounded(px(R_S))
                    .cursor_pointer()
                    .text_color(if on { t.text } else { t.dim })
                    .when(on, |d| d.bg(t.panel))
                    .hover(|s| s.bg(t.panel))
                    .on_click(move |_, _, cx| {
                        let _ = me.update(cx, |v, cx| {
                            v.tab = tab;
                            cx.notify();
                        });
                    })
                    .child(label)
                    .when(tab == Tab::Hooks && pending_hooks > 0, |d| {
                        d.child(
                            div()
                                .px(px(7.))
                                .rounded(px(999.))
                                .bg(t.accent)
                                .text_color(t.accent_ink)
                                .text_size(px(11.))
                                .child(pending_hooks.to_string()),
                        )
                    }),
            );
        }
        let pane = match self.tab {
            Tab::Device => self.device_pane(&t, cx),
            Tab::Accounts => self.accounts_pane(&t, cx),
            Tab::Hooks => self.hooks_pane(&t, cx),
            Tab::Alerts => self.alerts_pane(&t, cx),
            Tab::Background => self.background_pane(&t, cx),
        };
        let mut content = div().id("pane").flex_1().min_w_0().overflow_y_scroll().p(px(24.)).child(pane);
        if let Some(e) = self.config_error.clone().or_else(|| self.notice.as_ref().map(|(_, m)| m.clone())) {
            content = content.child(div().pt(px(12.)).text_size(px(13.)).text_color(t.bad).child(e));
        }
        let body = div().size_full().flex().bg(t.surface).child(tabs.bg(t.bg)).child(content);
        chrome(&t, "Clawdmeter settings", window, body)
    }
}
