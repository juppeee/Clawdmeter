//! Usage from one tiny message's rate-limit headers, and the rule for when
//! sending that message is allowed.
//!
//! `/usage` asks Claude Code's usage endpoint, which rate-limits per token
//! (a 429 blanks its limits for ~10 min). Every normal API response carries
//! the same numbers in its headers, and `claude -p --output-format
//! stream-json --verbose` prints them as a `rate_limit_event`:
//!
//! ```text
//! {"type":"rate_limit_event","rate_limit_info":{"status":"allowed",…,
//!  "isUsingOverage":false,"unifiedWindows":{
//!    "five_hour":{"utilization":0.32,"resetsAt":1791945600},
//!    "seven_day":{"utilization":0.2,"resetsAt":1792360800}}}}
//! ```
//!
//! The probe ([`probe_command`]) sends one short Haiku message with no
//! tools, hooks, MCP servers, memory or saved session, from a fixed working
//! directory.
//!
//! **A message starts the 5-hour window when none is running**, and usage
//! monitoring must never do that. [`choose_source`] allows the probe only
//! while the account's window is known to be running: its last known reset
//! is still ahead (by [`PROBE_MARGIN_SECS`]), or the user's own prompt or
//! tool activity from that account started one after it. A window that
//! ended with no activity since needs no call at all: the session is not
//! started ([`not_started`]). Anything less certain goes to `/usage`, which
//! answers locally and starts nothing, on a slower cadence
//! ([`crate::cadence::Cadence::no_window`]).

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::usage::{self, AccountState, UsageError, UsageReading, Window};

/// The session window's length.
pub const WINDOW_SECS: i64 = 5 * 3600;
/// How close to a window's end the probe stops: a message that lands after
/// the reset would start the next window. `/usage` covers the gap.
pub const PROBE_MARGIN_SECS: i64 = 5 * 60;
/// The message the probe sends.
pub const PROBE_PROMPT: &str = "Reply with just: ok";
/// Claude Code settings for the probe: none of the user's hooks (they would
/// feed our own events log), no auto memory.
pub const PROBE_SETTINGS: &str = r#"{"disableAllHooks":true,"autoMemoryEnabled":false}"#;

/// Where one account's next reading comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Send the probe message: the window is known to be running.
    Probe,
    /// Run `/usage` (no message; under the account's backoff).
    Usage,
    /// No call: the window ended and nothing started another. The reading
    /// is [`not_started`] of the last one.
    NotStarted(UsageReading),
}

/// The time after which the last reading no longer tells whether a window
/// is running: its session reset, or when it was read if no window was
/// running then. Activity after this started a window. `None` before the
/// first reading.
pub fn anchor(st: &AccountState) -> Option<i64> {
    let last = st.last.as_ref()?;
    let read = st.read_started_at.or(st.last_ok_at)?;
    let reset = last.session.as_ref().and_then(|w| w.resets_at).map(|t| t.timestamp());
    Some(reset.map_or(read, |r| r.max(read)))
}

/// Pick the source for one account at `now`. `unattributed` is the latest
/// prompt or tool activity that couldn't be tied to an account.
pub fn choose_source(st: &AccountState, unattributed: Option<i64>, now: i64) -> Source {
    let Some(last) = &st.last else { return Source::Usage };
    // Spend-capped accounts: the headers carry no spend cap.
    if last.is_spend_only() {
        return Source::Usage;
    }
    let anchor = anchor(st).unwrap_or(now);
    if let Some(reset) = last.session.as_ref().and_then(|w| w.resets_at).map(|t| t.timestamp()) {
        if reset - now > PROBE_MARGIN_SECS {
            return Source::Probe;
        }
        if reset > now {
            return Source::Usage;
        }
    }
    if let Some(first) = st.first_activity {
        // That activity started a window; it runs WINDOW_SECS from there.
        return if now < first + WINDOW_SECS - PROBE_MARGIN_SECS { Source::Probe } else { Source::Usage };
    }
    // The known window just ended and nothing here started another: show
    // it over at once. After that, `/usage` keeps watch (a window can start
    // on another computer, where no hook of ours runs).
    if last.window_started() && unattributed.is_none_or(|t| t <= anchor) {
        return Source::NotStarted(not_started(last, now));
    }
    Source::Usage
}

/// `last` with the session window over: not started, and any weekly window
/// whose reset has passed back at 0 % with no reset.
pub fn not_started(last: &UsageReading, now: i64) -> UsageReading {
    let over = |w: &Window| w.resets_at.is_some_and(|t| t.timestamp() <= now);
    let reset = |w: &Window| if over(w) { Window { pct: 0, resets_at: None } } else { w.clone() };
    UsageReading {
        session: None,
        week: last.week.as_ref().map(reset),
        models: last.models.iter().map(|(m, w)| (m.clone(), reset(w))).collect(),
        spend: last.spend.clone(),
        overage: false,
    }
}

/// The probe for one account, run from `cwd` (a fixed directory, so Claude
/// Code keeps no per-project state for each place the app starts from).
pub fn probe_command(claude: &Path, config_dir: &Path, home: &Path, cwd: &Path) -> Command {
    let mut cmd = Command::new(claude);
    cmd.args([
        "-p",
        "--model",
        "haiku",
        "--output-format",
        "stream-json",
        "--verbose",
        "--no-session-persistence",
        "--strict-mcp-config",
        "--tools",
        "",
        "--settings",
        PROBE_SETTINGS,
        PROBE_PROMPT,
    ])
    .current_dir(cwd);
    usage::for_account(&mut cmd, config_dir, home);
    cmd
}

/// Read the last `rate_limit_event` with `unifiedWindows` out of the
/// probe's stream-json output.
pub fn parse_stream(stdout: &str) -> Result<UsageReading, UsageError> {
    let info = stdout
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .filter(|v| v.get("type").and_then(Value::as_str) == Some("rate_limit_event"))
        .filter_map(|v| v.get("rate_limit_info").cloned())
        .find(|i| i.get("unifiedWindows").is_some_and(Value::is_object));
    let Some(info) = info else {
        return Err(usage::classify_auth(stdout)
            .unwrap_or_else(|| UsageError::BadOutput("no rate_limit_event with unifiedWindows".into())));
    };
    let windows = &info["unifiedWindows"];
    let window = |key: &str| -> Option<Window> {
        let w = windows.get(key)?;
        let used = w.get("utilization")?.as_f64()?;
        let resets_at = w.get("resetsAt").and_then(Value::as_i64).and_then(|t| DateTime::<Utc>::from_timestamp(t, 0));
        Some(Window { pct: (used * 100.0).round().max(0.0) as u32, resets_at })
    };
    let reading = UsageReading {
        session: window("five_hour"),
        week: window("seven_day"),
        models: Vec::new(),
        spend: None,
        overage: info.get("isUsingOverage").and_then(Value::as_bool).unwrap_or(false),
    };
    if reading.session.is_none() && reading.week.is_none() {
        return Err(UsageError::BadOutput("rate_limit_event without five_hour or seven_day".into()));
    }
    Ok(reading)
}

/// Send the probe for one account (blocking) and parse it.
pub fn run_probe(
    claude: &Path,
    config_dir: &Path,
    home: &Path,
    cwd: &Path,
    timeout: Duration,
) -> Result<UsageReading, UsageError> {
    parse_stream(&usage::run_cli(probe_command(claude, config_dir, home, cwd), timeout)?)
}

/// What [`poll`] does, chosen by the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// The probe; on a miss, `/usage` when `usage_fallback` (the account
    /// isn't backing off).
    Probe {
        /// Fall back to `/usage` when the probe misses.
        usage_fallback: bool,
    },
    /// `/usage` only.
    Usage,
    /// This reading, no call ([`Source::NotStarted`]).
    Known(UsageReading),
}

/// Read one account by `plan`. `cwd` is where the probe runs.
pub fn poll(
    claude: &Path,
    config_dir: &Path,
    home: &Path,
    cwd: &Path,
    plan: &Plan,
    now: DateTime<Utc>,
) -> Result<UsageReading, UsageError> {
    match plan {
        Plan::Known(r) => Ok(r.clone()),
        Plan::Usage => usage::poll_account(claude, config_dir, home, now),
        Plan::Probe { usage_fallback } => match run_probe(claude, config_dir, home, cwd, usage::CLI_TIMEOUT) {
            Ok(r) => Ok(r),
            Err(e) if e.is_auth() || !usage_fallback => Err(e),
            Err(e) => {
                log::info!("{}: probe missed ({e}); asking /usage", config_dir.display());
                usage::poll_account(claude, config_dir, home, now)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_791_930_000;

    fn ts(t: i64) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp(t, 0)
    }

    fn reading(session: Option<(u32, Option<i64>)>, week: Option<(u32, Option<i64>)>) -> UsageReading {
        let w = |(pct, r): (u32, Option<i64>)| Window { pct, resets_at: r.and_then(ts) };
        UsageReading { session: session.map(w), week: week.map(w), ..Default::default() }
    }

    fn state(last: UsageReading, read_at: i64) -> AccountState {
        AccountState {
            last: Some(last),
            last_ok_at: Some(read_at),
            read_started_at: Some(read_at),
            ..Default::default()
        }
    }

    #[test]
    fn parses_the_sample_stream() {
        let text = include_str!("../tests/fixtures/probe-stream.jsonl");
        let r = parse_stream(text).unwrap();
        assert_eq!(r.session, Some(Window { pct: 32, resets_at: ts(1_791_945_600) }));
        assert_eq!(r.week, Some(Window { pct: 20, resets_at: ts(1_792_360_800) }));
        assert!(!r.overage);
        assert!(r.window_started());
    }

    #[test]
    fn stream_edge_cases() {
        let ev = |info: &str| {
            format!("{{\"type\":\"system\"}}\n{{\"type\":\"rate_limit_event\",\"rate_limit_info\":{info}}}\n")
        };
        // The last event with windows wins; rounding; over 100 %.
        let two = ev(r#"{"unifiedWindows":{"five_hour":{"utilization":0.1,"resetsAt":10}}}"#)
            + &ev(r#"{"isUsingOverage":true,"unifiedWindows":{"five_hour":{"utilization":1.054,"resetsAt":20}}}"#)
            + &ev(r#"{"status":"allowed"}"#);
        let r = parse_stream(&two).unwrap();
        assert_eq!(r.session, Some(Window { pct: 105, resets_at: ts(20) }));
        assert_eq!(r.week, None);
        assert!(r.overage);
        // No event, an event without windows, empty windows: a miss.
        assert!(matches!(parse_stream("{\"type\":\"result\"}\n"), Err(UsageError::BadOutput(_))));
        assert!(matches!(parse_stream(&ev(r#"{"status":"allowed"}"#)), Err(UsageError::BadOutput(_))));
        assert!(matches!(parse_stream(&ev(r#"{"unifiedWindows":{}}"#)), Err(UsageError::BadOutput(_))));
        assert!(parse_stream("not json\n").is_err());
        // A login problem says so.
        assert!(parse_stream("Not logged in · Please run /login\n").unwrap_err().is_auth());
    }

    #[test]
    fn unknown_or_spend_capped_accounts_use_usage() {
        assert_eq!(choose_source(&AccountState::default(), None, T0), Source::Usage);
        let spend = UsageReading {
            spend: Some(usage::Spend { pct: 40, resets_at: ts(T0 + 86_400), period: None }),
            ..Default::default()
        };
        assert_eq!(choose_source(&state(spend, T0 - 60), None, T0), Source::Usage);
    }

    #[test]
    fn probes_only_while_the_window_runs() {
        let st = state(reading(Some((30, Some(T0 + 3600))), Some((10, Some(T0 + 86_400)))), T0 - 60);
        assert_eq!(choose_source(&st, None, T0), Source::Probe);
        // Inside the margin before the reset: a message could land after it.
        assert_eq!(choose_source(&st, None, T0 + 3600 - PROBE_MARGIN_SECS), Source::Usage);
        assert_eq!(choose_source(&st, None, T0 + 3600 - PROBE_MARGIN_SECS - 1), Source::Probe);
        assert_eq!(choose_source(&st, None, T0 + 3599), Source::Usage);
    }

    #[test]
    fn a_window_that_ended_quietly_needs_no_call() {
        let st = state(reading(Some((30, Some(T0))), Some((10, Some(T0 + 86_400)))), T0 - 600);
        // At the reset second and after: over.
        for now in [T0, T0 + 1, T0 + 7200] {
            let Source::NotStarted(r) = choose_source(&st, None, now) else { panic!("{now}") };
            assert_eq!(r.session, None);
            assert_eq!(r.week, Some(Window { pct: 10, resets_at: ts(T0 + 86_400) }), "weekly kept");
        }
        // Past the weekly reset too: back at 0 % with no reset.
        let Source::NotStarted(r) = choose_source(&st, None, T0 + 86_400) else { panic!() };
        assert_eq!(r.week, Some(Window { pct: 0, resets_at: None }));
        // Already not started at the last reading: /usage keeps watch.
        let idle = state(reading(None, Some((10, Some(T0 + 86_400)))), T0 - 600);
        assert_eq!(choose_source(&idle, None, T0), Source::Usage);
    }

    #[test]
    fn the_users_own_activity_opens_the_probe() {
        let mut st = state(reading(Some((30, Some(T0))), None), T0 - 600);
        // Activity at or before the reset belongs to the old window.
        st.note_activity(T0 - 5);
        st.note_activity(T0);
        assert_eq!(st.first_activity, None);
        assert!(matches!(choose_source(&st, None, T0 + 60), Source::NotStarted(_)));
        st.note_activity(T0 + 30);
        st.note_activity(T0 + 90);
        assert_eq!(st.first_activity, Some(T0 + 30), "the first one started the window");
        assert_eq!(choose_source(&st, None, T0 + 60), Source::Probe);
        // That window ends 5 h after it began; near its end, /usage.
        assert_eq!(choose_source(&st, None, T0 + 30 + WINDOW_SECS - PROBE_MARGIN_SECS - 1), Source::Probe);
        assert_eq!(choose_source(&st, None, T0 + 30 + WINDOW_SECS - PROBE_MARGIN_SECS), Source::Usage);
        assert_eq!(choose_source(&st, None, T0 + 30 + WINDOW_SECS + 60), Source::Usage);
    }

    #[test]
    fn activity_from_an_unknown_account_means_usage() {
        let st = state(reading(Some((30, Some(T0 - 60))), None), T0 - 600);
        assert!(matches!(choose_source(&st, Some(T0 - 61), T0), Source::NotStarted(_)), "before the reset");
        assert_eq!(choose_source(&st, Some(T0 - 30), T0), Source::Usage);
    }

    #[test]
    fn a_new_reading_moves_the_anchor() {
        let mut st = state(reading(None, None), T0 - 600);
        st.last_attempt_at = Some(T0 - 600);
        st.note_activity(T0 - 300);
        assert_eq!(st.first_activity, Some(T0 - 300));
        // The next reading saw the window that activity started.
        st.last_attempt_at = Some(T0 - 10);
        st.record(T0, Ok(reading(Some((5, Some(T0 + 4 * 3600))), None)));
        assert_eq!(st.first_activity, None);
        assert_eq!(anchor(&st), Some(T0 + 4 * 3600));
        assert_eq!(choose_source(&st, None, T0 + 60), Source::Probe);
        // A not-started reading keeps activity that came after its poll began.
        let mut st = state(reading(None, None), T0 - 600);
        st.note_activity(T0 - 5);
        st.last_attempt_at = Some(T0 - 10);
        st.record(T0, Ok(reading(None, None)));
        assert_eq!(st.first_activity, Some(T0 - 5));
    }
}
