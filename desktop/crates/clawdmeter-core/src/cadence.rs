//! When to poll `/usage` (spec §08 "Polling cadence").
//!
//! | Situation                        | /usage                         |
//! |----------------------------------|--------------------------------|
//! | A prompt was sent                | immediately, then every 60 s   |
//! | Just after a Stop                | once, ~10 s later              |
//! | No hook events for 10 min        | every 15 min                   |
//! | Device asks on connect (REQ)     | immediately                    |
//! | No hooks installed at all        | every 2 min                    |
//!
//! Between a Stop and the 10-minute quiet mark the no-hooks cadence (2 min)
//! applies. An explicit `poll_interval` in the config sets the working
//! cadence and replaces the 2-minute one, so a user who tuned the old daemon
//! keeps roughly what they had. A failed poll retries after 30 s at most.

use crate::events::Activity;

/// Cadence tunables, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cadence {
    /// While a session is working.
    pub working: i64,
    /// Delay of the one-shot poll after a Stop.
    pub after_stop: i64,
    /// Quiet time after which the idle cadence applies.
    pub quiet_after: i64,
    /// Idle cadence once hooks have been quiet for `quiet_after`.
    pub quiet: i64,
    /// Cadence when no hooks are installed (and between Stop and quiet).
    pub no_hooks: i64,
    /// Retry delay after a failed poll (capped by the current interval).
    pub retry: i64,
}

impl Default for Cadence {
    fn default() -> Self {
        Cadence { working: 60, after_stop: 10, quiet_after: 600, quiet: 900, no_hooks: 120, retry: 30 }
    }
}

impl Cadence {
    /// Defaults adjusted by the config's `poll_interval`.
    pub fn from_settings(poll_interval: u64, explicit: bool) -> Self {
        let mut c = Cadence::default();
        if explicit {
            let p = poll_interval.min(i64::MAX as u64) as i64;
            c.working = p;
            c.no_hooks = p;
        }
        c
    }
}

/// Why a poll is due (for logs and the tray).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollReason {
    /// First poll after start.
    Startup,
    /// The device asked (REQ notify) or the user hit "Refresh now".
    Requested,
    /// A prompt was just sent.
    Prompt,
    /// The one-shot ~10 s after a Stop.
    AfterStop,
    /// The regular interval elapsed.
    Interval,
    /// Retrying a failed poll.
    Retry,
}

/// Decides when the next `/usage` poll is due. Times are unix seconds.
#[derive(Debug, Clone, Default)]
pub struct Scheduler {
    /// Cadence in use.
    pub cadence: Cadence,
    last_poll: Option<i64>,
    last_ok: bool,
    immediate: Option<PollReason>,
    after_stop_at: Option<i64>,
}

impl Scheduler {
    /// A scheduler that polls on its first check.
    pub fn new(cadence: Cadence) -> Self {
        Scheduler { cadence, immediate: Some(PollReason::Startup), last_ok: true, ..Default::default() }
    }

    /// The device or the user asked for fresh numbers.
    pub fn request(&mut self) {
        self.immediate = Some(PollReason::Requested);
    }

    /// A prompt was sent.
    pub fn on_prompt(&mut self) {
        if self.immediate.is_none() {
            self.immediate = Some(PollReason::Prompt);
        }
        self.after_stop_at = None;
    }

    /// A turn stopped at `now`.
    pub fn on_stop(&mut self, now: i64) {
        self.after_stop_at = Some(now + self.cadence.after_stop);
    }

    /// The regular interval for the current situation.
    pub fn interval(&self, activity: &Activity, hooks_installed: bool, now: i64) -> i64 {
        if !hooks_installed && !activity.hooks_seen {
            return self.cadence.no_hooks;
        }
        if activity.working(now) {
            return self.cadence.working;
        }
        match activity.quiet_for(now) {
            Some(q) if q < self.cadence.quiet_after => self.cadence.no_hooks,
            _ => self.cadence.quiet,
        }
    }

    /// When the next poll is due, and why.
    pub fn next_due(&self, activity: &Activity, hooks_installed: bool, now: i64) -> (i64, PollReason) {
        if let Some(r) = self.immediate {
            return (now, r);
        }
        let Some(last) = self.last_poll else { return (now, PollReason::Startup) };
        let interval = self.interval(activity, hooks_installed, now);
        let (mut due, mut why) = if self.last_ok {
            (last + interval, PollReason::Interval)
        } else {
            (last + interval.min(self.cadence.retry), PollReason::Retry)
        };
        if let Some(t) = self.after_stop_at {
            if t < due {
                due = t;
                why = PollReason::AfterStop;
            }
        }
        (due, why)
    }

    /// `Some(reason)` when a poll should start now.
    pub fn due(&self, activity: &Activity, hooks_installed: bool, now: i64) -> Option<PollReason> {
        let (at, why) = self.next_due(activity, hooks_installed, now);
        (at <= now).then_some(why)
    }

    /// A poll finished at `now`.
    pub fn polled(&mut self, now: i64, ok: bool) {
        self.last_poll = Some(now);
        self.last_ok = ok;
        self.immediate = None;
        if self.after_stop_at.is_some_and(|t| t <= now) {
            self.after_stop_at = None;
        }
    }

    /// Unix seconds of the last poll attempt.
    pub fn last_poll(&self) -> Option<i64> {
        self.last_poll
    }
}

/// Heartbeat: replay the last payload when nothing was written for
/// `heartbeat_interval` seconds (marked `rp=1`).
pub fn heartbeat_due(last_write: Option<i64>, heartbeat_interval: u64, now: i64) -> bool {
    last_write.is_some_and(|t| now - t >= heartbeat_interval.min(i64::MAX as u64) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{HookEvent, HookKind};

    fn ev(ts: i64, kind: &str) -> HookEvent {
        HookEvent { ts, kind: HookKind::parse(kind), session: "s".into() }
    }

    #[test]
    fn startup_and_no_hooks() {
        let a = Activity::default();
        let mut s = Scheduler::new(Cadence::default());
        assert_eq!(s.due(&a, false, 0), Some(PollReason::Startup));
        s.polled(0, true);
        assert_eq!(s.due(&a, false, 119), None);
        assert_eq!(s.due(&a, false, 120), Some(PollReason::Interval));
    }

    #[test]
    fn prompt_then_every_minute_then_stop_once() {
        let mut a = Activity::default();
        let mut s = Scheduler::new(Cadence::default());
        s.polled(0, true);
        a.ingest(&ev(100, "UserPromptSubmit"));
        s.on_prompt();
        assert_eq!(s.due(&a, true, 100), Some(PollReason::Prompt));
        s.polled(100, true);
        a.ingest(&ev(130, "PostToolUse"));
        assert_eq!(s.due(&a, true, 159), None);
        assert_eq!(s.due(&a, true, 160), Some(PollReason::Interval));
        s.polled(160, true);
        a.ingest(&ev(170, "Stop"));
        s.on_stop(170);
        assert_eq!(s.due(&a, true, 179), None);
        assert_eq!(s.due(&a, true, 180), Some(PollReason::AfterStop));
        s.polled(180, true);
        // After the stop, not working: 2-min cadence until 10 min quiet.
        assert_eq!(s.next_due(&a, true, 181).0, 300);
        s.polled(300, true);
        // 10+ minutes since the last event: 15 min cadence.
        assert_eq!(s.next_due(&a, true, 170 + 600).0, 300 + 900);
    }

    #[test]
    fn device_request_is_immediate() {
        let a = Activity::default();
        let mut s = Scheduler::new(Cadence::default());
        s.polled(0, true);
        s.request();
        assert_eq!(s.due(&a, true, 1), Some(PollReason::Requested));
        s.polled(1, true);
        assert_eq!(s.due(&a, true, 2), None);
    }

    #[test]
    fn failed_poll_retries_sooner() {
        let a = Activity::default();
        let mut s = Scheduler::new(Cadence::default());
        s.polled(0, false);
        assert_eq!(s.due(&a, false, 29), None);
        assert_eq!(s.due(&a, false, 30), Some(PollReason::Retry));
    }

    #[test]
    fn explicit_poll_interval() {
        let c = Cadence::from_settings(300, true);
        assert_eq!((c.working, c.no_hooks, c.quiet), (300, 300, 900));
        assert_eq!(Cadence::from_settings(300, false), Cadence::default());
    }

    // Ported from the legacy daemon test test_bash_heartbeat.sh: heartbeat timing.
    #[test]
    fn heartbeat_timing() {
        assert!(!heartbeat_due(None, 60, 1000));
        assert!(!heartbeat_due(Some(1000), 60, 1059));
        assert!(heartbeat_due(Some(1000), 60, 1060));
    }
}
