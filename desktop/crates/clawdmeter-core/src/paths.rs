//! Where the daemon keeps its files.
//!
//! One state dir holds everything (config, hook files, events log, BLE
//! cache, logs, app state): `~/.config/clawdmeter` on Linux and macOS,
//! `%LOCALAPPDATA%\Clawdmeter` on Windows. The hook (`clawdmeter hook`)
//! resolves the same dir, so the two always agree. Installs from before the
//! rename kept their files in `~/.config/claude-usage-monitor`;
//! [`migrate_legacy`] moves them over once.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// All filesystem locations the daemon touches. Build one with
/// [`Paths::for_user`] in production or [`Paths::under`] in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// The user's home dir; `~` in config values expands to this.
    pub home: PathBuf,
    /// The state dir: hook files, events log, BLE cache, logs, app state.
    pub state_dir: PathBuf,
    /// The key = value config file (in the state dir).
    pub config_file: PathBuf,
    /// Firmware downloads and update-check state.
    pub cache_dir: PathBuf,
}

impl Paths {
    /// The real per-user locations.
    pub fn for_user() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let state_dir = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData").join("Local"))
                .join("Clawdmeter")
        } else {
            home.join(".config").join("clawdmeter")
        };
        let config_file = state_dir.join("config");
        let cache_dir = dirs::cache_dir().unwrap_or_else(|| home.join(".cache")).join("clawdmeter");
        Paths { home, state_dir, config_file, cache_dir }
    }

    /// Everything under one root (tests, portable installs).
    pub fn under(root: &Path) -> Self {
        let state_dir = root.join("state");
        Paths {
            home: root.to_path_buf(),
            config_file: state_dir.join("config"),
            state_dir,
            cache_dir: root.join("cache"),
        }
    }

    /// `events.log`, appended by the hook in event mode:
    /// `<unix_ts> <HookEventName> <session_id>` per line.
    pub fn events_log(&self) -> PathBuf {
        self.state_dir.join("events.log")
    }

    /// The attention alert file (`{"m": message, "s": session id}`).
    pub fn attention_file(&self) -> PathBuf {
        self.state_dir.join("attention")
    }

    /// Cached device address (one line).
    pub fn ble_address_file(&self) -> PathBuf {
        self.state_dir.join("ble-address")
    }

    /// Downloaded firmware images.
    pub fn firmware_cache(&self) -> PathBuf {
        self.cache_dir.join("firmware")
    }

    /// Expand a leading `~` / `~/` against [`Paths::home`].
    pub fn expand_tilde(&self, raw: &str) -> PathBuf {
        expand_tilde(&self.home, raw)
    }
}

/// The state dir of installs from before the rename, on every OS.
pub fn legacy_state_dir(home: &Path) -> PathBuf {
    home.join(".config").join("claude-usage-monitor")
}

/// What [`migrate_legacy`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Migration {
    /// File names moved into the state dir.
    pub moved: Vec<String>,
    /// File names left behind because the state dir already has one.
    pub kept: Vec<String>,
}

/// Move the files of a pre-rename install from [`legacy_state_dir`] into
/// `paths.state_dir`. A name the state dir already has stays where it is
/// (the new file wins), lock files are runtime-only and stay too. The
/// legacy dir is removed once it is empty. `None` when there is no legacy
/// dir, so this is cheap to call on every start.
pub fn migrate_legacy(paths: &Paths) -> io::Result<Option<Migration>> {
    migrate_from(&legacy_state_dir(&paths.home), &paths.state_dir)
}

fn migrate_from(old: &Path, new: &Path) -> io::Result<Option<Migration>> {
    if old == new || !old.is_dir() {
        return Ok(None);
    }
    fs::create_dir_all(new)?;
    let mut m = Migration::default();
    let mut names: Vec<String> =
        fs::read_dir(old)?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for name in names {
        let (from, to) = (old.join(&name), new.join(&name));
        if name.ends_with(".lock") || to.exists() || !from.is_file() {
            m.kept.push(name);
            continue;
        }
        if fs::rename(&from, &to).is_err() {
            // Different filesystem (a symlinked ~/.config): copy, then drop.
            fs::copy(&from, &to)?;
            fs::remove_file(&from)?;
        }
        m.moved.push(name);
    }
    let _ = fs::remove_dir(old); // only succeeds when empty
    Ok(Some(m))
}

/// Expand a leading `~` (alone, `~/x` or `~\x`) and `$HOME` against `home`.
pub fn expand_tilde(home: &Path, raw: &str) -> PathBuf {
    if raw == "~" || raw == "$HOME" {
        return home.to_path_buf();
    }
    for prefix in ["~/", "~\\", "$HOME/"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expansion() {
        let h = Path::new("/home/u");
        assert_eq!(expand_tilde(h, "~"), PathBuf::from("/home/u"));
        assert_eq!(expand_tilde(h, "~/.claude-work"), PathBuf::from("/home/u/.claude-work"));
        assert_eq!(expand_tilde(h, "$HOME/.claude"), PathBuf::from("/home/u/.claude"));
        assert_eq!(expand_tilde(h, "/abs/dir"), PathBuf::from("/abs/dir"));
        assert_eq!(expand_tilde(h, "~other"), PathBuf::from("~other"));
    }

    #[test]
    fn under_layout() {
        let p = Paths::under(Path::new("/t"));
        assert_eq!(p.events_log(), PathBuf::from("/t/state/events.log"));
        assert_eq!(p.config_file, PathBuf::from("/t/state/config"));
        assert_eq!(p.ble_address_file(), PathBuf::from("/t/state/ble-address"));
    }

    #[cfg(not(windows))]
    #[test]
    fn user_layout_is_one_dir() {
        let p = Paths::for_user();
        assert!(p.state_dir.ends_with(".config/clawdmeter"));
        assert_eq!(p.config_file, p.state_dir.join("config"));
    }

    #[test]
    fn legacy_files_move_once() {
        let root = std::env::temp_dir().join(format!("cm-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let p = Paths::under(&root);
        assert_eq!(migrate_legacy(&p).unwrap(), None);

        let old = legacy_state_dir(&root);
        fs::create_dir_all(&old).unwrap();
        for (n, body) in
            [("config", "chime = off\n"), ("ble-address", "AA:BB\n"), ("events.log", "old\n"), ("app.lock", "")]
        {
            fs::write(old.join(n), body).unwrap();
        }
        fs::create_dir_all(&p.state_dir).unwrap();
        fs::write(p.events_log(), "new\n").unwrap();

        let m = migrate_legacy(&p).unwrap().unwrap();
        assert_eq!(m.moved, ["ble-address", "config"]);
        assert_eq!(m.kept, ["app.lock", "events.log"]);
        assert_eq!(fs::read_to_string(&p.config_file).unwrap(), "chime = off\n");
        assert_eq!(fs::read_to_string(p.events_log()).unwrap(), "new\n");
        assert!(old.join("events.log").exists(), "a clash leaves the old file in place");

        // Once the leftovers are gone, the legacy dir goes too.
        fs::remove_file(old.join("events.log")).unwrap();
        fs::remove_file(old.join("app.lock")).unwrap();
        assert_eq!(migrate_legacy(&p).unwrap(), Some(Migration::default()));
        assert!(!old.exists());
        assert_eq!(migrate_legacy(&p).unwrap(), None);
        let _ = fs::remove_dir_all(&root);
    }
}
