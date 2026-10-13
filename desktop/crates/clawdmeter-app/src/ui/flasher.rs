//! The flasher (spec §02): plug in, confirm the board, flash.
//!
//! Steps: Connect → Identify → Firmware → Flash → Set up. Detection is the
//! three probes from the spec, cheapest first: the firmware's `info` command,
//! then chip and flash size from the ROM, then the user picks among what's
//! left, shown at true aspect ratio. Images come from the latest GitHub
//! Release (or a local release folder with a manifest.json) and are checked
//! against the manifest's sha256 before anything is written.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clawdmeter_core::{Paths, Settings};
use clawdmeter_flash::device::{self, ChipInfo, DeviceError, FirmwareInfo, FlashProgress, Identification};
use clawdmeter_flash::release::{self, Release};
use clawdmeter_flash::{BoardImage, Manifest};
use futures::StreamExt as _;
use gpui::{div, prelude::*, px, Context, PathPromptOptions, SharedString, Window};

use super::theme::{Theme, FONT_DISPLAY, FONT_MONO, R_M, R_PILL, R_S};
use super::widgets::{bar, board_thumb, button, caption, card, clawd, kv, notice, para, pill, ButtonKind, Tone};
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
    fn label(&self, home: &std::path::Path) -> String {
        match self {
            Source::Remote(_) => "GitHub".into(),
            Source::Local { dir, .. } => crate::tilde(&dir.display().to_string(), home),
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

/// Read a local release folder's manifest.json.
fn load_local(path: &std::path::Path) -> Result<Source, (String, bool)> {
    std::fs::read_to_string(path)
        .map_err(|e| format!("Couldn't open {}. {}", path.display(), crate::errors::io_message(&e)))
        .and_then(|s| {
            Manifest::parse(&s).map_err(|e| {
                log::warn!("{}: {e}", path.display());
                "That isn't a Clawdmeter release manifest.json.".to_string()
            })
        })
        .map(|manifest| Source::Local { dir: path.parent().map(PathBuf::from).unwrap_or_default(), manifest })
        .map_err(|m| (m, false))
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

/// What the flasher needs from USB. [`Usb`] is the real thing; tests drive
/// the window with a fake.
pub trait FlashHardware: Send + Sync + 'static {
    /// Attached Espressif ports.
    fn ports(&self) -> Vec<String>;
    /// Probe 1: ask running firmware who it is.
    fn probe_info(&self, port: &str) -> Option<FirmwareInfo>;
    /// Probe 2: chip and flash size from the ROM bootloader.
    fn probe_chip(&self, port: &str) -> Result<ChipInfo, DeviceError>;
    /// Write a merged image at 0x0 and reset into it.
    fn flash(
        &self,
        port: &str,
        image: &[u8],
        board: &BoardImage,
        progress: &mut dyn FnMut(FlashProgress),
    ) -> Result<(), DeviceError>;
}

/// The board on USB, through `clawdmeter_flash::device`.
pub struct Usb;

impl FlashHardware for Usb {
    fn ports(&self) -> Vec<String> {
        device::list_ports().into_iter().map(|p| p.path).collect()
    }
    fn probe_info(&self, port: &str) -> Option<FirmwareInfo> {
        device::probe_info(port, Duration::from_millis(1500)).ok().flatten()
    }
    fn probe_chip(&self, port: &str) -> Result<ChipInfo, DeviceError> {
        device::probe_chip(port)
    }
    fn flash(
        &self,
        port: &str,
        image: &[u8],
        board: &BoardImage,
        progress: &mut dyn FnMut(FlashProgress),
    ) -> Result<(), DeviceError> {
        device::flash_merged(port, image, board, progress)
    }
}

/// The flasher window.
pub struct FlasherView {
    paths: Paths,
    hw: Arc<dyn FlashHardware>,
    step: Step,
    /// `None` while checking; `Err((message, no_release_yet))`.
    source: Option<Result<Source, (String, bool)>>,
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
    /// `manifest`: a local release `manifest.json` to use instead of GitHub.
    pub fn new(paths: Paths, manifest: Option<PathBuf>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_hardware(paths, manifest, Arc::new(Usb), cx)
    }

    /// [`FlasherView::new`] with the USB side swapped out.
    pub fn with_hardware(
        paths: Paths,
        manifest: Option<PathBuf>,
        hw: Arc<dyn FlashHardware>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(REFRESH).await;
            if this.update(cx, |v, cx| v.tick(cx)).is_err() {
                break;
            }
        })
        .detach();
        let mut v = FlasherView {
            paths,
            hw,
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
        match manifest {
            Some(m) => v.source = Some(load_local(&m)),
            None => v.fetch_release(cx),
        }
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
                    log::warn!("release check for {repo}: {e} ({})", e.detail().unwrap_or_default());
                    let custom = repo != clawdmeter_flash::DEFAULT_RELEASE_REPO;
                    let none_yet = matches!(e, release::ReleaseError::NoRelease);
                    let msg = match (none_yet, custom) {
                        (true, true) => format!("{repo} hasn't published any firmware yet."),
                        _ => e.to_string(),
                    };
                    (msg, none_yet)
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
                    v.source = Some(load_local(&path));
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
            if let Some(p) = self.hw.ports().into_iter().next() {
                self.port = Some(p);
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
        let hw = self.hw.clone();
        run_blocking(
            cx,
            move || -> Result<(Option<FirmwareInfo>, Option<ChipInfo>, Identification), String> {
                let manifest = source.manifest();
                // Probe 1: running firmware names its env.
                if let Some(info) = hw.probe_info(&port) {
                    if manifest.board(&info.env).is_some() {
                        return Ok((Some(info.clone()), None, Identification::Firmware(info)));
                    }
                    log::info!("the board runs {}, which this release doesn't have", info.env);
                }
                // Probe 2: chip and flash size from the ROM.
                let chip = hw.probe_chip(&port).map_err(|e| e.to_string())?;
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
                                    "This board has an {}{}. None of the boards in this release match it.",
                                    device::pretty_chip(&c.chip),
                                    c.flash_mb.map(|m| format!(" with {m} MB of flash")).unwrap_or_default()
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
        cx.background_executor()
            .spawn(async move {
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
                            .map_err(|e| match e.kind() {
                                std::io::ErrorKind::NotFound => {
                                    format!("{} isn't next to the manifest.json.", board.file)
                                }
                                _ => format!("Couldn't read {}. {}", board.file, crate::errors::io_message(&e)),
                            })
                            .and_then(|d| release::verify(&board, &d).map(|_| d).map_err(|e| e.to_string()))
                    }
                };
                let _ = tx.unbounded_send(Err(r));
            })
            .detach();
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
            format!("Board on {port}"),
            format!("{} ({:.1} MB), checksum verified", board.file, data.len() as f32 / 1_048_576.),
        ];
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<FlashMsg>();
        let hw = self.hw.clone();
        cx.background_executor()
            .spawn(async move {
                let _ = tx.unbounded_send(FlashMsg::Log("Restarting the board into its bootloader…".into()));
                let mut total = data.len().max(1);
                let r = hw.flash(&port, &data, &board, &mut |p| {
                    let m = match p {
                        FlashProgress::Start { total: t } => {
                            total = t.max(1);
                            FlashMsg::Log(format!("Writing {:.1} MB…", t as f32 / 1_048_576.))
                        }
                        FlashProgress::Write { done } => FlashMsg::Progress(done as f32 / total as f32),
                        FlashProgress::Verify => FlashMsg::Log("Checking what was written…".into()),
                        FlashProgress::Done => FlashMsg::Log("Restarting into the new firmware".into()),
                    };
                    let _ = tx.unbounded_send(m);
                });
                let _ = tx.unbounded_send(FlashMsg::Finished(r.map_err(|e| e.to_string())));
            })
            .detach();
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
                            v.log.push("Stopped.".into());
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
        let (msg, none_yet) = match &self.source {
            None => return Some(div().flex().justify_center().child(caption(t, "Checking GitHub for firmware…"))),
            Some(Ok(_)) => return None,
            Some(Err(e)) => e.clone(),
        };
        let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
        let text = if none_yet {
            format!("{msg} If you have a release folder, open its manifest.json to flash from it.")
        } else {
            msg
        };
        Some(
            notice(t, if none_yet { Tone::Plain } else { Tone::Warn }, text).child(
                div()
                    .flex()
                    .gap(px(8.))
                    .pl(px(18.))
                    .child(button(t, "local", "Open manifest.json…", ButtonKind::Small, true, move |_, _, cx| {
                        let _ = me2.update(cx, |v, cx| v.open_local(cx));
                    }))
                    .child(button(t, "retry-release", "Check again", ButtonKind::Small, true, move |_, _, cx| {
                        let _ = me1.update(cx, |v, cx| v.fetch_release(cx));
                    })),
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
            (None, None) => String::new(),
        };
        let mut d = div().flex().flex_col().gap(px(12.)).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().font_family(FONT_DISPLAY).text_size(px(20.)).child("Which board is this?"))
                .when(self.probing || !chip_text.is_empty(), |d| {
                    let (text, tone) = if self.probing {
                        ("Asking the board…".to_string(), Tone::Accent)
                    } else {
                        (chip_text, Tone::Ok)
                    };
                    d.child(pill(t, text, tone))
                }),
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
            // Our short names read better side by side than the manifest's
            // full ones; unknown envs fall back to the manifest.
            let name: SharedString = info
                .map(|b| b.name.to_owned())
                .or_else(|| manifest.as_ref().and_then(|m| m.board(&env)).map(|b| b.display_name.clone()))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| env.clone())
                .into();
            let summary = info.map(|b| b.summary).unwrap_or("");
            let on = self.pick.as_deref() == Some(env.as_str());
            let me = cx.entity().downgrade();
            let e2 = env.clone();
            grid = grid.child(
                div()
                    .id(SharedString::from(format!("board-{i}")))
                    .debug_selector(|| format!("board-{i}"))
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
                    .child(board_thumb(t, info.map(|b| b.thumb(56.)).unwrap_or((40., 40.)), 64.))
                    .child(div().pt(px(4.)).text_size(px(13.5)).text_center().child(name))
                    .child(div().text_size(px(11.5)).text_color(t.mute).text_center().child(summary)),
            );
        }
        d = d.child(grid);
        let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
        d.child(
            div()
                .flex()
                .items_center()
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
        let home = self.paths.home.clone();
        // Where it comes from and how that's going sit on separate rows: a
        // local folder path is long, and the state must never be the part
        // that gets cut off.
        let from: SharedString = source.as_ref().map_or_else(|| "–".to_owned(), |s| s.label(&home)).into();
        let state: SharedString = match (&self.image, &source) {
            (None, Some(_)) => format!("Downloading… {:.0}%", self.download * 100.).into(),
            (Some(Ok(_)), Some(_)) => "Checksum verified".into(),
            (Some(Err(_)), Some(_)) => "Failed".into(),
            _ => "–".into(),
        };
        let title = match &self.image {
            None => "Getting the firmware",
            Some(Ok(_)) => "Ready to flash",
            Some(Err(_)) => "Couldn't get the firmware",
        };
        let mut d = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(div().font_family(FONT_DISPLAY).text_size(px(20.)).child(title))
            .child(
                card(t)
                    .flex_row()
                    .items_center()
                    .gap(px(18.))
                    .child(board_thumb(t, info.map(|b| b.thumb(96.)).unwrap_or((80., 80.)), 104.))
                    .child(kv(
                        t,
                        vec![
                            ("Board", info.map(|b| b.name.to_owned()).unwrap_or(env.clone()).into()),
                            (
                                "Firmware",
                                format!("{ver} · {}", board.as_ref().map(|b| b.file.clone()).unwrap_or_default())
                                    .into(),
                            ),
                            ("From", from),
                            ("Image", state),
                            ("USB", self.port.clone().unwrap_or_default().into()),
                        ],
                    )),
            );
        if let Some(Err(e)) = &self.image {
            d = d.child(notice(t, Tone::Bad, e.clone()));
        }
        let ready = matches!(self.image, Some(Ok(_)));
        let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
        d.child(
            div()
                .flex()
                .items_center()
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
        let mut d = div()
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
            .child(bar(t, self.progress, if self.step == Step::Done { t.ok } else { t.accent }))
            .child(log)
            .child(caption(
                t,
                "Keep it plugged in. If it stops, unplug it, hold BOOT while you plug it back in, and retry.",
            ));
        if self.error.is_some() && !self.flashing {
            let (me1, me2) = (cx.entity().downgrade(), cx.entity().downgrade());
            d = d.child(
                div()
                    .flex()
                    .items_center()
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
            .child(div().font_family(FONT_DISPLAY).text_size(px(22.)).child("Flashed. Now pair it."))
            .child(div().max_w(px(440.)).text_center().child(para(
                t,
                "It's restarting and will show up in your Bluetooth settings as Clawdmeter. Pair it once; \
                 after that the app finds it on its own.",
            )))
            .child(if self.connected {
                pill(t, "Connected", Tone::Ok)
            } else {
                pill(t, "Waiting for it to connect", Tone::Accent)
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
            body = body.child(notice(&t, Tone::Bad, e.clone()).child(div().flex().pl(px(18.)).child(button(
                &t,
                "err-retry",
                "Start over",
                ButtonKind::Small,
                true,
                move |_, _, cx| {
                    let _ = me.update(cx, |v, cx| {
                        v.restart();
                        cx.notify();
                    });
                },
            ))));
        }
        chrome(&t, "Flash your Clawdmeter", window, body)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    use super::*;

    /// A board on a fake port: an ESP32-S3 with 16 MB that doesn't run
    /// Clawdmeter yet. Records what gets flashed.
    #[derive(Default)]
    struct FakeBoard {
        flashed: Mutex<Option<(String, usize, String)>>,
    }

    impl FlashHardware for FakeBoard {
        fn ports(&self) -> Vec<String> {
            vec!["/dev/fake0".into()]
        }
        fn probe_info(&self, _port: &str) -> Option<FirmwareInfo> {
            None
        }
        fn probe_chip(&self, _port: &str) -> Result<ChipInfo, DeviceError> {
            Ok(ChipInfo { chip: "esp32s3".into(), flash_mb: Some(16), mac: None, features: vec![] })
        }
        fn flash(
            &self,
            port: &str,
            image: &[u8],
            board: &BoardImage,
            progress: &mut dyn FnMut(FlashProgress),
        ) -> Result<(), DeviceError> {
            progress(FlashProgress::Start { total: image.len() });
            progress(FlashProgress::Write { done: image.len() / 2 });
            progress(FlashProgress::Write { done: image.len() });
            progress(FlashProgress::Verify);
            progress(FlashProgress::Done);
            *self.flashed.lock().unwrap() = Some((port.into(), image.len(), board.env.clone()));
            Ok(())
        }
    }

    fn release_dir(tag: &str) -> (std::path::PathBuf, Paths) {
        let root = std::env::temp_dir().join(format!("cm-flasher-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let rel = root.join("release");
        std::fs::create_dir_all(&rel).unwrap();
        let image = vec![0xA5u8; 4096];
        std::fs::write(rel.join("waveshare_amoled_18-v1.9.0.bin"), &image).unwrap();
        let sha = release::sha256_hex(&image);
        let manifest = format!(
            r#"{{"version":"1.9.0","boards":[
              {{"env":"waveshare_amoled_216","chip":"esp32s3","flash_size":"8MB","display_name":"2.16","file":"waveshare_amoled_216-v1.9.0.bin","sha256":"00"}},
              {{"env":"waveshare_amoled_18","chip":"esp32s3","flash_size":"16MB","display_name":"1.8","file":"waveshare_amoled_18-v1.9.0.bin","sha256":"{sha}"}},
              {{"env":"waveshare_amoled_216_c6","chip":"esp32c6","flash_size":"16MB","display_name":"C6","file":"c6.bin","sha256":"00"}}]}}"#
        );
        std::fs::write(rel.join("manifest.json"), manifest).unwrap();
        (rel.join("manifest.json"), Paths::under(&root))
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        cx.run_until_parked();
        let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} isn't on screen"));
        cx.simulate_click(b.center(), Modifiers::none());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn walks_from_plug_in_to_done(cx: &mut TestAppContext) {
        let (manifest, paths) = release_dir("flow");
        let hw = Arc::new(FakeBoard::default());
        let hw2: Arc<dyn FlashHardware> = hw.clone();
        let p = paths.clone();
        let (view, cx) =
            cx.add_window_view(move |_, cx| FlasherView::with_hardware(p, Some(manifest.clone()), hw2, cx));

        // Connect → Identify: the tick notices the port and probes the chip.
        cx.executor().advance_clock(REFRESH * 3);
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(v.step == Step::Identify);
            assert_eq!(v.port.as_deref(), Some("/dev/fake0"));
            // 16 MB S3: the exact match first, the 8 MB 2.16 image still offered.
            assert_eq!(v.candidates, ["waveshare_amoled_18", "waveshare_amoled_216"]);
            assert!(v.pick.is_none());
        });

        // Continue stays disabled until a board is picked.
        click(cx, "continue");
        view.read_with(cx, |v, _| assert!(v.step == Step::Identify));
        click(cx, "board-0");
        click(cx, "continue");
        view.read_with(cx, |v, _| {
            assert!(v.step == Step::Firmware);
            assert!(matches!(v.image, Some(Ok(ref d)) if d.len() == 4096), "local image read and checksum-verified");
        });

        click(cx, "flash");
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(v.step == Step::Done, "log: {:?}, error: {:?}", v.log, v.error);
            assert_eq!(v.progress, 1.);
        });
        assert_eq!(
            hw.flashed.lock().unwrap().clone(),
            Some(("/dev/fake0".to_string(), 4096, "waveshare_amoled_18".to_string()))
        );
        let rec = ipc::load_firmware_record(&paths).unwrap();
        assert_eq!((rec.env.as_str(), rec.ver.as_str()), ("waveshare_amoled_18", "1.9.0"));
        let _ = std::fs::remove_dir_all(&paths.home);
    }

    #[gpui::test]
    fn missing_image_is_reported_and_blocks_flashing(cx: &mut TestAppContext) {
        let (manifest, paths) = release_dir("missing");
        let hw: Arc<dyn FlashHardware> = Arc::new(FakeBoard::default());
        let p = paths.clone();
        let (view, cx) = cx.add_window_view(move |_, cx| FlasherView::with_hardware(p, Some(manifest.clone()), hw, cx));
        cx.executor().advance_clock(REFRESH * 3);
        click(cx, "board-1");
        click(cx, "continue");
        view.read_with(cx, |v, _| match &v.image {
            Some(Err(e)) => assert_eq!(e, "waveshare_amoled_216-v1.9.0.bin isn't next to the manifest.json."),
            other => {
                panic!("expected a missing-file error, got {:?}", other.as_ref().map(|r| r.as_ref().map(Vec::len)))
            }
        });
        click(cx, "flash");
        view.read_with(cx, |v, _| assert!(v.step == Step::Firmware, "disabled Flash does nothing"));
        let _ = std::fs::remove_dir_all(&paths.home);
    }
}
