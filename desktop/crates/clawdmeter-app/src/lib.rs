//! The app's library half: the daemon runtime, the tray process, and the
//! GPUI windows, shared by the headless `--daemon` mode and the desktop app.
//!
//! Process layout: `clawdmeter` (no arguments) is the tray process. It runs
//! the daemon and the tray icon and lives for the whole session. Each window
//! is a separate `clawdmeter --window <kind>` process running GPUI, which
//! exits when its window closes (see [`ipc`] for why and how they talk).

#![warn(missing_docs)]

pub mod art;
pub mod boards;
pub mod daemon;
pub mod diff;
pub mod errors;
pub mod hooks;
pub mod ipc;
pub mod logger;
pub mod tray;
pub mod ui;

/// The app's version: `CLAWDMETER_VERSION` at build time (CI sets it from
/// the release tag; a leading `v` is dropped), else the crate version.
pub fn version() -> &'static str {
    const RAW: &str = match option_env!("CLAWDMETER_VERSION") {
        Some(v) => v,
        None => env!("CARGO_PKG_VERSION"),
    };
    let v = RAW.trim();
    let v = v.strip_prefix('v').unwrap_or(v);
    if v.is_empty() {
        env!("CARGO_PKG_VERSION")
    } else {
        v
    }
}

/// The app's windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowKind {
    /// Settings (tabs: Device, Accounts, Hooks, Alerts, Background).
    Settings,
    /// The firmware flasher.
    Flasher,
    /// The tray menu as a window, for desktops without a tray.
    Status,
}

impl WindowKind {
    /// Parse the `--window` argument.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "settings" => Some(WindowKind::Settings),
            "flasher" => Some(WindowKind::Flasher),
            "status" => Some(WindowKind::Status),
            _ => None,
        }
    }

    /// The `--window` argument.
    pub fn as_str(self) -> &'static str {
        match self {
            WindowKind::Settings => "settings",
            WindowKind::Flasher => "flasher",
            WindowKind::Status => "status",
        }
    }
}

/// Shorten a path under the home dir to `~/…` for display.
pub fn tilde(path: &str, home: &std::path::Path) -> String {
    let home = home.to_string_lossy();
    match path.strip_prefix(home.as_ref()) {
        Some("") => "~".to_owned(),
        Some(rest) if rest.starts_with('/') || rest.starts_with('\\') => format!("~{rest}"),
        _ => path.to_owned(),
    }
}

/// "12 s ago" / "3 min ago" / "2 h ago".
pub fn ago(secs: i64) -> String {
    match secs.max(0) {
        s if s < 60 => format!("{s} s ago"),
        s if s < 3600 => format!("{} min ago", s / 60),
        s => format!("{} h ago", s / 3600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_and_ago() {
        let home = std::path::Path::new("/home/u");
        assert_eq!(tilde("/home/u/.claude", home), "~/.claude");
        assert_eq!(tilde("/home/u", home), "~");
        assert_eq!(tilde("/home/uu/.claude", home), "/home/uu/.claude");
        assert_eq!(ago(12), "12 s ago");
        assert_eq!(ago(200), "3 min ago");
        assert_eq!(ago(7300), "2 h ago");
        for k in [WindowKind::Settings, WindowKind::Flasher, WindowKind::Status] {
            assert_eq!(WindowKind::parse(k.as_str()), Some(k));
        }
    }
}
