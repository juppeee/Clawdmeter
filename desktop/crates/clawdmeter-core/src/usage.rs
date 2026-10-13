//! Usage from `claude --output-format json -p /usage`.
//!
//! The CLI answers locally (`"local_command": "usage"`, zero tokens), so
//! polling it never sends a message or starts a window. The numbers live in
//! human-readable lines of the `result` text:
//!
//! ```text
//! Current session: 15% used · resets Oct 13, 7am (Atlantic/Reykjavik)
//! Current week (all models): 6% used · resets Oct 18, 10pm (Atlantic/Reykjavik)
//! Current week (Fable): 0% used · resets Oct 18, 10pm (Atlantic/Reykjavik)
//! ```
//!
//! Reset times are local wall-clock text with an IANA zone and no year; the
//! parser takes the next occurrence. A missing session line (or one with no
//! reset) means the 5-hour window hasn't started. A text with neither a
//! session nor a week line is a parse failure, never a zero.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use chrono::{DateTime, Datelike, Duration as ChronoDuration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

/// One usage window (session or week).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// Percent used, rounded. Can exceed 100 on overage.
    pub pct: u32,
    /// When the window resets; `None` when the line has no (parseable) reset.
    pub resets_at: Option<DateTime<Utc>>,
}

/// Everything `/usage` told us for one account.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UsageReading {
    /// `Current session` (5-hour window). `None` = window not started.
    pub session: Option<Window>,
    /// `Current week (all models)`.
    pub week: Option<Window>,
    /// `Current week (<model>)` lines, in order. Not sent to the device yet.
    pub models: Vec<(String, Window)>,
}

impl UsageReading {
    /// Session % used, 0 when the window hasn't started.
    pub fn session_pct(&self) -> u32 {
        self.session.as_ref().map_or(0, |w| w.pct)
    }

    /// True when a 5-hour window is running (a session line with a reset).
    pub fn window_started(&self) -> bool {
        self.session.as_ref().is_some_and(|w| w.resets_at.is_some())
    }
}

/// Why a poll produced no reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageError {
    /// `claude` could not be found or started.
    Spawn(String),
    /// The CLI ran longer than the timeout and was killed.
    Timeout,
    /// The CLI exited non-zero or printed something that isn't its JSON.
    BadOutput(String),
    /// The CLI says this account needs `/login`.
    NotLoggedIn(String),
    /// The account isn't on a subscription plan (API key billing).
    NotSubscription,
    /// The text had neither a session nor a week line.
    NoUsageLines(String),
}

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsageError::Spawn(e) => write!(f, "could not run claude: {e}"),
            UsageError::Timeout => write!(f, "claude /usage timed out"),
            UsageError::BadOutput(e) => write!(f, "unexpected claude output: {e}"),
            UsageError::NotLoggedIn(e) => write!(f, "not logged in: {e}"),
            UsageError::NotSubscription => write!(f, "account is not on a subscription plan"),
            UsageError::NoUsageLines(e) => write!(f, "no usage lines in /usage output: {e}"),
        }
    }
}

impl std::error::Error for UsageError {}

impl UsageError {
    /// Errors that say this account can't be read at all until the user acts,
    /// as opposed to a glitch worth retrying.
    pub fn is_auth(&self) -> bool {
        matches!(self, UsageError::NotLoggedIn(_) | UsageError::NotSubscription)
    }
}

fn snippet(s: &str) -> String {
    let s = s.trim();
    let mut out: String = s.chars().take(160).collect();
    if s.chars().count() > 160 {
        out.push('…');
    }
    out
}

/// Parse the CLI's JSON (`--output-format json`) and then its `result` text.
pub fn parse_cli_output(stdout: &str, now: DateTime<Utc>) -> Result<UsageReading, UsageError> {
    // The CLI prints one JSON object; tolerate leading noise lines.
    let json_start = stdout.find('{').ok_or_else(|| UsageError::BadOutput(snippet(stdout)))?;
    let v: serde_json::Value = serde_json::from_str(stdout[json_start..].trim())
        .map_err(|e| UsageError::BadOutput(format!("{e}: {}", snippet(stdout))))?;
    let result = v.get("result").and_then(|r| r.as_str()).unwrap_or("");
    let is_error = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
    // Lines first: a hint like "/login" elsewhere in a good answer must not
    // turn it into an error.
    match parse_usage_text(result, now) {
        Err(e) if is_error && !e.is_auth() => Err(UsageError::BadOutput(snippet(result))),
        other => other,
    }
}

fn classify_auth(text: &str) -> Option<UsageError> {
    let l = text.to_ascii_lowercase();
    if l.contains("not logged in")
        || l.contains("/login")
        || l.contains("invalid api key")
        || l.contains("oauth token has expired")
    {
        return Some(UsageError::NotLoggedIn(snippet(text)));
    }
    if l.contains("using your api key") || l.contains("only available for subscription") {
        return Some(UsageError::NotSubscription);
    }
    None
}

/// Parse the human-readable `/usage` text.
pub fn parse_usage_text(text: &str, now: DateTime<Utc>) -> Result<UsageReading, UsageError> {
    let mut r = UsageReading::default();
    let mut matched = false;
    for raw in text.lines() {
        let line = raw.trim();
        let Some(rest) = line.strip_prefix("Current ") else { continue };
        let Some((label, body)) = rest.split_once(':') else { continue };
        let Some(window) = parse_window(body, now) else {
            log::warn!("/usage line stopped matching: {line:?}");
            continue;
        };
        let label = label.trim();
        if label.eq_ignore_ascii_case("session") {
            r.session = Some(window);
            matched = true;
        } else if let Some(inner) = label
            .strip_prefix("week")
            .map(str::trim)
            .and_then(|s| s.strip_prefix('('))
            .and_then(|s| s.strip_suffix(')'))
        {
            if inner.eq_ignore_ascii_case("all models") {
                r.week = Some(window);
            } else {
                r.models.push((inner.to_owned(), window));
            }
            matched = true;
        } else if label.eq_ignore_ascii_case("week") {
            r.week = Some(window);
            matched = true;
        }
    }
    if matched {
        Ok(r)
    } else {
        Err(classify_auth(text).unwrap_or_else(|| UsageError::NoUsageLines(snippet(text))))
    }
}

/// `" 15% used · resets Oct 13, 7am (Atlantic/Reykjavik)"` → Window.
fn parse_window(body: &str, now: DateTime<Utc>) -> Option<Window> {
    let pct_end = body.find('%')?;
    let pct: f64 = body[..pct_end].trim().parse().ok()?;
    if !pct.is_finite() || pct < 0.0 {
        return None;
    }
    let after = &body[pct_end + 1..];
    let resets_at = after.find("resets").and_then(|i| parse_reset(after[i + "resets".len()..].trim(), now));
    Some(Window { pct: pct.round().min(9999.0) as u32, resets_at })
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// Parse reset text such as `Oct 13, 9:59am (Atlantic/Reykjavik)`,
/// `Oct 13 at 7 PM (Europe/Berlin)`, `10pm (UTC)` or `Oct 13, 19:00 (UTC)`.
/// No year: the next occurrence after `now` wins (a reset that passed less
/// than an hour ago is kept, so a lagging CLI reads as "resets in 0m").
/// Without a zone the machine's local zone is used.
pub fn parse_reset(text: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let text = text.trim();
    let (when, zone) = match (text.rfind('('), text.rfind(')')) {
        (Some(a), Some(b)) if b > a => (text[..a].trim(), Some(text[a + 1..b].trim())),
        _ => (text, None),
    };
    let when = when.replace(" at ", ", ");
    if let Some((d, t)) = when.rsplit_once(',') {
        return parse_reset_parts(Some(d.trim()), t.trim(), zone, now);
    }
    // "Oct 13 7am" or a bare "7am" / "7 PM".
    let toks: Vec<&str> = when.split_whitespace().collect();
    if toks.len() >= 3 {
        return parse_reset_parts(Some(&toks[..2].join(" ")), &toks[2..].join(" "), zone, now);
    }
    parse_reset_parts(None, &when, zone, now)
}

fn parse_reset_parts(
    date_part: Option<&str>,
    time_part: &str,
    zone: Option<&str>,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let time = parse_time(time_part)?;
    let md = match date_part {
        Some(d) if !d.is_empty() => Some(parse_month_day(d)?),
        _ => None,
    };
    match zone {
        Some(z) => {
            let tz: Tz = z.parse().ok()?;
            next_occurrence(&tz, md, time, now)
        }
        None => next_occurrence(&chrono::Local, md, time, now),
    }
}

/// `Oct 13` / `October 13` / `13 Oct`.
fn parse_month_day(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let month = |p: &str| {
        let p = p.trim_end_matches('.').to_ascii_lowercase();
        if p.len() < 3 {
            return None;
        }
        MONTHS.iter().position(|m| p.starts_with(m)).map(|i| i as u32 + 1)
    };
    let day = |p: &str| p.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse::<u32>().ok();
    match (month(parts[0]), day(parts[1])) {
        (Some(m), Some(d)) => Some((m, d)),
        _ => match (day(parts[0]), month(parts[1])) {
            (Some(d), Some(m)) => Some((m, d)),
            _ => None,
        },
    }
}

/// `7am`, `9:59am`, `7 PM`, `7:00 p.m.`, `19:00`.
fn parse_time(s: &str) -> Option<NaiveTime> {
    let s = s.trim().to_ascii_lowercase().replace('.', "");
    let s = s.replace(' ', "");
    let (digits, meridiem) = if let Some(d) = s.strip_suffix("am") {
        (d, Some(false))
    } else if let Some(d) = s.strip_suffix("pm") {
        (d, Some(true))
    } else {
        (s.as_str(), None)
    };
    let (h, m) = match digits.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (digits.parse::<u32>().ok()?, 0),
    };
    if m > 59 {
        return None;
    }
    let h = match meridiem {
        Some(pm) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            (h % 12) + if pm { 12 } else { 0 }
        }
        None => {
            if h > 23 {
                return None;
            }
            h
        }
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

/// Grace for a reset that has just passed (CLI lag): keep it rather than
/// jumping a year (or a day) ahead.
const PAST_GRACE_MINUTES: i64 = 60;

fn next_occurrence<Z: TimeZone>(
    tz: &Z,
    md: Option<(u32, u32)>,
    time: NaiveTime,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let local_now = now.with_timezone(tz).naive_local();
    let floor = now - ChronoDuration::minutes(PAST_GRACE_MINUTES);
    let resolve = |date: NaiveDate| -> Option<DateTime<Utc>> {
        let naive = date.and_time(time);
        let local = tz
            .from_local_datetime(&naive)
            .earliest()
            // Nonexistent (DST gap): the wall clock jumps an hour forward.
            .or_else(|| tz.from_local_datetime(&(naive + ChronoDuration::hours(1))).earliest())?;
        Some(local.with_timezone(&Utc))
    };
    let candidates: Vec<NaiveDate> = match md {
        Some((m, d)) => {
            (local_now.year() - 1..=local_now.year() + 1).filter_map(|y| NaiveDate::from_ymd_opt(y, m, d)).collect()
        }
        None => {
            let today = local_now.date();
            vec![today - ChronoDuration::days(1), today, today + ChronoDuration::days(1)]
        }
    };
    candidates.into_iter().filter_map(resolve).filter(|t| *t >= floor).min()
}

/// Locate the `claude` CLI: the configured path, then `PATH`, then the usual
/// install locations (daemons under systemd/launchd get a thin `PATH`).
pub fn find_claude(explicit: Option<&Path>, home: &Path) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return p.is_file().then(|| p.to_path_buf());
    }
    let names: &[&str] = if cfg!(windows) { &["claude.exe", "claude.cmd", "claude"] } else { &["claude"] };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for n in names {
                let c = dir.join(n);
                if c.is_file() {
                    return Some(c);
                }
            }
        }
    }
    let mut extra = vec![
        home.join(".local/bin"),
        home.join(".claude/local"),
        home.join(".npm-global/bin"),
        home.join(".bun/bin"),
        home.join(".volta/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
    ];
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            extra.push(PathBuf::from(appdata).join("npm"));
        }
        extra.push(home.join(".local\\bin"));
    }
    extra.into_iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|c| c.is_file())
}

/// Default per-call timeout for the CLI (it answers in about a second).
pub const CLI_TIMEOUT: Duration = Duration::from_secs(30);

/// Build the command for one account. `~/.claude` (the CLI's own default) runs
/// with `CLAUDE_CONFIG_DIR` unset, because setting it changes where the CLI
/// looks for `.claude.json` and which Keychain item it reads on macOS.
pub fn usage_command(claude: &Path, config_dir: &Path, home: &Path) -> Command {
    let mut cmd = Command::new(claude);
    cmd.args(["--output-format", "json", "-p", "/usage"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if config_dir == home.join(".claude") {
        cmd.env_remove("CLAUDE_CONFIG_DIR");
    } else {
        cmd.env("CLAUDE_CONFIG_DIR", config_dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Run `/usage` for one account (blocking) and return the CLI's stdout.
pub fn run_usage_cli(claude: &Path, config_dir: &Path, home: &Path, timeout: Duration) -> Result<String, UsageError> {
    let mut child = usage_command(claude, config_dir, home).spawn().map_err(|e| UsageError::Spawn(e.to_string()))?;
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let out_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(UsageError::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(UsageError::Spawn(e.to_string())),
        }
    };
    let out = out_reader.join().unwrap_or_default();
    let err = err_reader.join().unwrap_or_default();
    if !status.success() && !out.contains('{') {
        if let Some(e) = classify_auth(&err).or_else(|| classify_auth(&out)) {
            return Err(e);
        }
        return Err(UsageError::BadOutput(format!("exit {status}: {}", snippet(&err))));
    }
    Ok(out)
}

/// Run and parse `/usage` for one account.
pub fn poll_account(
    claude: &Path,
    config_dir: &Path,
    home: &Path,
    now: DateTime<Utc>,
) -> Result<UsageReading, UsageError> {
    let out = run_usage_cli(claude, config_dir, home, CLI_TIMEOUT)?;
    parse_cli_output(&out, now)
}

/// Consecutive misses after which an account's numbers count as stale.
pub const STALE_AFTER_MISSES: u32 = 3;

/// Per-account memory across polls: last good reading, its age, and misses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountState {
    /// Last successful reading, kept through misses.
    pub last: Option<UsageReading>,
    /// When `last` was read (unix seconds).
    pub last_ok_at: Option<i64>,
    /// Consecutive failed polls since `last_ok_at`.
    pub misses: u32,
    /// The most recent error, for the tray.
    pub last_error: Option<UsageError>,
}

impl AccountState {
    /// Record a poll outcome.
    pub fn record(&mut self, now: i64, result: Result<UsageReading, UsageError>) {
        match result {
            Ok(r) => {
                self.last = Some(r);
                self.last_ok_at = Some(now);
                self.misses = 0;
                self.last_error = None;
            }
            Err(e) => {
                self.misses = self.misses.saturating_add(1);
                self.last_error = Some(e);
            }
        }
    }

    /// Three misses in a row (or never read at all after three tries).
    pub fn is_stale(&self) -> bool {
        self.misses >= STALE_AFTER_MISSES
    }

    /// Seconds since the last good reading.
    pub fn age(&self, now: i64) -> Option<i64> {
        self.last_ok_at.map(|t| (now - t).max(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn reset_basic_formats() {
        let now = at("2026-10-13T05:40:00Z");
        assert_eq!(parse_reset("Oct 13, 7am (Atlantic/Reykjavik)", now), Some(at("2026-10-13T07:00:00Z")));
        assert_eq!(parse_reset("Oct 13, 9:59am (Atlantic/Reykjavik)", now), Some(at("2026-10-13T09:59:00Z")));
        assert_eq!(parse_reset("Oct 18, 10pm (Atlantic/Reykjavik)", now), Some(at("2026-10-18T22:00:00Z")));
        assert_eq!(parse_reset("Oct 13, 12am (UTC)", now), Some(at("2027-10-13T00:00:00Z")));
        assert_eq!(parse_reset("Oct 13, 12pm (UTC)", now), Some(at("2026-10-13T12:00:00Z")));
        assert_eq!(parse_reset("Oct 13 at 7 PM (Europe/Berlin)", now), Some(at("2026-10-13T17:00:00Z")));
        assert_eq!(parse_reset("Oct 13, 19:00 (America/New_York)", now), Some(at("2026-10-13T23:00:00Z")));
        assert_eq!(parse_reset("October 14, 2:59 p.m. (Atlantic/Reykjavik)", now), Some(at("2026-10-14T14:59:00Z")));
    }

    #[test]
    fn reset_year_rollover_and_grace() {
        // Dec 31 local, reset "Jan 1" is next year.
        let now = at("2026-12-31T20:00:00Z");
        assert_eq!(parse_reset("Jan 1, 3am (UTC)", now), Some(at("2027-01-01T03:00:00Z")));
        // Just passed (CLI lag) stays, does not jump a year.
        let now = at("2026-10-13T07:10:00Z");
        assert_eq!(parse_reset("Oct 13, 7am (UTC)", now), Some(at("2026-10-13T07:00:00Z")));
        // Long passed: next year.
        let now = at("2026-10-13T12:00:00Z");
        assert_eq!(parse_reset("Oct 13, 7am (UTC)", now), Some(at("2027-10-13T07:00:00Z")));
    }

    #[test]
    fn reset_time_only_and_zone_offsets() {
        let now = at("2026-10-13T20:00:00Z");
        assert_eq!(parse_reset("10pm (UTC)", now), Some(at("2026-10-13T22:00:00Z")));
        assert_eq!(parse_reset("3am (UTC)", now), Some(at("2026-10-14T03:00:00Z")));
        // Zone east of UTC: local date differs from UTC date.
        let now = at("2026-10-13T22:30:00Z"); // already Oct 14 in Tokyo
        assert_eq!(parse_reset("Oct 14, 9am (Asia/Tokyo)", now), Some(at("2026-10-14T00:00:00Z")));
    }

    #[test]
    fn reset_dst_gap_and_bad_input() {
        let now = at("2026-03-01T00:00:00Z");
        // 2:30am on Mar 29 doesn't exist in Berlin; take 3:30 CEST = 01:30Z.
        assert_eq!(parse_reset("Mar 29, 2:30am (Europe/Berlin)", now), Some(at("2026-03-29T01:30:00Z")));
        assert_eq!(parse_reset("Oct 13, 7am (Not/AZone)", now), None);
        assert_eq!(parse_reset("someday", now), None);
        assert_eq!(parse_reset("Oct 13, 13pm (UTC)", now), None);
        assert_eq!(parse_reset("Feb 30, 1am (UTC)", now), None);
    }

    #[test]
    fn account_state_staleness() {
        let mut a = AccountState::default();
        a.record(100, Ok(UsageReading::default()));
        assert!(!a.is_stale());
        a.record(160, Err(UsageError::Timeout));
        a.record(220, Err(UsageError::Timeout));
        assert!(!a.is_stale());
        assert!(a.last.is_some(), "a miss keeps the last values");
        assert_eq!(a.age(220), Some(120));
        a.record(280, Err(UsageError::NoUsageLines("x".into())));
        assert!(a.is_stale());
        a.record(340, Ok(UsageReading::default()));
        assert!(!a.is_stale());
    }

    #[test]
    fn command_env_for_default_and_extra_dirs() {
        let home = Path::new("/home/u");
        let c = usage_command(Path::new("/bin/claude"), &home.join(".claude"), home);
        let envs: Vec<_> = c.get_envs().collect();
        assert!(envs.iter().any(|(k, v)| *k == "CLAUDE_CONFIG_DIR" && v.is_none()));
        let c = usage_command(Path::new("/bin/claude"), &home.join(".claude-work"), home);
        let envs: Vec<_> = c.get_envs().collect();
        assert!(envs
            .iter()
            .any(|(k, v)| *k == "CLAUDE_CONFIG_DIR" && *v == Some(home.join(".claude-work").as_os_str())));
        let args: Vec<_> = c.get_args().collect();
        assert_eq!(args, ["--output-format", "json", "-p", "/usage"]);
    }
}
