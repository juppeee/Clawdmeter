//! Start at login: a systemd user unit or XDG autostart entry (Linux), a
//! LaunchAgent (macOS), an HKCU Run value (Windows).
//!
//! The text generators are pure and tested on every OS; `enable` / `disable`
//! / `is_enabled` act on the current OS only.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How the app is launched at login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// The tray app (desktop sessions).
    Tray,
    /// Headless `--daemon` (servers, no tray).
    Daemon,
}

impl LaunchMode {
    fn args(self) -> &'static [&'static str] {
        match self {
            LaunchMode::Tray => &[],
            LaunchMode::Daemon => &["--daemon"],
        }
    }
}

/// systemd unit name.
pub const SYSTEMD_UNIT: &str = "clawdmeter.service";
/// The old bash daemon's unit; running both would fight over the device.
pub const LEGACY_SYSTEMD_UNIT: &str = "claude-usage-daemon.service";
/// LaunchAgent label.
pub const LAUNCH_AGENT_LABEL: &str = "com.clawdmeter.app";
/// The old Python daemon's LaunchAgent label.
pub const LEGACY_LAUNCH_AGENT_LABEL: &str = "com.user.claude-usage-daemon";
/// HKCU Run value name (same as the old Windows tray, which it replaces).
pub const RUN_VALUE: &str = "Clawdmeter";

/// Quote one argument for a systemd `ExecStart=` line.
fn systemd_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%").replace('$', "$$");
    format!("\"{escaped}\"")
}

/// The systemd user unit (`--daemon`, restarts on failure).
pub fn systemd_unit(exe: &Path) -> String {
    let mut exec = systemd_quote(&exe.to_string_lossy());
    for a in LaunchMode::Daemon.args() {
        exec.push(' ');
        exec.push_str(a);
    }
    format!(
        "[Unit]\n\
         Description=Clawdmeter daemon (Claude Code usage over BLE)\n\
         After=bluetooth.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exec}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

/// XDG autostart entry for the tray app.
pub fn xdg_desktop_entry(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    let quoted = if path.contains(' ') { format!("\"{path}\"") } else { path.into_owned() };
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Clawdmeter\n\
         Comment=Claude Code usage on your desk\n\
         Exec={quoted}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The LaunchAgent plist.
pub fn launch_agent_plist(exe: &Path, mode: LaunchMode, log_path: &Path) -> String {
    let mut args = format!("        <string>{}</string>\n", xml_escape(&exe.to_string_lossy()));
    for a in mode.args() {
        args.push_str(&format!("        <string>{a}</string>\n"));
    }
    let log = xml_escape(&log_path.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCH_AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
{args}    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#
    )
}

/// The HKCU Run command line.
pub fn windows_run_command(exe: &Path, mode: LaunchMode) -> String {
    let mut cmd = format!("\"{}\"", exe.to_string_lossy());
    for a in mode.args() {
        cmd.push(' ');
        cmd.push_str(a);
    }
    cmd
}

/// Where the login entry lives for `mode` on this OS (`None` on Windows,
/// which uses the registry).
pub fn entry_path(home: &Path, mode: LaunchMode) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(home.join("Library/LaunchAgents").join(format!("{LAUNCH_AGENT_LABEL}.plist")))
    } else if cfg!(windows) {
        None
    } else {
        Some(match mode {
            LaunchMode::Daemon => home.join(".config/systemd/user").join(SYSTEMD_UNIT),
            LaunchMode::Tray => home.join(".config/autostart/clawdmeter.desktop"),
        })
    }
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

fn run_quiet(cmd: &str, args: &[&str]) -> io::Result<bool> {
    let st = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    Ok(st.success())
}

fn write_file(path: &Path, text: &str) -> io::Result<()> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(path, text)
}

/// Install and activate the login entry for `exe`.
pub fn enable(home: &Path, exe: &Path, mode: LaunchMode) -> io::Result<()> {
    if cfg!(windows) {
        let cmd = windows_run_command(exe, mode);
        if !run_quiet("reg", &["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &cmd, "/f"])? {
            return Err(io::Error::other("reg add failed"));
        }
        return Ok(());
    }
    let path = entry_path(home, mode).expect("unix has a path");
    if cfg!(target_os = "macos") {
        let log = home.join("Library/Logs/clawdmeter.log");
        write_file(&path, &launch_agent_plist(exe, mode, &log))?;
        // Best effort: load now so it runs without a re-login.
        let _ = run_quiet("launchctl", &["load", "-w", &path.to_string_lossy()]);
        return Ok(());
    }
    match mode {
        LaunchMode::Daemon => {
            write_file(&path, &systemd_unit(exe))?;
            let _ = run_quiet("systemctl", &["--user", "daemon-reload"]);
            if !run_quiet("systemctl", &["--user", "enable", SYSTEMD_UNIT])? {
                return Err(io::Error::other("systemctl --user enable failed"));
            }
        }
        LaunchMode::Tray => write_file(&path, &xdg_desktop_entry(exe))?,
    }
    Ok(())
}

/// Remove the login entry (idempotent).
pub fn disable(home: &Path, mode: LaunchMode) -> io::Result<()> {
    if cfg!(windows) {
        let _ = run_quiet("reg", &["delete", RUN_KEY, "/v", RUN_VALUE, "/f"]);
        return Ok(());
    }
    let path = entry_path(home, mode).expect("unix has a path");
    if cfg!(target_os = "macos") {
        let _ = run_quiet("launchctl", &["unload", "-w", &path.to_string_lossy()]);
    } else if mode == LaunchMode::Daemon {
        let _ = run_quiet("systemctl", &["--user", "disable", SYSTEMD_UNIT]);
    }
    match fs::remove_file(&path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Whether the login entry is present.
pub fn is_enabled(home: &Path, mode: LaunchMode) -> bool {
    if cfg!(windows) {
        return run_quiet("reg", &["query", RUN_KEY, "/v", RUN_VALUE]).unwrap_or(false);
    }
    entry_path(home, mode).is_some_and(|p| p.exists())
}

/// Stop and disable the old daemon's login entry (bash systemd unit or the
/// Python LaunchAgent) so it doesn't fight the app for the device. Best effort.
pub fn disable_legacy(home: &Path) {
    if cfg!(target_os = "macos") {
        let p = home.join("Library/LaunchAgents").join(format!("{LEGACY_LAUNCH_AGENT_LABEL}.plist"));
        if p.exists() {
            let _ = run_quiet("launchctl", &["unload", "-w", &p.to_string_lossy()]);
        }
    } else if !cfg!(windows) {
        let _ = run_quiet("systemctl", &["--user", "disable", "--now", LEGACY_SYSTEMD_UNIT]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_unit_text() {
        let u = systemd_unit(Path::new("/opt/My Apps/clawdmeter"));
        assert!(u.contains("ExecStart=\"/opt/My Apps/clawdmeter\" --daemon\n"));
        assert!(u.contains("Restart=on-failure"));
        assert!(u.contains("WantedBy=default.target"));
        assert!(systemd_unit(Path::new("/a%b")).contains("\"/a%%b\""));
    }

    #[test]
    fn desktop_entry_text() {
        assert!(xdg_desktop_entry(Path::new("/usr/bin/clawdmeter")).contains("Exec=/usr/bin/clawdmeter\n"));
        assert!(xdg_desktop_entry(Path::new("/a b/c")).contains("Exec=\"/a b/c\"\n"));
    }

    #[test]
    fn plist_text() {
        let p =
            launch_agent_plist(Path::new("/Applications/Clawdmeter.app/x&y"), LaunchMode::Daemon, Path::new("/l.log"));
        assert!(p.contains("<string>/Applications/Clawdmeter.app/x&amp;y</string>\n        <string>--daemon</string>"));
        assert!(p.contains(LAUNCH_AGENT_LABEL));
        let t = launch_agent_plist(Path::new("/x"), LaunchMode::Tray, Path::new("/l.log"));
        assert!(!t.contains("--daemon"));
    }

    #[test]
    fn windows_command() {
        assert_eq!(
            windows_run_command(Path::new(r"C:\Program Files\Clawdmeter\clawdmeter.exe"), LaunchMode::Tray),
            r#""C:\Program Files\Clawdmeter\clawdmeter.exe""#
        );
        assert!(windows_run_command(Path::new("c.exe"), LaunchMode::Daemon).ends_with(" --daemon"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_paths_and_tray_enable_disable() {
        let home = std::env::temp_dir().join(format!("cm-auto-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        assert_eq!(
            entry_path(&home, LaunchMode::Daemon).unwrap(),
            home.join(".config/systemd/user/clawdmeter.service")
        );
        assert!(!is_enabled(&home, LaunchMode::Tray));
        enable(&home, Path::new("/usr/bin/clawdmeter"), LaunchMode::Tray).unwrap();
        assert!(is_enabled(&home, LaunchMode::Tray));
        disable(&home, LaunchMode::Tray).unwrap();
        assert!(!is_enabled(&home, LaunchMode::Tray));
        disable(&home, LaunchMode::Tray).unwrap();
        let _ = fs::remove_dir_all(&home);
    }
}
