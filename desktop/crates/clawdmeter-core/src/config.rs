//! The `key = value` config file shared with the old daemons and the settings
//! window.
//!
//! Reading matches `daemon/claude-usage-daemon.sh` and the Python daemons:
//! `#` starts a comment, whitespace and CR are trimmed, keys compare
//! case-insensitively, and the last occurrence of a key wins. Writing edits
//! the file line by line so comments, blank lines, unknown keys and their
//! order survive a round trip through the GUI.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::paths::Paths;

/// Default seconds between `/usage` polls while a session is working.
pub const DEFAULT_POLL_INTERVAL: u64 = 60;
/// Default seconds between heartbeat replays.
pub const DEFAULT_HEARTBEAT_INTERVAL: u64 = 60;
/// Smallest accepted poll / heartbeat interval.
pub const MIN_INTERVAL: u64 = 10;
/// GitHub repo the flasher reads releases from.
pub const DEFAULT_RELEASE_REPO: &str = "HermannBjorgvin/Clawdmeter";

/// A config file held as its original lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigFile {
    lines: Vec<String>,
    trailing_newline: bool,
}

/// Split one line into (key, value) when it is an active assignment.
fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_end_matches('\r');
    let code = match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    };
    let (k, v) = code.split_once('=')?;
    let k = k.trim();
    if k.is_empty() || k.contains(char::is_whitespace) {
        return None;
    }
    Some((k, v.trim()))
}

impl ConfigFile {
    /// Parse file contents. Never fails: unparseable lines are kept verbatim.
    pub fn parse(text: &str) -> Self {
        let trailing_newline = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines = if text.is_empty() { Vec::new() } else { body.split('\n').map(str::to_owned).collect() };
        ConfigFile { lines, trailing_newline: trailing_newline || text.is_empty() }
    }

    /// Load from disk; a missing file is an empty config.
    pub fn load(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(t) => Ok(Self::parse(&t)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// Write atomically (temp file + rename), creating the parent dir.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp: PathBuf = path.with_extension("tmp");
        fs::write(&tmp, self.render())?;
        fs::rename(&tmp, path)
    }

    /// The file contents.
    pub fn render(&self) -> String {
        let mut s = self.lines.join("\n");
        if self.trailing_newline && !self.lines.is_empty() {
            s.push('\n');
        }
        s
    }

    /// The value of `key` (last occurrence wins), comment and whitespace stripped.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines
            .iter()
            .rev()
            .filter_map(|l| split_assignment(l))
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    /// Set `key`. Rewrites the last active line for it in place (keeping its
    /// indentation and trailing comment); otherwise inserts after a commented
    /// example (`# key = ...`) if there is one, or appends.
    pub fn set(&mut self, key: &str, value: &str) {
        let active =
            self.lines.iter().rposition(|l| split_assignment(l).is_some_and(|(k, _)| k.eq_ignore_ascii_case(key)));
        if let Some(i) = active {
            let line = self.lines[i].trim_end_matches('\r').to_owned();
            let cr = if self.lines[i].ends_with('\r') { "\r" } else { "" };
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            let comment = line.find('#').map(|j| line[j..].to_owned());
            let mut new = format!("{indent}{key} = {value}");
            if let Some(c) = comment {
                new.push(' ');
                new.push_str(&c);
            }
            new.push_str(cr);
            self.lines[i] = new;
            return;
        }
        let example = self.lines.iter().rposition(|l| {
            let t = l.trim_start();
            t.strip_prefix('#')
                .and_then(|rest| split_assignment(rest.trim_start()))
                .is_some_and(|(k, _)| k.eq_ignore_ascii_case(key))
        });
        let new = format!("{key} = {value}");
        match example {
            Some(i) => self.lines.insert(i + 1, new),
            None => self.lines.push(new),
        }
        self.trailing_newline = true;
    }

    /// Remove every active line for `key`. Comments are left alone.
    pub fn remove(&mut self, key: &str) {
        self.lines.retain(|l| !split_assignment(l).is_some_and(|(k, _)| k.eq_ignore_ascii_case(key)));
    }

    /// All active `(key, value)` pairs in file order (duplicates included).
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.lines.iter().filter_map(|l| split_assignment(l))
    }
}

/// `clock` option: whether and how the device shows a wall clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockMode {
    /// No clock; the device shows "Usage" (default).
    #[default]
    Off,
    /// Show a clock, 12/24h from the locale.
    Auto,
    /// Forced 12-hour.
    H12,
    /// Forced 24-hour.
    H24,
}

impl ClockMode {
    /// Parse a config value; anything unknown is `Off`, like the daemons.
    pub fn parse(v: &str) -> Self {
        match v.trim().to_ascii_lowercase().as_str() {
            "auto" => ClockMode::Auto,
            "12" => ClockMode::H12,
            "24" => ClockMode::H24,
            _ => ClockMode::Off,
        }
    }

    /// The config-file spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            ClockMode::Off => "off",
            ClockMode::Auto => "auto",
            ClockMode::H12 => "12",
            ClockMode::H24 => "24",
        }
    }
}

/// Typed view of the config, with the old daemons' defaults and clamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Claude config dirs to poll (accounts). Default `[~/.claude]`.
    pub config_dirs: Vec<PathBuf>,
    /// `/usage` poll interval while a session is working (s, >= 10).
    pub poll_interval: u64,
    /// True when `poll_interval` came from the file or the env var, in which
    /// case it also replaces the 2-minute no-hooks cadence.
    pub poll_interval_explicit: bool,
    /// Heartbeat replay interval (s, >= 10).
    pub heartbeat_interval: u64,
    /// Session-reset chime (`c`).
    pub chime: bool,
    /// Attention beep (`b`).
    pub beep: bool,
    /// Wall clock on the device (`t`/`tf`).
    pub clock: ClockMode,
    /// `owner/repo` for firmware releases.
    pub release_repo: String,
    /// Explicit path to the `claude` CLI, if the user set one.
    pub claude_path: Option<PathBuf>,
    /// Daily firmware update check (default on).
    pub update_check: bool,
}

/// Parse an interval like the bash daemon: digits only, clamped to >= 10.
pub fn parse_interval(v: &str) -> Option<u64> {
    let v = v.trim();
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // All digits but too long for u64 is still "a big number".
    Some(v.parse::<u64>().unwrap_or(u64::MAX).max(MIN_INTERVAL))
}

fn on_off(v: Option<&str>, default: bool) -> bool {
    match v.map(|s| s.trim().to_ascii_lowercase()) {
        Some(s) if s == "on" => true,
        Some(s) if s == "off" => false,
        Some(_) => false,
        None => default,
    }
}

impl Settings {
    /// Build from a parsed file. `env_poll` is the `POLL_INTERVAL` env var,
    /// which seeds the default the file overrides (bash daemon behaviour).
    pub fn from_config(cfg: &ConfigFile, paths: &Paths, env_poll: Option<&str>) -> Self {
        let env_default = env_poll.and_then(parse_interval);
        let file_poll = cfg.get("poll_interval").and_then(parse_interval);
        let poll_interval = file_poll.or(env_default).unwrap_or(DEFAULT_POLL_INTERVAL);
        let heartbeat_interval =
            cfg.get("heartbeat_interval").and_then(parse_interval).unwrap_or(DEFAULT_HEARTBEAT_INTERVAL);
        Settings {
            config_dirs: parse_config_dirs(cfg.get("config_dirs"), paths),
            poll_interval,
            poll_interval_explicit: file_poll.is_some() || env_default.is_some(),
            heartbeat_interval,
            // chime/beep: only an explicit "on" enables (bash read_onoff_setting).
            chime: on_off(cfg.get("chime"), false),
            beep: on_off(cfg.get("beep"), false),
            clock: cfg.get("clock").map(ClockMode::parse).unwrap_or_default(),
            release_repo: cfg
                .get("release_repo")
                .filter(|s| s.split('/').count() == 2 && !s.starts_with('/') && !s.ends_with('/'))
                .unwrap_or(DEFAULT_RELEASE_REPO)
                .to_owned(),
            claude_path: cfg.get("claude_path").filter(|s| !s.is_empty()).map(|s| paths.expand_tilde(s)),
            update_check: on_off(cfg.get("update_check"), true),
        }
    }

    /// Load the user's config file (missing file → defaults).
    pub fn load(paths: &Paths) -> io::Result<Self> {
        let cfg = ConfigFile::load(&paths.config_file)?;
        let env = std::env::var("POLL_INTERVAL").ok();
        Ok(Self::from_config(&cfg, paths, env.as_deref()))
    }

    /// Write these settings into `cfg`, touching only keys whose value differs
    /// from what the file already says (so a hand-written `chime = off` stays
    /// as written and absent keys at their defaults stay absent).
    pub fn apply_to(&self, cfg: &mut ConfigFile, paths: &Paths) {
        let current = Settings::from_config(cfg, paths, None);
        if current.config_dirs != self.config_dirs {
            let home = &paths.home;
            let list: Vec<String> = self
                .config_dirs
                .iter()
                .map(|d| match d.strip_prefix(home) {
                    Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
                    Ok(rest) => format!("~/{}", rest.to_string_lossy().replace('\\', "/")),
                    Err(_) => d.to_string_lossy().into_owned(),
                })
                .collect();
            cfg.set("config_dirs", &list.join(", "));
        }
        if current.poll_interval != self.poll_interval {
            cfg.set("poll_interval", &self.poll_interval.to_string());
        }
        if current.heartbeat_interval != self.heartbeat_interval {
            cfg.set("heartbeat_interval", &self.heartbeat_interval.to_string());
        }
        let oo = |b: bool| if b { "on" } else { "off" };
        if current.chime != self.chime {
            cfg.set("chime", oo(self.chime));
        }
        if current.beep != self.beep {
            cfg.set("beep", oo(self.beep));
        }
        if current.clock != self.clock {
            cfg.set("clock", self.clock.as_str());
        }
        if current.release_repo != self.release_repo {
            cfg.set("release_repo", &self.release_repo);
        }
        if current.claude_path != self.claude_path {
            match &self.claude_path {
                Some(p) => cfg.set("claude_path", &p.to_string_lossy()),
                None => cfg.remove("claude_path"),
            }
        }
        if current.update_check != self.update_check {
            cfg.set("update_check", oo(self.update_check));
        }
    }
}

/// `config_dirs`: comma list, `~` expanded, blanks dropped; default `[~/.claude]`.
pub fn parse_config_dirs(raw: Option<&str>, paths: &Paths) -> Vec<PathBuf> {
    let dirs: Vec<PathBuf> =
        raw.unwrap_or("").split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| paths.expand_tilde(s)).collect();
    if dirs.is_empty() {
        vec![paths.home.join(".claude")]
    } else {
        dirs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> Paths {
        Paths::under(Path::new("/home/u"))
    }

    fn settings(text: &str) -> Settings {
        Settings::from_config(&ConfigFile::parse(text), &paths(), None)
    }

    // Ported from daemon/tests/test_bash_heartbeat.sh (interval parsing).
    #[test]
    fn intervals_like_bash() {
        let s = settings("");
        assert_eq!((s.poll_interval, s.heartbeat_interval), (60, 60));
        assert!(!s.poll_interval_explicit);
        let s = settings("poll_interval = 300\nheartbeat_interval = 45\n");
        assert_eq!((s.poll_interval, s.heartbeat_interval), (300, 45));
        assert!(s.poll_interval_explicit);
        assert_eq!(settings("  poll_interval=120 # comment\r\n").poll_interval, 120);
        let s = settings("poll_interval = 3\nheartbeat_interval = 0\n");
        assert_eq!((s.poll_interval, s.heartbeat_interval), (10, 10));
        assert_eq!(settings("poll_interval = abc\n").poll_interval, 60);
        assert_eq!(settings("poll_interval = 30\npoll_interval = 90\n").poll_interval, 90);
    }

    #[test]
    fn env_poll_interval_seeds_default() {
        let cfg = ConfigFile::parse("");
        let p = paths();
        assert_eq!(Settings::from_config(&cfg, &p, Some("45")).poll_interval, 45);
        assert_eq!(Settings::from_config(&cfg, &p, Some("1")).poll_interval, 10);
        assert_eq!(Settings::from_config(&cfg, &p, Some("abc")).poll_interval, 60);
        assert_eq!(Settings::from_config(&cfg, &p, Some("-5")).poll_interval, 60);
        assert_eq!(Settings::from_config(&cfg, &p, Some("")).poll_interval, 60);
        let file = ConfigFile::parse("poll_interval = 200\n");
        assert_eq!(Settings::from_config(&file, &p, Some("45")).poll_interval, 200);
    }

    // Ported from daemon/tests/test_macos_multidir.py (read_config_dirs).
    #[test]
    fn config_dirs() {
        assert_eq!(settings("").config_dirs, vec![PathBuf::from("/home/u/.claude")]);
        assert_eq!(settings("clock = auto\nchime = on\n").config_dirs, vec![PathBuf::from("/home/u/.claude")]);
        assert_eq!(
            settings("config_dirs = ~/.claude, ~/.claude-work  # two plans\n").config_dirs,
            vec![PathBuf::from("/home/u/.claude"), PathBuf::from("/home/u/.claude-work")]
        );
        assert_eq!(settings("config_dirs = , ,\n").config_dirs, vec![PathBuf::from("/home/u/.claude")]);
    }

    #[test]
    fn on_off_and_clock() {
        let s = settings("chime = ON\nbeep = yes\nclock = 12\n");
        assert!(s.chime);
        assert!(!s.beep);
        assert_eq!(s.clock, ClockMode::H12);
        assert_eq!(settings("clock = weird").clock, ClockMode::Off);
        assert!(settings("").update_check);
        assert_eq!(settings("").release_repo, DEFAULT_RELEASE_REPO);
        assert_eq!(settings("release_repo = me/fork").release_repo, "me/fork");
        assert_eq!(settings("release_repo = nonsense").release_repo, DEFAULT_RELEASE_REPO);
    }

    #[test]
    fn keys_are_case_insensitive_and_comments_ignored() {
        let c = ConfigFile::parse("# chime = on\nCHIME = on\n");
        assert_eq!(c.get("chime"), Some("on"));
        let c = ConfigFile::parse("# chime = on\n");
        assert_eq!(c.get("chime"), None);
    }

    #[test]
    fn set_preserves_everything_else() {
        let text = "# header\n\nchime = off # quiet\nfoo = bar\n# beep = on\nclock = off\n";
        let mut c = ConfigFile::parse(text);
        c.set("chime", "on");
        c.set("beep", "on");
        c.set("poll_interval", "120");
        assert_eq!(
            c.render(),
            "# header\n\nchime = on # quiet\nfoo = bar\n# beep = on\nbeep = on\nclock = off\npoll_interval = 120\n"
        );
        c.remove("foo");
        assert!(c.get("foo").is_none());
        assert!(c.render().contains("# header"));
    }

    #[test]
    fn set_crlf_and_empty() {
        let mut c = ConfigFile::parse("chime = off\r\n");
        c.set("chime", "on");
        assert_eq!(c.render(), "chime = on\r\n");
        let mut e = ConfigFile::parse("");
        e.set("beep", "on");
        assert_eq!(e.render(), "beep = on\n");
        let mut n = ConfigFile::parse("a = 1");
        n.set("b", "2");
        assert_eq!(n.render(), "a = 1\nb = 2\n");
    }

    #[test]
    fn apply_only_touches_changed_keys() {
        let p = paths();
        let mut c = ConfigFile::parse("# mine\nchime = off\nunknown = 7\n");
        let mut s = Settings::from_config(&c, &p, None);
        s.beep = true;
        s.config_dirs = vec![PathBuf::from("/home/u/.claude"), PathBuf::from("/home/u/.claude-work")];
        s.apply_to(&mut c, &p);
        let out = c.render();
        assert!(out.starts_with("# mine\nchime = off\nunknown = 7\n"), "{out}");
        assert!(out.contains("beep = on\n"));
        assert!(out.contains("config_dirs = ~/.claude, ~/.claude-work\n"));
        assert!(!out.contains("poll_interval"));
        let back = Settings::from_config(&c, &p, None);
        assert_eq!(back, s);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("cm-cfg-{}", std::process::id()));
        let path = dir.join("sub/config");
        let mut c = ConfigFile::parse("# x\nchime = on\n");
        c.set("beep", "on");
        c.save(&path).unwrap();
        assert_eq!(ConfigFile::load(&path).unwrap(), c);
        assert_eq!(ConfigFile::load(&dir.join("missing")).unwrap(), ConfigFile::default());
        let _ = fs::remove_dir_all(&dir);
    }
}
