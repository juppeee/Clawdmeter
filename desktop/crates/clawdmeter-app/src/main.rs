//! `clawdmeter`: the desktop companion.
//!
//! - no arguments: the tray app (daemon + tray icon; windows open as
//!   `--window` child processes).
//! - `--daemon`: headless (no window, no tray) for servers and CI.
//! - `--window settings|flasher|status`: one GPUI window.

// The tray app is a GUI program on Windows: no console window. CLI modes
// attach to the parent console instead (see `attach_console`).
#![cfg_attr(windows, windows_subsystem = "windows")]

use clawdmeter_app::{daemon, logger, tray, ui, WindowKind};
use clawdmeter_core::Paths;

const USAGE: &str = "\
clawdmeter - Claude Code usage on your desk

USAGE:
    clawdmeter [OPTIONS]

With no options, runs the tray app: the daemon plus a tray icon (or a status
window on desktops without a tray).

OPTIONS:
    --daemon          Run headless: poll usage, follow hooks, drive the device over BLE
    --window KIND     Open one window: settings, flasher or status
      [--tab NAME]    settings tab: device, accounts, hooks, alerts, background
    --once            Poll /usage for every account once, print the payload, exit (no BLE)
    --ports           List attached Espressif USB ports
    --info PORT       Ask the firmware on PORT for its env/version
    -V, --version
    -h, --help

ENVIRONMENT:
    CLAWDMETER_LOG       error|warn|info|debug (default info)
    CLAWDMETER_NO_TRAY   1 = show the status window instead of a tray icon
    POLL_INTERVAL        default poll interval (the config file overrides it)
";

#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: plain Win32 call; failure (no parent console) is fine.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = Paths::for_user();
    if args.is_empty() {
        logger::init_with_file(&paths.state_dir.join("daemon.log"));
        std::process::exit(tray::run(paths));
    }
    attach_console();
    logger::init();
    let code = match args[0].as_str() {
        "--daemon" => run_daemon(paths),
        "--window" => match args.get(1).and_then(|k| WindowKind::parse(k)) {
            Some(kind) => {
                let tab = (args.get(2).map(String::as_str) == Some("--tab")).then(|| args.get(3).cloned()).flatten();
                ui::run(kind, paths, tab)
            }
            None => {
                eprintln!("--window needs one of: settings, flasher, status");
                2
            }
        },
        "--once" => daemon::once(&paths),
        "--ports" => {
            for p in clawdmeter_flash::device::list_ports() {
                println!("{}  {:04x}:{:04x}  {}", p.path, p.vid, p.pid, p.serial_number.unwrap_or_default());
            }
            0
        }
        "--info" => match args.get(1) {
            Some(port) => match clawdmeter_flash::device::probe_info(port, std::time::Duration::from_secs(2)) {
                Ok(Some(i)) => {
                    println!("env={} ver={} rev={} link={}", i.env, i.ver, i.rev, i.link);
                    0
                }
                Ok(None) => {
                    eprintln!("no reply (not running Clawdmeter firmware, or too old for `info`)");
                    1
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            },
            None => {
                eprintln!("--info needs a port");
                2
            }
        },
        "-V" | "--version" => {
            println!("clawdmeter {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "-h" | "--help" => {
            print!("{USAGE}");
            0
        }
        other => {
            eprintln!("unknown option {other}\n\n{USAGE}");
            2
        }
    };
    std::process::exit(code);
}

fn run_daemon(paths: Paths) -> i32 {
    let _lock = match clawdmeter_app::ipc::acquire_instance_lock(&paths) {
        Ok(Some(f)) => Some(f),
        Ok(None) => {
            eprintln!("Clawdmeter is already running (tray app or --daemon); not starting a second daemon");
            return 1;
        }
        Err(e) => {
            log::warn!("instance lock unavailable ({e}); continuing");
            None
        }
    };
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime: {e}");
            return 1;
        }
    };
    rt.block_on(async move {
        log::info!("=== Clawdmeter daemon {} ===", env!("CARGO_PKG_VERSION"));
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (st_tx, _st_rx) = tokio::sync::watch::channel(daemon::Status::default());
        tokio::spawn(async move {
            shutdown_signal().await;
            let _ = cmd_tx.send(daemon::Command::Shutdown);
        });
        daemon::run(paths, cmd_rx, st_tx).await;
    });
    0
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
