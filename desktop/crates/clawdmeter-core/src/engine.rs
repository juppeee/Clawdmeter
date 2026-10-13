//! The daemon's decisions, with no I/O: what to poll, when, and what to write
//! to the device. The runtime (app crate) feeds it events, poll results and
//! the clock, and performs the writes it asks for.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{DateTime, TimeZone, Utc};

use crate::attention::{Alert, AttentionState};
use crate::cadence::{heartbeat_due, Cadence, PollReason, Scheduler};
use crate::config::Settings;
use crate::events::{Activity, HookEvent, Signal};
use crate::payload::{clock_fields, Payload};
use crate::rate::{Mood, RateTracker};
use crate::selector::PlanSelector;
use crate::usage::{AccountState, UsageError, UsageReading};

/// `a` changes are sent at most this often (s).
pub const ACTIVITY_MIN_INTERVAL: i64 = 5;
/// `e`/`es` ride along for this long after the event (s), so a device that
/// reconnects or reboots later doesn't react to an old event.
pub const EVENT_FRESH_SECS: i64 = 60;

/// Why a write happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteKind {
    /// Fresh `/usage` numbers.
    Poll,
    /// Attention alert or clear.
    Attention,
    /// A hook event / activity / session-count change.
    Event,
    /// Keep-alive replay.
    Heartbeat,
    /// Every account is logged out: `{"ok":false}`.
    NoData,
}

/// A write the runtime should perform.
#[derive(Debug, Clone, PartialEq)]
pub struct Write {
    /// What to send.
    pub payload: Payload,
    /// Why.
    pub kind: WriteKind,
}

/// Host facts the payload needs at write time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostClock {
    /// Local UTC offset (s), for `t`.
    pub utc_offset_secs: i64,
    /// `clock = auto` resolves to 12h.
    pub prefers_12h: bool,
}

/// Snapshot for the tray / status output.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EngineStatus {
    /// Account shown on the device.
    pub active_account: Option<PathBuf>,
    /// Its latest numbers.
    pub reading: Option<UsageReading>,
    /// Seconds since those numbers were read.
    pub age_secs: Option<i64>,
    /// Three misses in a row on the shown account.
    pub stale: bool,
    /// Every account failed with a login problem.
    pub logged_out: bool,
    /// Per-account last error, for the settings window.
    pub errors: Vec<(PathBuf, String)>,
    /// An attention alert is up.
    pub attention: bool,
    /// Hook activity level 0-3.
    pub activity: u8,
    /// Sessions active in the last 5 min.
    pub sessions: u32,
    /// Hooks have been seen (fast clock available).
    pub hooks: bool,
    /// Mood from the rate math.
    pub mood: Option<Mood>,
    /// Unix seconds of the next scheduled poll.
    pub next_poll_at: Option<i64>,
}

/// The daemon state machine.
#[derive(Debug)]
pub struct Engine {
    settings: Settings,
    scheduler: Scheduler,
    activity: Activity,
    attention: AttentionState,
    selector: PlanSelector<PathBuf>,
    accounts: BTreeMap<PathBuf, AccountState>,
    active: Option<PathBuf>,
    rate: RateTracker,
    hooks_installed: bool,
    fresh_poll: bool,
    logged_out: bool,
    last_write: Option<i64>,
    sent_seq: u32,
    sent_a: Option<u8>,
    sent_a_at: i64,
    sent_n: Option<u32>,
    last_event_ts: Option<i64>,
}

fn utc(now: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(now, 0).single().unwrap_or_default()
}

impl Engine {
    /// A fresh engine; the first `poll_due` fires immediately.
    pub fn new(settings: Settings) -> Self {
        let cadence = Cadence::from_settings(settings.poll_interval, settings.poll_interval_explicit);
        Engine {
            settings,
            scheduler: Scheduler::new(cadence),
            activity: Activity::default(),
            attention: AttentionState::default(),
            selector: PlanSelector::default(),
            accounts: BTreeMap::new(),
            active: None,
            rate: RateTracker::default(),
            hooks_installed: false,
            fresh_poll: false,
            logged_out: false,
            last_write: None,
            sent_seq: 0,
            sent_a: None,
            sent_a_at: i64::MIN / 2,
            sent_n: None,
            last_event_ts: None,
        }
    }

    /// Current settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Apply re-read settings (the config is re-read every few seconds).
    pub fn update_settings(&mut self, settings: Settings) {
        if settings == self.settings {
            return;
        }
        self.scheduler.cadence = Cadence::from_settings(settings.poll_interval, settings.poll_interval_explicit);
        if settings.config_dirs != self.settings.config_dirs {
            self.accounts.retain(|k, _| settings.config_dirs.contains(k));
            self.scheduler.request();
        }
        self.settings = settings;
    }

    /// Whether events.log exists.
    pub fn set_hooks_installed(&mut self, installed: bool) {
        self.hooks_installed = installed;
    }

    /// History from events.log at start-up (no `e` is emitted for it).
    pub fn seed_events(&mut self, events: &[HookEvent]) {
        self.activity.seed(events);
    }

    /// One live hook event.
    pub fn on_hook_event(&mut self, ev: &HookEvent, now: i64) {
        match self.activity.ingest(ev) {
            Signal::Prompt { .. } => {
                self.scheduler.on_prompt();
                self.last_event_ts = Some(now);
            }
            Signal::Stop => {
                self.scheduler.on_stop(now);
                self.last_event_ts = Some(now);
            }
            Signal::Tool | Signal::Other => {}
        }
    }

    /// One tick's attention files.
    pub fn on_attention(&mut self, alert: Option<Alert>, clears: &HashMap<String, SystemTime>) {
        self.attention.ingest(alert, clears);
    }

    /// The device (REQ notify) or the user asked for fresh numbers.
    pub fn request_refresh(&mut self) {
        self.scheduler.request();
    }

    /// A (re)connection: resend everything the device needs.
    pub fn on_connected(&mut self) {
        self.attention.reset();
        self.sent_a = None;
        self.sent_n = None;
        if self.active.is_some() || self.logged_out {
            self.fresh_poll = true;
        }
    }

    /// `Some(reason)` when a `/usage` poll should start now.
    pub fn poll_due(&self, now: i64) -> Option<PollReason> {
        self.scheduler.due(&self.activity, self.hooks_installed, now)
    }

    /// The accounts to poll.
    pub fn accounts_to_poll(&self) -> Vec<PathBuf> {
        self.settings.config_dirs.clone()
    }

    /// Results of one poll over all accounts.
    pub fn on_poll_results(&mut self, now: i64, results: Vec<(PathBuf, Result<UsageReading, UsageError>)>) {
        let any_ok = results.iter().any(|(_, r)| r.is_ok());
        let all_auth =
            !results.is_empty() && results.iter().all(|(_, r)| r.as_ref().err().is_some_and(UsageError::is_auth));
        let ok_dirs: Vec<PathBuf> = results.iter().filter(|(_, r)| r.is_ok()).map(|(d, _)| d.clone()).collect();
        for (dir, r) in results {
            self.accounts.entry(dir).or_default().record(now, r);
        }
        // Accounts that missed this cycle keep their last values, which don't
        // count as a rise, so a glitch never flips the shown account.
        let sessions: Vec<(PathBuf, u32)> =
            self.accounts.iter().filter_map(|(k, a)| a.last.as_ref().map(|r| (k.clone(), r.session_pct()))).collect();
        let prev_active = self.active.clone();
        self.active = self.selector.choose(&sessions);
        if self.active != prev_active && self.settings.config_dirs.len() > 1 {
            if let Some(a) = &self.active {
                log::info!("active plan: {}", a.display());
            }
        }
        // Only numbers read in this cycle for the shown account are a real
        // poll. When it missed (another account answered), its last values
        // must not enter the rate math, here or on the device: no `Poll`
        // write, so they would only ride along on a replay.
        let active_fresh = self.active.as_ref().is_some_and(|a| ok_dirs.contains(a));
        if active_fresh {
            if let Some(r) = self.active_reading() {
                let pct = r.session_pct();
                self.rate.sample(now, pct, false);
            }
        }
        self.logged_out = all_auth;
        self.fresh_poll = active_fresh || all_auth;
        self.scheduler.polled(now, any_ok);
    }

    fn active_state(&self) -> Option<&AccountState> {
        self.active.as_ref().and_then(|a| self.accounts.get(a))
    }

    fn active_reading(&self) -> Option<&UsageReading> {
        self.active_state().and_then(|a| a.last.as_ref())
    }

    /// The shown account has missed three polls in a row.
    pub fn stale(&self) -> bool {
        self.active_state().is_some_and(AccountState::is_stale)
    }

    /// The next write, if one is due. Call [`Engine::mark_written`] after a
    /// successful write; on failure just call this again next tick.
    pub fn next_write(&mut self, now: i64, host: HostClock) -> Option<Write> {
        self.activity.prune(now);
        let level = self.activity.level(now);
        let sessions = self.activity.sessions_active(now);
        let hooks = self.activity.hooks_seen;
        let event_fresh = self.last_event_ts.is_some_and(|t| now - t < EVENT_FRESH_SECS);

        let new_event = event_fresh && self.activity.seq != self.sent_seq;
        let a_changed = hooks && self.sent_a != Some(level) && now - self.sent_a_at >= ACTIVITY_MIN_INTERVAL;
        let n_changed = hooks && self.sent_n != Some(sessions);

        let (mut payload, kind) = if self.logged_out {
            if !(self.fresh_poll || self.attention.pending()) {
                return None;
            }
            (Payload::no_data(), WriteKind::NoData)
        } else {
            // Nothing to show until the first good poll (the old daemons
            // also held attention until then).
            let reading = self.active_reading()?.clone();
            let kind = if self.fresh_poll {
                WriteKind::Poll
            } else if self.attention.pending() {
                WriteKind::Attention
            } else if (new_event || a_changed || n_changed) && !self.stale() {
                // Any ok payload refreshes the device's DATA_FRESH_MS, so
                // while the numbers are stale only alerts and polls go out
                // and the device can show its stale state.
                WriteKind::Event
            } else if !self.stale() && heartbeat_due(self.last_write, self.settings.heartbeat_interval, now) {
                WriteKind::Heartbeat
            } else {
                return None;
            };
            let mut p = Payload::from_reading(&reading, utc(now));
            if self.settings.chime {
                p.c = Some(1);
            }
            if let Some(cf) = clock_fields(self.settings.clock, utc(now), host.utc_offset_secs, host.prefers_12h) {
                p.t = Some(cf.local_epoch);
                p.tf = Some(cf.hour_format);
            }
            if kind != WriteKind::Poll {
                p.rp = Some(1);
            }
            (p, kind)
        };

        let att = self.attention.fields(self.settings.beep);
        if att.beep {
            payload.b = Some(1);
        }
        payload.m = att.message;
        if att.clear {
            payload.x = Some(1);
        }
        if hooks {
            payload.a = Some(level);
            payload.n = Some(sessions);
            if event_fresh {
                if let Some(code) = self.activity.latest {
                    payload.e = Some(code.as_str().to_owned());
                    payload.es = Some(self.activity.seq);
                }
            }
        }
        Some(Write { payload, kind })
    }

    /// The write from [`Engine::next_write`] reached the device.
    pub fn mark_written(&mut self, now: i64, w: &Write) {
        self.last_write = Some(now);
        self.fresh_poll = false;
        if w.payload.b.is_some() || w.payload.m.is_some() || w.payload.x.is_some() || self.attention.pending() {
            self.attention.mark_sent();
        }
        if let Some(es) = w.payload.es {
            self.sent_seq = es;
        }
        if let Some(a) = w.payload.a {
            if self.sent_a != Some(a) {
                self.sent_a_at = now;
            }
            self.sent_a = Some(a);
        }
        if let Some(n) = w.payload.n {
            self.sent_n = Some(n);
        }
    }

    /// Snapshot for the UI.
    pub fn status(&self, now: i64) -> EngineStatus {
        let st = self.active_state();
        EngineStatus {
            active_account: self.active.clone(),
            reading: st.and_then(|a| a.last.clone()),
            age_secs: st.and_then(|a| a.age(now)),
            stale: self.stale(),
            logged_out: self.logged_out,
            errors: self
                .accounts
                .iter()
                .filter_map(|(k, a)| a.last_error.as_ref().map(|e| (k.clone(), e.user_message())))
                .collect(),
            attention: self.attention.active(),
            activity: self.activity.level(now),
            sessions: self.activity.sessions_active(now),
            hooks: self.activity.hooks_seen,
            mood: st.and_then(|a| a.last.as_ref()).map(|_| self.rate.mood(self.activity.level(now))),
            next_poll_at: Some(self.scheduler.next_due(&self.activity, self.hooks_installed, now).0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigFile, Settings};
    use crate::events::HookKind;
    use crate::paths::Paths;
    use crate::usage::Window;
    use std::path::Path;
    use std::time::Duration;

    const T0: i64 = 1_760_000_000;

    fn settings(text: &str) -> Settings {
        Settings::from_config(&ConfigFile::parse(text), &Paths::under(Path::new("/h")), None)
    }

    fn reading(s: u32) -> UsageReading {
        UsageReading {
            session: Some(Window { pct: s, resets_at: Some(utc(T0 + 3600)) }),
            week: Some(Window { pct: 10, resets_at: Some(utc(T0 + 86_400)) }),
            ..Default::default()
        }
    }

    fn ev(ts: i64, kind: &str) -> HookEvent {
        HookEvent { ts, kind: HookKind::parse(kind), session: "s1".into() }
    }

    fn write(e: &mut Engine, now: i64) -> Option<Write> {
        let w = e.next_write(now, HostClock::default())?;
        e.mark_written(now, &w);
        Some(w)
    }

    #[test]
    fn nothing_before_first_poll_then_poll_write() {
        let mut e = Engine::new(settings(""));
        assert_eq!(e.poll_due(T0), Some(PollReason::Startup));
        assert!(write(&mut e, T0).is_none());
        e.on_poll_results(T0, vec![(PathBuf::from("/h/.claude"), Ok(reading(15)))]);
        let w = write(&mut e, T0).unwrap();
        assert_eq!(w.kind, WriteKind::Poll);
        assert_eq!(w.payload.to_json(), r#"{"s":15,"sr":60,"w":10,"wr":1440,"st":"allowed","acct":"pro","ok":true}"#);
        assert!(write(&mut e, T0 + 1).is_none());
    }

    // Heartbeat replays (bash parity + rp marker), and none once stale.
    #[test]
    fn heartbeat_replays_with_aged_countdowns() {
        let mut e = Engine::new(settings("chime = on\n"));
        let dir = PathBuf::from("/h/.claude");
        e.on_poll_results(T0, vec![(dir.clone(), Ok(reading(15)))]);
        write(&mut e, T0).unwrap();
        assert!(write(&mut e, T0 + 59).is_none());
        let hb = write(&mut e, T0 + 300).unwrap();
        assert_eq!(hb.kind, WriteKind::Heartbeat);
        assert_eq!(
            hb.payload.to_json(),
            r#"{"s":15,"sr":55,"w":10,"wr":1435,"st":"allowed","acct":"pro","c":1,"ok":true,"rp":1}"#
        );
        // Three misses: stale, heartbeats stop so the device's freshness lapses.
        for i in 1..=3 {
            e.on_poll_results(T0 + 300 + i, vec![(dir.clone(), Err(UsageError::Timeout))]);
        }
        assert!(e.stale());
        assert!(write(&mut e, T0 + 1000).is_none());
        assert_eq!(e.status(T0 + 1000).reading, Some(reading(15)), "last values kept");
    }

    #[test]
    fn multi_account_selection_and_glitch_keeps_choice() {
        let mut e = Engine::new(settings("config_dirs = ~/.claude, ~/.claude-work\n"));
        let a = PathBuf::from("/h/.claude");
        let b = PathBuf::from("/h/.claude-work");
        assert_eq!(e.accounts_to_poll(), vec![a.clone(), b.clone()]);
        e.on_poll_results(T0, vec![(a.clone(), Ok(reading(10))), (b.clone(), Ok(reading(30)))]);
        assert_eq!(e.status(T0).active_account, Some(b.clone()));
        e.on_poll_results(T0 + 60, vec![(a.clone(), Ok(reading(20))), (b.clone(), Ok(reading(30)))]);
        assert_eq!(e.status(T0).active_account, Some(a.clone()));
        // A misses: still shown with its last values.
        e.on_poll_results(T0 + 120, vec![(a.clone(), Err(UsageError::Timeout)), (b.clone(), Ok(reading(30)))]);
        assert_eq!(e.status(T0).active_account, Some(a.clone()));
        assert_eq!(e.status(T0).reading.unwrap().session_pct(), 20);
        // B tokenless/logged out doesn't matter while A works.
        e.on_poll_results(T0 + 180, vec![(a.clone(), Ok(reading(20))), (b, Err(UsageError::NotLoggedIn("x".into())))]);
        assert!(!e.status(T0 + 180).logged_out);
    }

    // A miss on the shown account is never a poll, even when another
    // account answered: its old values must not reach the rate math.
    #[test]
    fn miss_on_shown_account_is_not_a_poll() {
        let mut e = Engine::new(settings("config_dirs = ~/.claude, ~/.claude-work\n"));
        let a = PathBuf::from("/h/.claude");
        let b = PathBuf::from("/h/.claude-work");
        e.on_poll_results(T0, vec![(a.clone(), Ok(reading(20))), (b.clone(), Ok(reading(5)))]);
        assert_eq!(write(&mut e, T0).unwrap().kind, WriteKind::Poll);
        e.on_poll_results(T0 + 30, vec![(a.clone(), Err(UsageError::Timeout)), (b, Ok(reading(5)))]);
        assert_eq!(e.status(T0 + 30).active_account, Some(a));
        assert!(write(&mut e, T0 + 30).is_none(), "no poll write for the shown account's old values");
        // The keep-alive still carries them, marked as a replay.
        assert_eq!(write(&mut e, T0 + 60).unwrap().payload.rp, Some(1));
    }

    // Stale: hook events stop refreshing the device, so its freshness lapses.
    #[test]
    fn stale_suppresses_event_writes() {
        let mut e = Engine::new(settings(""));
        let dir = PathBuf::from("/h/.claude");
        e.on_poll_results(T0, vec![(dir.clone(), Ok(reading(15)))]);
        write(&mut e, T0).unwrap();
        for i in 1..=3 {
            e.on_poll_results(T0 + i, vec![(dir.clone(), Err(UsageError::Timeout))]);
        }
        e.on_hook_event(&ev(T0 + 10, "UserPromptSubmit"), T0 + 10);
        assert!(write(&mut e, T0 + 10).is_none());
    }

    #[test]
    fn all_logged_out_sends_no_data() {
        let mut e = Engine::new(settings(""));
        e.on_poll_results(T0, vec![(PathBuf::from("/h/.claude"), Err(UsageError::NotLoggedIn("x".into())))]);
        let w = write(&mut e, T0).unwrap();
        assert_eq!((w.kind, w.payload.to_json().as_str()), (WriteKind::NoData, r#"{"ok":false}"#));
        assert!(write(&mut e, T0 + 100).is_none());
    }

    // Ported from test_windows_reconnect.py::test_transient_poll_failure_does_not_set_error
    // and test_freeride.py: only a login problem on every account is "no
    // data"; a glitch (timeout, bad output, CLI missing) stays silent.
    #[test]
    fn transient_failures_are_not_no_data() {
        let mut e = Engine::new(settings("config_dirs = ~/.claude, ~/.claude-work\n"));
        let (a, b) = (PathBuf::from("/h/.claude"), PathBuf::from("/h/.claude-work"));
        e.on_poll_results(
            T0,
            vec![(a.clone(), Err(UsageError::Timeout)), (b.clone(), Err(UsageError::Spawn("x".into())))],
        );
        assert!(!e.status(T0).logged_out);
        assert!(write(&mut e, T0).is_none());
        // One logged out, one glitching: still not "no data".
        e.on_poll_results(
            T0 + 30,
            vec![(a.clone(), Err(UsageError::NotLoggedIn("x".into()))), (b.clone(), Err(UsageError::Timeout))],
        );
        assert!(write(&mut e, T0 + 30).is_none());
        // Both need a login: no data, once.
        e.on_poll_results(
            T0 + 60,
            vec![(a, Err(UsageError::NotSubscription)), (b, Err(UsageError::NotLoggedIn("x".into())))],
        );
        assert_eq!(write(&mut e, T0 + 60).unwrap().kind, WriteKind::NoData);
    }

    // A spend-capped account drives the device's enterprise layout and the
    // multi-account choice by its spend %.
    #[test]
    fn spend_only_account() {
        use crate::usage::Spend;
        let mut e = Engine::new(settings(""));
        let r = UsageReading {
            spend: Some(Spend { pct: 41, resets_at: Some(utc(T0 + 86_400)), period: None }),
            ..Default::default()
        };
        assert_eq!(r.session_pct(), 41);
        e.on_poll_results(T0, vec![(PathBuf::from("/h/.claude"), Ok(r))]);
        let w = write(&mut e, T0).unwrap();
        assert_eq!((w.payload.acct.as_deref(), w.payload.s, w.payload.pd), (Some("ent"), Some(41), Some(30)));
    }

    #[test]
    fn attention_rides_a_replay() {
        let mut e = Engine::new(settings("beep = on\n"));
        e.on_poll_results(T0, vec![(PathBuf::from("/h/.claude"), Ok(reading(15)))]);
        write(&mut e, T0).unwrap();
        let alert = Alert { message: "Claude needs you".into(), session: "abc".into(), at: SystemTime::now() };
        e.on_attention(Some(alert.clone()), &HashMap::new());
        let w = write(&mut e, T0 + 2).unwrap();
        assert_eq!(w.kind, WriteKind::Attention);
        assert_eq!((w.payload.b, w.payload.m.as_deref(), w.payload.rp), (Some(1), Some("Claude needs you"), Some(1)));
        assert!(write(&mut e, T0 + 3).is_none());
        let clears: HashMap<_, _> = [("abc".to_owned(), alert.at + Duration::from_secs(1))].into();
        e.on_attention(None, &clears);
        let w = write(&mut e, T0 + 4).unwrap();
        assert_eq!((w.payload.x, w.payload.m.clone()), (Some(1), None));
    }

    #[test]
    fn hook_events_reach_the_device() {
        let mut e = Engine::new(settings(""));
        e.set_hooks_installed(true);
        e.on_poll_results(T0, vec![(PathBuf::from("/h/.claude"), Ok(reading(15)))]);
        let w = write(&mut e, T0).unwrap();
        assert_eq!(w.payload.a, None, "no hooks seen yet");

        e.on_hook_event(&ev(T0 + 10, "UserPromptSubmit"), T0 + 10);
        assert_eq!(e.poll_due(T0 + 10), Some(PollReason::Prompt));
        let w = write(&mut e, T0 + 10).unwrap();
        assert_eq!(w.kind, WriteKind::Event, "the event goes out before the poll lands");
        assert_eq!(
            (w.payload.e.as_deref(), w.payload.es, w.payload.n, w.payload.a),
            (Some("p"), Some(1), Some(1), Some(0))
        );
        assert_eq!(w.payload.rp, Some(1));
        e.on_poll_results(T0 + 11, vec![(PathBuf::from("/h/.claude"), Ok(reading(16)))]);
        let w = write(&mut e, T0 + 11).unwrap();
        assert_eq!((w.kind, w.payload.rp, w.payload.s), (WriteKind::Poll, None, Some(16)));

        // Tool calls raise `a`, sent at most every 5 s.
        for i in 0..5 {
            e.on_hook_event(&ev(T0 + 11 + i, "PostToolUse"), T0 + 11 + i);
        }
        assert!(write(&mut e, T0 + 14).is_none());
        let w = write(&mut e, T0 + 15).unwrap();
        assert_eq!(w.payload.a, Some(2));
        // `e` keeps riding along but doesn't trigger writes by itself.
        assert_eq!(w.payload.es, Some(1));

        e.on_hook_event(&ev(T0 + 30, "Stop"), T0 + 30);
        let w = write(&mut e, T0 + 30).unwrap();
        assert_eq!((w.payload.e.as_deref(), w.payload.es), (Some("s"), Some(2)));
        assert_eq!(e.poll_due(T0 + 40), Some(PollReason::AfterStop));

        // Old events stop riding along after a minute.
        let w = e.next_write(T0 + 200, HostClock::default()).unwrap();
        assert_eq!((w.payload.e, w.payload.es), (None, None));
    }

    #[test]
    fn reconnect_resends() {
        let mut e = Engine::new(settings(""));
        e.on_poll_results(T0, vec![(PathBuf::from("/h/.claude"), Ok(reading(15)))]);
        write(&mut e, T0).unwrap();
        e.on_connected();
        assert_eq!(write(&mut e, T0 + 1).unwrap().kind, WriteKind::Poll);
    }

    #[test]
    fn settings_change_triggers_poll() {
        let mut e = Engine::new(settings(""));
        e.on_poll_results(T0, vec![(PathBuf::from("/h/.claude"), Ok(reading(15)))]);
        assert_eq!(e.poll_due(T0 + 1), None);
        e.update_settings(settings("config_dirs = ~/.claude, ~/.w\n"));
        assert_eq!(e.poll_due(T0 + 1), Some(PollReason::Requested));
    }

    #[test]
    fn clock_fields_included() {
        let mut e = Engine::new(settings("clock = 12\n"));
        e.on_poll_results(T0, vec![(PathBuf::from("/h/.claude"), Ok(reading(15)))]);
        let w = e.next_write(T0, HostClock { utc_offset_secs: -3600, prefers_12h: false }).unwrap();
        assert_eq!((w.payload.t, w.payload.tf), (Some(T0 - 3600), Some(12)));
    }
}
