//! The flasher (spec §02): plug in, confirm the board, flash.
//!
//! Steps: Connect → Identify → Firmware → Flash → Set up. Detection is the
//! three probes from the spec, cheapest first: the firmware's `info` command,
//! then chip and flash size from the ROM, then the user picks among what's
//! left, shown at true aspect ratio. Images come from the latest GitHub
//! Release (or a local release folder with a manifest.json) and are checked
//! against the manifest's sha256 before anything is written.

use std::path::PathBuf;
use std::time::Duration;

use clawdmeter_core::{Paths, Settings};
use clawdmeter_flash::device::{self, ChipInfo, FirmwareInfo, FlashProgress, Identification};
use clawdmeter_flash::release::{self, Release};
use clawdmeter_flash::{BoardImage, Manifest};
use futures::StreamExt as _;
use gpui::{div, prelude::*, px, Context, PathPromptOptions, SharedString, Window};

use super::theme::{Theme, FONT_DISPLAY, FONT_MONO, R_M, R_PILL, R_S};
use super::widgets::{board_thumb, button, card, clawd, kv, para, pill, ButtonKind, Tone};
use super::{chrome, command, live_status, run_blocking, REFRESH};
use crate::boards;
use crate::ipc::{self, AppCommand, FirmwareRecord};

const STEPS: [&str; 5] = ["Connect", "Identify", "Firmware", "Flash", "Set up"];

/// Where images come from.
#[derive(Clone)]
enum Source {
    Remote(Release),
    Local { dir: PathBuf, manifest: Manifest },
}

impl Source {
    fn manifest(&self) -> &Manifest {
        match self {
            Source::Remote(r) => &r.manifest,
            Source::Local { manifest, .. } => manifest,
        }
    }
    fn label(&self) -> String {
        match self {
            Source::Remote(_) => "GitHub Releases".into(),
            Source::Local { dir, .. } => dir.display().to_string(),
        }
    }
    fn version(&self) -> String {
        let v = &self.manifest().version;
        if v.starts_with('v') {
            v.clone()
        } else {
            format!("v{v}")
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Connect,
    Identify,
    Firmware,
    Flash,
    Done,
}

impl Step {
    fn index(self) -> usize {
        self as usize
    }
}

enum FlashMsg {
    Log(String),
    Progress(f32),
    Finished(Result<(), String>),
}

/// The flasher window.
pub struct FlasherView {
    paths: Paths,
    step: Step,
    source: Option<Result<Source, String>>,
    port: Option<String>,
    probing: bool,
    info: Option<FirmwareInfo>,
    chip: Option<ChipInfo>,
    candidates: Vec<String>,
    pick: Option<String>,
    image: Option<Result<Vec<u8>, String>>,
    download: f32,
    progress: f32,
    log: Vec<String>,
    flashing: bool,
    error: Option<String>,
    connected: bool,
    ticks: u32,
}

impl FlasherView {
    /// Build, fetch the release manifest, and start listening on USB.
    pub fn new(paths: Paths, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(REFRESH).await;
            if this.update(cx, |v, cx| v.tick(cx)).is_err() {
                break;
            }
        })
        .detach();
        let mut v = FlasherView {
            paths,
            step: Step::Connect,
            source: None,
            port: None,
            probing: false,
            info: None,
            chip: None,
            candidates: Vec::new(),
            pick: None,
            image: None,
            download: 0.,
            progress: 0.,
            log: Vec::new(),
            flashing: false,
            error: None,
            connected: false,
            ticks: 0,
        };
        v.fetch_release(cx);
        v
    }

    fn fetch_release(&mut self, cx: &mut Context<Self>) {
        self.source = None;
        let repo = Settings::load(&self.paths)
            .map(|s| s.release_repo)
            .unwrap_or_else(|_| clawdmeter_flash::DEFAULT_RELEASE_REPO.into());
        run_blocking(
            cx,
            move || {
                release::latest_release(&repo).map_err(|e| {
                    let e = e.to_string();
                    if e.contains("404") {
                        format!("{repo} has no published firmware release yet.")
                    } else {
                        format!("{repo}: {e}")
                    }
                })
            },
            |v, r, cx| {
                if let Ok(rel) = &r {
                    let _ = std::fs::create_dir_all(&v.paths.cache_dir);
                    let _ = std::fs::write(ipc::latest_release_file(&v.paths), &rel.manifest.version);
                }
                v.source = Some(r.map(Source::Remote));
                if v.step == Step::Identify && v.candidates.is_empty() && !v.probing {
                    v.identify(cx);
                }
            },
        );
    }

    fn open_local(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a release manifest.json".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = rx.await {
                let Some(path) = paths.pop() else { return };
                let _ = this.update(cx, |v, cx| {
                    let r = std::fs::read_to_string(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|s| Manifest::parse(&s).map_err(|e| e.to_string()))
                        .map(|manifest| Source::Local {
                            dir: path.parent().map(PathBuf::from).unwrap_or_default(),
                            manifest,
                        })
                        .map_err(|e| format!("{}: {e}", path.display()));
                    v.source = Some(r);
                    v.error = None;
                    if v.port.is_some() {
                        v.step = Step::Identify;
                        v.identify(cx);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.ticks += 1;
        if self.step == Step::Connect && !self.probing && self.ticks.is_multiple_of(2) {
            if let Some(p) = device::list_ports().into_iter().next() {
                self.port = Some(p.path);
                self.step = Step::Identify;
                self.identify(cx);
            }
        }
        if self.step == Step::Done {
            self.connected = live_status(&self.paths).is_some_and(|s| s.link == "connected");
        }
        cx.notify();
    }

    fn identify(&mut self, cx: &mut Context<Self>) {
        let Some(port) = self.port.clone() else { return };
        let Some(Ok(source)) = self.source.clone() else { return };
        self.probing = true;
        self.error = None;
        self.candidates.clear();
        self.pick = None;
        self.info = None;
        self.chip = None;
        run_blocking(
            cx,
            move || -> Result<(Option<FirmwareInfo>, Option<ChipInfo>, Identification), String> {
                let manifest = source.manifest();
                // Probe 1: running firmware names its env.
                if let Ok(Some(info)) = device::probe_info(&port, Duration::from_millis(1500)) {
                    if manifest.board(&info.env).is_some() {
                        return Ok((Some(info.clone()), None, Identification::Firmware(info)));
                    }
                }
                // Probe 2: chip and flash size from the ROM.
                let chip = device::probe_chip(&port).map_err(|e| e.to_string())?;
                let id = device::identify_from_chip(manifest, &chip);
                Ok((None, Some(chip), id))
            },
            |v, r, _| {
                v.probing = false;
                match r {
                    Err(e) => v.error = Some(e),
                    Ok((info, chip, id)) => {
                        v.chip = chip;
                        if let Some(i) = &info {
                            let _ = ipc::save_firmware_record(
                                &v.paths,
                                &FirmwareRecord { env: i.env.clone(), ver: i.ver.clone() },
                            );
                        }
                        v.info = info;
                        match id {
                            Identification::Firmware(i) => {
                                v.candidates = vec![i.env.clone()];
                                v.pick = Some(i.env);
                            }
                            Identification::Unique(env) => {
                                v.candidates = vec![env.clone()];
                                v.pick = Some(env);
                            }
                            Identification::Choose(envs) => v.candidates = envs,
                            Identification::Unsupported(c) => {
                                v.error = Some(format!(
                                    "No Clawdmeter board in this release matches an {} with {} flash.",
                                    c.chip,
                                    c.flash_mb.map(|m| format!("{m} MB")).unwrap_or_else(|| "unknown".into())
                                ))
                            }
                        }
                    }
                }
            },
        );
    }

    fn board_image(&self) -> Option<BoardImage> {
        let source = self.source.as_ref()?.as_ref().ok()?;
        source.manifest().board(self.pick.as_deref()?).cloned()
    }

    fn start_download(&mut self, cx: &mut Context<Self>) {
        let (Some(Ok(source)), Some(board)) = (self.source.clone(), self.board_image()) else { return };
        self.step = Step::Firmware;
        self.image = None;
        self.download = 0.;
        let cache = self.paths.cache_dir.clone();
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Result<f32, Result<Vec<u8>, String>>>();
        std::thread::spawn(move || {
            let r = match &source {
                Source::Remote(rel) => release::fetch_image(rel, &board, &cache, |done, total| {
                    if total > 0 {
                        let _ = tx.unbounded_send(Ok(done as f32 / total as f32));
                    }
                })
                .map_err(|e| e.to_string()),
                Source::Local { dir, .. } => {
                    let name = std::path::Path::new(&board.file).file_name().map(PathBuf::from).unwrap_or_default();
                    std::fs::read(dir.join(name))
                        .map_err(|e| format!("{}: {e}", board.file))
                        .and_then(|d| release::verify(&board, &d).map(|_| d).map_err(|e| e.to_string()))
                }
            };
            let _ = tx.unbounded_send(Err(r));
        });
        cx.spawn(async move |this, cx| {
            while let Some(m) = rx.next().await {
                let stop = m.is_err();
                let _ = this.update(cx, |v, cx| {
                    match m {
                        Ok(f) => v.download = f,
                        Err(r) => {
                            v.download = 1.;
                            v.image = Some(r);
                        }
                    }
                    cx.notify();
                });
                if stop {
                    break;
                }
            }
        })
        .detach();
    }

    fn start_flash(&mut self, cx: &mut Context<Self>) {
        let (Some(Ok(data)), Some(board), Some(port)) = (self.image.clone(), self.board_image(), self.port.clone())
        else {
            return;
        };
        let version = self.source.as_ref().and_then(|s| s.as_ref().ok()).map(Source::version).unwrap_or_default();
        self.step = Step::Flash;
        self.flashing = true;
        self.progress = 0.;
        self.error = None;
        self.log = vec![
            format!("Port {port}"),
            format!("Image {} ({:.1} MB), sha256 matches manifest.json", board.file, data.len() as f32 / 1_048_576.),
        ];
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<FlashMsg>();
        std::thread::spawn(move || {
            let _ = tx.unbounded_send(FlashMsg::Log("Connecting to the ROM bootloader…".into()));
            let mut total = data.len().max(1);
            let r = device::flash_merged(&port, &data, &board, |p| {
                let m = match p {
                    FlashProgress::Start { total: t } => {
                        total = t.max(1);
                        FlashMsg::Log(format!("Writing {t} bytes at 0x00000000…"))
                    }
                    FlashProgress::Write { done } => FlashMsg::Progress(done as f32 / total as f32),
                    FlashProgress::Verify => FlashMsg::Log("Verifying…".into()),
                    FlashProgress::Done => FlashMsg::Log("Resetting into the new firmware".into()),
                };
                let _ = tx.unbounded_send(m);
            });
            let _ = tx.unbounded_send(FlashMsg::Finished(r.map_err(|e| e.to_string())));
        });
        let env = self.pick.clone().unwrap_or_default();
        cx.spawn(async move |this, cx| {
            while let Some(m) = rx.next().await {
                let fin = matches!(m, FlashMsg::Finished(_));
                let env = env.clone();
                let version = version.clone();
                let _ = this.update(cx, |v, cx| {
                    match m {
                        FlashMsg::Log(l) => v.log.push(l),
                        FlashMsg::Progress(p) => v.progress = p,
                        FlashMsg::Finished(Ok(())) => {
                            v.flashing = false;
                            v.progress = 1.;
                            v.log.push("Done".into());
                            let ver = version.trim_start_matches('v').to_owned();
                            let _ = ipc::save_firmware_record(&v.paths, &FirmwareRecord { env, ver });
                            command(&v.paths, AppCommand::Reconnect);
                            v.step = Step::Done;
                        }
                        FlashMsg::Finished(Err(e)) => {
                            v.flashing = false;
                            v.log.push(format!("Failed: {e}"));
                            v.error = Some(e);
                        }
                    }
                    cx.notify();
                });
                if fin {
                    break;
                }
            }
        })
        .detach();
    }

    fn restart(&mut self) {
        self.step = Step::Connect;
        self.port = None;
        self.info = None;
        self.chip = None;
        self.candidates.clear();
        self.pick = None;
        self.image = None;
        self.error = None;
        self.log.clear();
        self.progress = 0.;
        self.ticks = 0;
    }

    // --------------------------------------------------------------- views

    fn steps(&self, t: &Theme) -> impl IntoElement {
        let mut row = div().flex().items_center().gap(px(6.)).flex_wrap();
        for (i, s) in STEPS.iter().enumerate() {
            let (done, on) = (i < self.step.index(), i == self.step.index());
            let c = if on {
                t.accent
            } else if done {
                t.ok
            } else {
                t.track
            };
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
                    .child(div().size(px(8.)).rounded(px(R_PILL)).bg(c))
                    .child(*s),
            );
        }
        row
    }

    fn source_problem(&self, t: &Theme, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let e = match &self.source {
            None => {
                return Some(
                    div().text_size(px(12.5)).text_color(t.mute).child("Checking GitHub for the latest firmware…"),
                )
            }
            Some(Ok(_)) => return None,
            Some(Err(e)) => e.clone(),
        };
        let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
        Some(
            card(t)
                .border_1()
                .border_color(t.warn)
                .child(
                    div().text_color(t.warn).text_size(px(13.)).child(format!("Couldn't get the latest release: {e}")),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(button(t, "retry-release", "Try again", ButtonKind::Small, true, move |_, _, cx| {
                            let _ = me1.update(cx, |v, cx| v.fetch_release(cx));
                        }))
                        .child(button(
                            t,
                            "local",
                            "Use a local manifest.json…",
                            ButtonKind::Small,
                            true,
                            move |_, _, cx| {
                                let _ = me2.update(cx, |v, cx| v.open_local(cx));
                            },
                        )),
                ),
        )
    }

    fn connect_view(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut d = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .pt(px(18.))
            .child(clawd(4.))
            .child(div().font_family(FONT_DISPLAY).text_size(px(22.)).child("Plug in your Clawdmeter"))
            .child(div().max_w(px(420.)).text_center().child(para(
                t,
                "Use a USB-C data cable. Charge-only cables are the most common reason nothing shows up.",
            )))
            .child(pill(t, "Listening on USB", Tone::Accent));
        if let Some(p) = self.source_problem(t, cx) {
            d = d.child(div().w_full().pt(px(8.)).child(p));
        }
        d
    }

    fn identify_view(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let chip_text = match (&self.info, &self.chip) {
            (Some(i), _) => format!("Running Clawdmeter {}", i.ver),
            (None, Some(c)) => format!(
                "{} · {}{}",
                c.chip.to_uppercase().replace("ESP32", "ESP32-"),
                c.flash_mb.map(|m| format!("{m} MB")).unwrap_or_else(|| "? MB".into()),
                if c.features.iter().any(|f| f.to_ascii_lowercase().contains("psram")) { " · PSRAM" } else { "" }
            ),
            (None, None) => "Probing…".into(),
        };
        let mut d = div().flex().flex_col().gap(px(12.)).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().font_family(FONT_DISPLAY).text_size(px(20.)).child("Which board is this?"))
                .child(pill(t, chip_text, if self.probing { Tone::Accent } else { Tone::Ok })),
        );
        if let Some(p) = self.source_problem(t, cx) {
            return d.child(p);
        }
        if self.probing {
            return d.child(para(t, "Asking the board who it is. If it isn't running Clawdmeter yet, it restarts briefly into its bootloader."));
        }
        let hint = match (&self.info, self.candidates.len()) {
            (Some(_), _) => "It already runs Clawdmeter, so it told us exactly which board it is.".to_string(),
            (None, 1) => "Only one board in this release fits this chip.".to_string(),
            (None, n) if n > 1 => format!("The chip narrows it to {n} boards. Pick the one on your desk."),
            _ => String::new(),
        };
        if !hint.is_empty() {
            d = d.child(para(t, hint));
        }
        let manifest = self.source.as_ref().and_then(|s| s.as_ref().ok()).map(|s| s.manifest().clone());
        let mut grid = div().flex().flex_wrap().gap(px(10.));
        for (i, env) in self.candidates.clone().into_iter().enumerate() {
            let info = boards::board(&env);
            let name: SharedString = manifest
                .as_ref()
                .and_then(|m| m.board(&env))
                .map(|b| b.display_name.clone())
                .filter(|n| !n.is_empty())
                .or_else(|| info.map(|b| b.name.to_owned()))
                .unwrap_or_else(|| env.clone())
                .into();
            let summary = info.map(|b| b.summary).unwrap_or("");
            let on = self.pick.as_deref() == Some(env.as_str());
            let me = cx.entity().downgrade();
            let e2 = env.clone();
            grid = grid.child(
                div()
                    .id(SharedString::from(format!("board-{i}")))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .w(px(124.))
                    .p(px(10.))
                    .rounded(px(R_M))
                    .border_1()
                    .border_color(if on { t.accent } else { t.line })
                    .when(on, |d| d.bg(t.panel))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.panel))
                    .on_click(move |_, _, cx| {
                        let env = e2.clone();
                        let _ = me.update(cx, |v, cx| {
                            v.pick = Some(env);
                            cx.notify();
                        });
                    })
                    .child(board_thumb(t, info.map(|b| b.thumb(56.)).unwrap_or((40., 40.)), 60.))
                    .child(div().text_size(px(13.5)).child(name))
                    .child(div().text_size(px(11.5)).text_color(t.mute).text_center().child(summary)),
            );
        }
        d = d.child(grid);
        let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
        d.child(
            div()
                .flex()
                .justify_between()
                .pt(px(4.))
                .child(button(t, "back0", "Back", ButtonKind::Small, true, move |_, _, cx| {
                    let _ = me1.update(cx, |v, cx| {
                        v.restart();
                        cx.notify();
                    });
                }))
                .child(button(t, "continue", "Continue", ButtonKind::Primary, self.pick.is_some(), move |_, _, cx| {
                    let _ = me2.update(cx, |v, cx| v.start_download(cx));
                })),
        )
    }

    fn firmware_view(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let board = self.board_image();
        let env = self.pick.clone().unwrap_or_default();
        let info = boards::board(&env);
        let source = self.source.as_ref().and_then(|s| s.as_ref().ok()).cloned();
        let ver = source.as_ref().map(Source::version).unwrap_or_default();
        let from: SharedString = match (&self.image, &source) {
            (None, Some(s)) => format!("{} · downloading {:.0}%", s.label(), self.download * 100.).into(),
            (Some(Ok(_)), Some(s)) => format!("{} · sha256 ok", s.label()).into(),
            (Some(Err(_)), _) => "download failed".into(),
            _ => "–".into(),
        };
        let mut d = div().flex().flex_col().gap(px(14.)).child(
            card(t)
                .flex_row()
                .items_center()
                .gap(px(18.))
                .child(board_thumb(t, info.map(|b| b.thumb(96.)).unwrap_or((80., 80.)), 104.))
                .child(kv(
                    t,
                    vec![
                        ("Board", info.map(|b| b.name.to_owned()).unwrap_or(env.clone()).into()),
                        ("Image", board.as_ref().map(|b| b.file.clone()).unwrap_or_default().into()),
                        ("From", from),
                        ("Port", self.port.clone().unwrap_or_default().into()),
                    ],
                )),
        );
        if let Some(Err(e)) = &self.image {
            d = d.child(div().text_size(px(13.)).text_color(t.bad).child(e.clone()));
        }
        let ready = matches!(self.image, Some(Ok(_)));
        let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
        d.child(
            div()
                .flex()
                .justify_between()
                .child(button(t, "back1", "Back", ButtonKind::Small, true, move |_, _, cx| {
                    let _ = me1.update(cx, |v, cx| {
                        v.step = Step::Identify;
                        cx.notify();
                    });
                }))
                .child(button(t, "flash", format!("Flash {ver}"), ButtonKind::Primary, ready, move |_, _, cx| {
                    let _ = me2.update(cx, |v, cx| v.start_flash(cx));
                })),
        )
    }

    fn flash_view(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let title = if self.error.is_some() {
            "Flashing failed"
        } else if self.flashing {
            "Writing firmware"
        } else {
            "Done"
        };
        let mut log = div()
            .id("log")
            .flex()
            .flex_col()
            .h(px(150.))
            .overflow_y_scroll()
            .p(px(12.))
            .rounded(px(R_S))
            .bg(t.bg)
            .border_1()
            .border_color(t.line)
            .font_family(FONT_MONO)
            .text_size(px(12.))
            .text_color(t.dim);
        for l in &self.log {
            log = log.child(l.clone());
        }
        let mut d =
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(div().font_family(FONT_DISPLAY).text_size(px(20.)).child(title))
                        .child(div().font_family(FONT_MONO).child(format!("{:.0}%", self.progress * 100.))),
                )
                .child(div().h(px(8.)).rounded(px(R_PILL)).bg(t.track).child(
                    div().h_full().w(gpui::relative(self.progress.clamp(0., 1.))).rounded(px(R_PILL)).bg(t.accent),
                ))
                .child(log)
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(t.mute)
                        .child("Keep it plugged in. If it fails, unplug, hold BOOT, plug back in and retry."),
                );
        if self.error.is_some() && !self.flashing {
            let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
            d = d.child(
                div()
                    .flex()
                    .justify_between()
                    .child(button(t, "start-over", "Start over", ButtonKind::Small, true, move |_, _, cx| {
                        let _ = me1.update(cx, |v, cx| {
                            v.restart();
                            cx.notify();
                        });
                    }))
                    .child(button(t, "retry", "Retry", ButtonKind::Primary, true, move |_, _, cx| {
                        let _ = me2.update(cx, |v, cx| v.start_flash(cx));
                    })),
            );
        }
        d
    }

    fn done_view(&mut self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let (me, p) = (cx.entity().downgrade(), self.paths.clone());
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .pt(px(12.))
            .child(clawd(4.))
            .child(div().font_family(FONT_DISPLAY).text_size(px(22.)).child("Flashed. Now connect it."))
            .child(div().max_w(px(440.)).text_center().child(para(
                t,
                "Clawdmeter is advertising over Bluetooth. Pair it once in your system's Bluetooth settings; \
                 the app then finds it by name and keeps the connection.",
            )))
            .child(if self.connected {
                pill(t, "Connected", Tone::Ok)
            } else {
                pill(t, "Searching for \"Clawdmeter\"", Tone::Accent)
            })
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(button(t, "again", "Flash another", ButtonKind::Small, true, move |_, _, cx| {
                        let _ = me.update(cx, |v, cx| {
                            v.restart();
                            cx.notify();
                        });
                    }))
                    .child(button(t, "settings", "Open settings", ButtonKind::Small, true, move |_, _, _| {
                        command(&p, AppCommand::Open("settings".into()))
                    })),
            )
    }
}

impl Render for FlasherView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::for_appearance(window.appearance());
        let view = match self.step {
            Step::Connect => self.connect_view(&t, cx),
            Step::Identify => self.identify_view(&t, cx),
            Step::Firmware => self.firmware_view(&t, cx),
            Step::Flash => self.flash_view(&t, cx),
            Step::Done => self.done_view(&t, cx),
        };
        let mut body = div()
            .id("flasher")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(18.))
            .p(px(24.))
            .child(self.steps(&t))
            .child(view);
        if let (Some(e), false) = (&self.error, self.step == Step::Flash) {
            let me = cx.entity().downgrade();
            body = body.child(
                card(&t)
                    .border_1()
                    .border_color(t.bad)
                    .child(div().text_size(px(13.)).text_color(t.bad).child(e.clone()))
                    .child(div().flex().child(button(
                        &t,
                        "err-retry",
                        "Try again",
                        ButtonKind::Small,
                        true,
                        move |_, _, cx| {
                            let _ = me.update(cx, |v, cx| {
                                v.restart();
                                cx.notify();
                            });
                        },
                    ))),
            );
        }
        chrome(&t, "Flash your Clawdmeter", window, body)
    }
}
