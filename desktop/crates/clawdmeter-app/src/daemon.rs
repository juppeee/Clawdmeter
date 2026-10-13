//! The daemon runtime: drives [`clawdmeter_core::Engine`] with real I/O
//! (the `claude` CLI, events.log, attention files, the BLE link).
//!
//! It runs on its own thread with a current-thread tokio runtime, so the GUI
//! event loop never shares a reactor with it. The UI talks to it through a
//! [`DaemonHandle`]: commands in, a [`Status`] snapshot out.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use clawdmeter_ble::{AddressCache, DeviceLink, LinkError, LinkOptions, RefreshRequests, MAX_WRITE};
use clawdmeter_core::attention::AttentionInbox;
use clawdmeter_core::events::EventTail;
use clawdmeter_core::usage::{self, UsageError, UsageReading};
use clawdmeter_core::{unix_now, Engine, EngineStatus, HostClock, Paths, Settings, WriteKind};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

/// Loop tick: events reach the device within about this long.
const TICK: Duration = Duration::from_millis(500);
/// Config re-read interval (s).
const SETTINGS_EVERY: i64 = 5;
/// Connection check interval (s).
const LINK_CHECK_EVERY: i64 = 5;
/// Reconnect backoff cap (s), as in the bash daemon.
const BACKOFF_CAP: u64 = 60;

/// Requests from the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Poll `/usage` now.
    RefreshNow,
    /// Drop the link and look for the device again (e.g. after pairing).
    Reconnect,
    /// Forget the cached device address, then look again (after swapping boards).
    Forget,
    /// Mute the attention beep for this run, whatever the config says.
    SetBeepMuted(bool),
    /// Stop the daemon thread.
    Shutdown,
}

/// The BLE side, for the tray's status dot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LinkState {
    /// Not tried yet.
    #[default]
    Starting,
    /// No adapter, or Bluetooth is off.
    NoAdapter(String),
    /// No paired device found; retrying.
    Searching,
    /// Connecting to a known device.
    Connecting,
    /// Connected.
    Connected {
        /// Cache key (address).
        key: String,
        /// Device name.
        name: Option<String>,
    },
}

/// Everything the tray needs, updated every tick.
#[derive(Debug, Clone, Default)]
pub struct Status {
    /// BLE link.
    pub link: LinkState,
    /// Usage / hooks / attention.
    pub engine: EngineStatus,
    /// Unix seconds of the last successful device write.
    pub last_write_at: Option<i64>,
    /// `claude` CLI path in use (None = not found).
    pub claude: Option<PathBuf>,
}

/// Handle held by the UI.
#[derive(Debug)]
pub struct DaemonHandle {
    /// Send commands.
    pub commands: mpsc::UnboundedSender<Command>,
    /// Watch status.
    pub status: watch::Receiver<Status>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl DaemonHandle {
    /// Ask the daemon to stop and wait for it.
    pub fn shutdown(mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Start the daemon on a background thread.
pub fn spawn(paths: Paths) -> DaemonHandle {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (st_tx, st_rx) = watch::channel(Status::default());
    let thread = std::thread::Builder::new()
        .name("clawdmeter-daemon".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");
            rt.block_on(run(paths, cmd_rx, st_tx));
        })
        .expect("spawn daemon thread");
    DaemonHandle { commands: cmd_tx, status: st_rx, thread: Some(thread) }
}

type PollResults = Vec<(PathBuf, Result<UsageReading, UsageError>)>;
type ConnectResult = Result<(DeviceLink, RefreshRequests), LinkError>;

fn poll_all(claude: Option<PathBuf>, dirs: Vec<PathBuf>, home: PathBuf) -> PollResults {
    let now = chrono::Utc::now();
    dirs.into_iter()
        .map(|d| {
            let r = match &claude {
                Some(c) => usage::poll_account(c, &d, &home, now),
                None => Err(UsageError::Spawn("claude CLI not found (set claude_path in the config)".into())),
            };
            (d, r)
        })
        .collect()
}

/// The daemon loop. Returns on [`Command::Shutdown`] or when every command
/// sender is dropped.
pub async fn run(paths: Paths, mut commands: mpsc::UnboundedReceiver<Command>, status: watch::Sender<Status>) {
    let settings = Settings::load(&paths).unwrap_or_else(|e| {
        log::warn!("config unreadable ({e}); using defaults");
        Settings::from_config(&Default::default(), &paths, None)
    });
    log::info!(
        "accounts: {}",
        settings.config_dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ")
    );
    let mut engine = Engine::new(settings);
    let host = HostClock { utc_offset_secs: 0, prefers_12h: clawdmeter_core::platform::prefers_12h() };
    let inbox = AttentionInbox::new(&paths.state_dir);
    let (mut tail, seed) = EventTail::open(paths.events_log());
    engine.seed_events(&seed);
    engine.set_hooks_installed(tail.exists());
    let cache = AddressCache::new(paths.ble_address_file());
    let link_opts = LinkOptions::default();

    let mut claude = usage::find_claude(engine.settings().claude_path.as_deref(), &paths.home);
    match &claude {
        Some(c) => log::info!("claude CLI: {}", c.display()),
        None => log::warn!("claude CLI not found; set claude_path in {}", paths.config_file.display()),
    }

    let mut link: Option<DeviceLink> = None;
    let mut refresh: Option<RefreshRequests> = None;
    let mut connecting: Option<JoinHandle<ConnectResult>> = None;
    let mut polling: Option<JoinHandle<PollResults>> = None;
    let mut link_state = LinkState::Starting;
    let mut next_connect = 0i64;
    let mut backoff = 1u64;
    let mut last_settings = unix_now();
    let mut last_link_check = unix_now();
    let mut last_write_at = None;
    let mut beep_muted = false;
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;
        let now = unix_now();

        // ---- commands ----
        loop {
            match commands.try_recv() {
                Ok(Command::RefreshNow) => engine.request_refresh(),
                Ok(cmd @ (Command::Reconnect | Command::Forget)) => {
                    if let Some(l) = link.take() {
                        l.close().await;
                    }
                    if cmd == Command::Forget {
                        if let Some(key) = cache.load() {
                            cache.invalidate(&key);
                        }
                    }
                    refresh = None;
                    link_state = LinkState::Searching;
                    next_connect = now;
                    backoff = 1;
                }
                Ok(Command::SetBeepMuted(m)) => {
                    beep_muted = m;
                    let mut s = engine.settings().clone();
                    if m {
                        s.beep = false;
                    } else if let Ok(fresh) = Settings::load(&paths) {
                        s = fresh;
                    }
                    engine.update_settings(s);
                }
                Ok(Command::Shutdown) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    if let Some(l) = link.take() {
                        l.close().await;
                    }
                    log::info!("daemon stopped");
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }

        // ---- config (re-read like the old daemons) ----
        if now - last_settings >= SETTINGS_EVERY {
            last_settings = now;
            if let Ok(mut s) = Settings::load(&paths) {
                if beep_muted {
                    s.beep = false;
                }
                if s.claude_path != engine.settings().claude_path || claude.is_none() {
                    claude = usage::find_claude(s.claude_path.as_deref(), &paths.home);
                }
                engine.update_settings(s);
            }
        }

        // ---- fast clock ----
        for ev in tail.poll() {
            log::debug!("hook {:?} {}", ev.kind, ev.session);
            engine.on_hook_event(&ev, now);
        }
        engine.set_hooks_installed(tail.exists());
        let alert = inbox.take_alert(SystemTime::now());
        let clears = inbox.take_clears();
        if alert.is_some() || !clears.is_empty() {
            engine.on_attention(alert, &clears);
        }

        // ---- slow clock ----
        if let Some(rx) = refresh.as_mut() {
            while rx.try_recv().is_ok() {
                log::info!("refresh requested by device");
                engine.request_refresh();
            }
        }
        if polling.as_ref().is_some_and(JoinHandle::is_finished) {
            if let Ok(results) = polling.take().expect("checked").await {
                for (d, r) in &results {
                    if let Err(e) = r {
                        log::warn!("{}: {e}", d.display());
                    }
                }
                engine.on_poll_results(unix_now(), results);
            }
        }
        if polling.is_none() {
            if let Some(why) = engine.poll_due(now) {
                log::debug!("polling /usage ({why:?})");
                let (c, dirs, home) = (claude.clone(), engine.accounts_to_poll(), paths.home.clone());
                polling = Some(tokio::task::spawn_blocking(move || poll_all(c, dirs, home)));
            }
        }

        // ---- link ----
        if connecting.as_ref().is_some_and(JoinHandle::is_finished) {
            match connecting.take().expect("checked").await {
                Ok(Ok((l, rx))) => {
                    log::info!("connected to {} ({})", l.name().unwrap_or("device"), l.key());
                    link_state = LinkState::Connected { key: l.key().to_owned(), name: l.name().map(str::to_owned) };
                    link = Some(l);
                    refresh = Some(rx);
                    backoff = 1;
                    inbox.reset();
                    engine.on_connected();
                }
                Ok(Err(e)) => {
                    link_state = match &e {
                        LinkError::NoAdapter(s) => LinkState::NoAdapter(s.clone()),
                        _ => LinkState::Searching,
                    };
                    if !matches!(e, LinkError::NotFound) || backoff == 1 {
                        log::info!("{e}; retrying in {backoff}s");
                    }
                    next_connect = now + backoff as i64;
                    backoff = (backoff * 2).min(BACKOFF_CAP);
                }
                Err(e) => log::error!("connect task failed: {e}"),
            }
        }
        if link.is_none() && connecting.is_none() && now >= next_connect {
            if link_state == LinkState::Starting {
                link_state = LinkState::Connecting;
            }
            let (cache, opts) = (cache.clone(), link_opts.clone());
            connecting = Some(tokio::spawn(async move {
                let adapter = clawdmeter_ble::adapter().await?;
                DeviceLink::open(&adapter, &cache, &opts).await
            }));
        }
        if let Some(l) = &link {
            if now - last_link_check >= LINK_CHECK_EVERY {
                last_link_check = now;
                if !l.is_connected().await {
                    log::info!("device disconnected, reconnecting...");
                    link = None;
                    refresh = None;
                    link_state = LinkState::Searching;
                    next_connect = now + 2;
                }
            }
        }

        // ---- write ----
        if let Some(l) = &link {
            if let Some(w) = engine
                .next_write(now, HostClock { utc_offset_secs: clawdmeter_core::platform::utc_offset_secs(), ..host })
            {
                let bytes = w.payload.encode(MAX_WRITE);
                let label = match w.kind {
                    WriteKind::Poll => "Sending",
                    WriteKind::Heartbeat => "Heartbeat",
                    WriteKind::Attention => "Attention",
                    WriteKind::Event => "Event",
                    WriteKind::NoData => "No data",
                };
                log::info!("{label}: {}", String::from_utf8_lossy(&bytes));
                match l.write(&bytes).await {
                    Ok(()) => {
                        engine.mark_written(now, &w);
                        last_write_at = Some(now);
                    }
                    Err(e) => {
                        log::warn!("write failed: {e}");
                        if !l.is_connected().await {
                            link = None;
                            refresh = None;
                            link_state = LinkState::Searching;
                            next_connect = now + 2;
                        }
                    }
                }
            }
        }

        let _ = status.send(Status {
            link: link_state.clone(),
            engine: engine.status(now),
            last_write_at,
            claude: claude.clone(),
        });
    }
}

/// One poll of every account, printed as the payload the device would get
/// (`clawdmeter --once`). No BLE.
pub fn once(paths: &Paths) -> i32 {
    let settings = match Settings::load(paths) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("config: {e}");
            return 1;
        }
    };
    let claude = usage::find_claude(settings.claude_path.as_deref(), &paths.home);
    let results = poll_all(claude, settings.config_dirs.clone(), paths.home.clone());
    let mut ok = false;
    for (d, r) in &results {
        match r {
            Ok(r) if r.is_spend_only() => {
                ok = true;
                println!("{}: spend limit {}% used", d.display(), r.session_pct());
            }
            Ok(r) => {
                ok = true;
                println!(
                    "{}: session {}%{}, week {}%",
                    d.display(),
                    r.session_pct(),
                    if r.window_started() { "" } else { " (window not started)" },
                    r.week.as_ref().map_or(0, |w| w.pct)
                );
            }
            Err(e) => println!("{}: {e}", d.display()),
        }
    }
    let now = unix_now();
    let mut engine = Engine::new(settings);
    engine.on_poll_results(now, results);
    let host = HostClock {
        utc_offset_secs: clawdmeter_core::platform::utc_offset_secs(),
        prefers_12h: clawdmeter_core::platform::prefers_12h(),
    };
    if let Some(w) = engine.next_write(now, host) {
        println!("{}", String::from_utf8_lossy(&w.payload.encode(MAX_WRITE)));
    }
    if ok {
        0
    } else {
        1
    }
}
