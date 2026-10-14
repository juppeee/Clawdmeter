//! Usage straight from Claude's usage endpoint, with Claude Code's own
//! sign-in, while no 5-hour window is known to be running.
//!
//! `GET https://api.anthropic.com/api/oauth/usage` with the account's OAuth
//! access token (`Authorization: Bearer …`, `anthropic-beta:
//! oauth-2025-04-20`) answers with the same windows `/usage` prints:
//!
//! ```text
//! {"five_hour": {"utilization": 35.0, "resets_at": "2026-10-14T02:40:00Z"},
//!  "seven_day": {"utilization": 20.0, "resets_at": "…"},
//!  "extra_usage": {…}, "limits": [{"kind": "spend", "group": "monthly",
//!  "percent": 54, "resets_at": "…"}]}
//! ```
//!
//! (`utilization` / `percent` are 0–100.) Sending no message, it never
//! starts a window.
//!
//! The token is only read, never refreshed: Claude Code (or the Claude
//! desktop app) owns it. It lives in `<config dir>/.credentials.json`
//! (`{"claudeAiOauth": {"accessToken", "expiresAt", …}}`) or, on macOS, in
//! the login Keychain under Claude Code's service name ([`keychain_service`]).
//! An expired token is skipped until Claude Code refreshes it. A 429 parks
//! the account for exactly the `retry-after` the server sends.

use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::usage::{Spend, SpendPeriod, UsageError, UsageReading, Window};

/// The usage endpoint.
pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// The beta header OAuth calls need.
pub const OAUTH_BETA: &str = "oauth-2025-04-20";
/// The User-Agent sent with the call.
pub const USER_AGENT: &str = concat!("claude-cli/2.1.296 (external, clawdmeter/", env!("CARGO_PKG_VERSION"), ")");
/// Timeout for the call.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// One account's OAuth access token, as Claude Code stored it.
#[derive(Clone, PartialEq, Eq)]
pub struct Token {
    /// The bearer token.
    pub access: String,
    /// When it expires (unix milliseconds), when stored.
    pub expires_at_ms: Option<i64>,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Token").field("access", &"<redacted>").field("expires_at_ms", &self.expires_at_ms).finish()
    }
}

impl Token {
    /// Expired at `now` (unix seconds), with a minute's slack.
    pub fn expired(&self, now: i64) -> bool {
        self.expires_at_ms.is_some_and(|ms| ms / 1000 <= now + 60)
    }
}

/// Pull the access token out of a credentials blob: `{"claudeAiOauth":
/// {"accessToken", "expiresAt"}}`, or the token object itself.
pub fn parse_credentials(text: &str) -> Option<Token> {
    let v: Value = serde_json::from_str(text.trim()).ok()?;
    let o = v.get("claudeAiOauth").unwrap_or(&v);
    let access = o.get("accessToken")?.as_str().filter(|s| !s.is_empty())?.to_owned();
    Some(Token { access, expires_at_ms: o.get("expiresAt").and_then(Value::as_i64) })
}

/// Claude Code's Keychain service name for a config dir (from its
/// `secureStorage`): `Claude Code-credentials` for the default `~/.claude`,
/// run without `CLAUDE_CONFIG_DIR`; otherwise suffixed with `-` and the first
/// 8 hex digits of the SHA-256 of the dir (NFC), as Clawdmeter passes it in
/// `CLAUDE_CONFIG_DIR`.
pub fn keychain_service(config_dir: &Path, home: &Path) -> String {
    if config_dir == home.join(".claude") {
        return "Claude Code-credentials".into();
    }
    let dir: String = config_dir.to_string_lossy().nfc().collect();
    let hash = Sha256::digest(dir.as_bytes());
    let hex: String = hash.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("Claude Code-credentials-{hex}")
}

/// `security -w` prints a secret holding non-printable bytes as one hex
/// string; a JSON blob never is valid hex.
fn decode_keychain(raw: &str) -> String {
    let t = raw.trim();
    if !t.is_empty() && t.len().is_multiple_of(2) && t.bytes().all(|b| b.is_ascii_hexdigit()) {
        let bytes: Option<Vec<u8>> =
            (0..t.len()).step_by(2).map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok()).collect();
        if let Some(s) = bytes.and_then(|b| String::from_utf8(b).ok()) {
            return s;
        }
    }
    t.to_owned()
}

/// Read one account's token (never refreshing it): its credentials file,
/// else (macOS) the Keychain. `None` when there is none to read.
pub fn read_token(config_dir: &Path, home: &Path) -> Option<Token> {
    if let Ok(text) = std::fs::read_to_string(config_dir.join(".credentials.json")) {
        if let Some(t) = parse_credentials(&text) {
            return Some(t);
        }
    }
    if !cfg!(target_os = "macos") {
        return None;
    }
    let user = std::env::var("USER").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| "claude-code-user".into());
    let out = std::process::Command::new("security")
        .args(["find-generic-password", "-a", &user, "-w", "-s", &keychain_service(config_dir, home)])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_credentials(&decode_keychain(&String::from_utf8_lossy(&out.stdout)))
}

fn window(v: Option<&Value>, pct_key: &str) -> Option<Window> {
    let v = v?;
    let pct = v.get(pct_key)?.as_f64()?;
    let resets_at = v
        .get("resets_at")
        .and_then(Value::as_str)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc));
    Some(Window { pct: pct.round().max(0.0) as u32, resets_at })
}

/// Read a 200 body. A five-hour window whose reset has passed (or with no
/// reset) means the window isn't running.
pub fn parse_usage_json(body: &str, now: DateTime<Utc>) -> Result<UsageReading, UsageError> {
    let v: Value = serde_json::from_str(body).map_err(|e| UsageError::BadOutput(format!("usage endpoint: {e}")))?;
    let session = window(v.get("five_hour"), "utilization").filter(|w| w.resets_at.is_some_and(|t| t > now));
    let week = window(v.get("seven_day"), "utilization");
    let models = v
        .get("model_scoped")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| Some((m.get("display_name")?.as_str()?.to_owned(), window(Some(m), "utilization")?)))
        .collect();
    let spend = v
        .get("limits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|l| l.get("kind").and_then(Value::as_str) == Some("spend"))
        .and_then(|l| {
            let w = window(Some(l), "percent")?;
            let period = match l.get("group").and_then(Value::as_str) {
                Some("daily") => Some(SpendPeriod::Daily),
                Some("weekly") => Some(SpendPeriod::Weekly),
                Some("monthly") => Some(SpendPeriod::Monthly),
                _ => None,
            };
            Some(Spend { pct: w.pct, resets_at: w.resets_at, period })
        });
    let overage = v.get("extra_usage").and_then(|x| x.get("is_enabled")).and_then(Value::as_bool).unwrap_or(false)
        && v.get("extra_usage").and_then(|x| x.get("used_credits")).and_then(Value::as_f64).is_some_and(|c| c > 0.0);
    let has_any = v.get("five_hour").is_some_and(|w| !w.is_null()) || week.is_some() || spend.is_some();
    if !has_any {
        return Err(UsageError::NoUsageLines("usage endpoint: no windows".into()));
    }
    Ok(UsageReading { session, week, models, spend, overage })
}

/// What one endpoint answer means.
pub fn interpret(
    status: u16,
    retry_after: Option<&str>,
    body: &str,
    now: DateTime<Utc>,
) -> Result<UsageReading, UsageError> {
    match status {
        200..=299 => parse_usage_json(body, now),
        401 | 403 => Err(UsageError::TokenRejected),
        429 => Err(UsageError::Throttled {
            retry_after_secs: retry_after.and_then(|r| r.trim().parse::<i64>().ok()).filter(|s| *s > 0),
        }),
        s => Err(UsageError::BadOutput(format!("usage endpoint answered HTTP {s}"))),
    }
}

/// Call the endpoint with `token` (blocking).
pub fn fetch_usage(token: &Token, now: DateTime<Utc>) -> Result<UsageReading, UsageError> {
    let agent: ureq::Agent =
        ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(HTTP_TIMEOUT)).build().into();
    let mut resp = agent
        .get(USAGE_URL)
        .header("Authorization", &format!("Bearer {}", token.access))
        .header("anthropic-beta", OAUTH_BETA)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .call()
        .map_err(|e| UsageError::Offline(e.to_string()))?;
    let status = resp.status().as_u16();
    let retry_after = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).map(str::to_owned);
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    interpret(status, retry_after.as_deref(), &body, now)
}

/// Read one account from the endpoint. `Ok(None)` when it has no readable
/// token (the caller falls back to `claude -p /usage`).
pub fn poll_endpoint(config_dir: &Path, home: &Path, now: DateTime<Utc>) -> Option<Result<UsageReading, UsageError>> {
    let token = read_token(config_dir, home)?;
    if token.expired(now.timestamp()) {
        return Some(Err(UsageError::TokenExpired));
    }
    Some(fetch_usage(&token, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn credentials_blobs() {
        let nested = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-x","refreshToken":"r","expiresAt":1791950000000,"scopes":["user:inference"]}}"#;
        let t = parse_credentials(nested).unwrap();
        assert_eq!((t.access.as_str(), t.expires_at_ms), ("sk-ant-oat01-x", Some(1_791_950_000_000)));
        assert!(!t.expired(1_791_949_000));
        assert!(t.expired(1_791_949_950), "a minute's slack");
        assert_eq!(parse_credentials(r#"{"accessToken":"d"}"#).unwrap().expires_at_ms, None);
        assert_eq!(parse_credentials(r#"{"claudeAiOauth":{"accessToken":""}}"#), None);
        assert_eq!(parse_credentials("not json"), None);
        assert!(!format!("{t:?}").contains("sk-ant"), "never logged");
    }

    #[test]
    fn keychain_names_match_claude_code() {
        let home = Path::new("/Users/me");
        assert_eq!(keychain_service(Path::new("/Users/me/.claude"), home), "Claude Code-credentials");
        // sha256("/Users/me/.claude-work") = 5a2b…; first 8 hex digits.
        let want = {
            let h = Sha256::digest(b"/Users/me/.claude-work");
            format!("Claude Code-credentials-{:02x}{:02x}{:02x}{:02x}", h[0], h[1], h[2], h[3])
        };
        assert_eq!(keychain_service(Path::new("/Users/me/.claude-work"), home), want);
        assert_eq!(want.len(), "Claude Code-credentials-".len() + 8);
        assert_eq!(decode_keychain("7b2261223a317d"), r#"{"a":1}"#);
        assert_eq!(decode_keychain(r#" {"a":1} "#), r#"{"a":1}"#);
    }

    #[test]
    fn subscription_body() {
        let now = at("2026-10-14T01:20:00Z");
        let body = r#"{"five_hour":{"utilization":35.0,"resets_at":"2026-10-14T02:40:00.123+00:00"},
            "seven_day":{"utilization":20.4,"resets_at":"2026-10-19T00:00:00Z"},
            "seven_day_opus":null,"model_scoped":[{"display_name":"Fable","utilization":3,"resets_at":"2026-10-19T00:00:00Z"}],
            "extra_usage":{"is_enabled":false,"monthly_limit":null,"used_credits":null,"utilization":null}}"#;
        let r = parse_usage_json(body, now).unwrap();
        assert_eq!(r.session.as_ref().map(|w| w.pct), Some(35));
        assert!(r.window_started());
        assert_eq!(r.week, Some(Window { pct: 20, resets_at: Some(at("2026-10-19T00:00:00Z")) }));
        assert_eq!(r.models[0].0, "Fable");
        assert!(!r.overage && r.spend.is_none());
    }

    #[test]
    fn no_running_window() {
        let now = at("2026-10-14T05:00:00Z");
        let passed = r#"{"five_hour":{"utilization":35.0,"resets_at":"2026-10-14T02:40:00Z"},"seven_day":{"utilization":20,"resets_at":"2026-10-19T00:00:00Z"}}"#;
        assert_eq!(parse_usage_json(passed, now).unwrap().session, None);
        let none = r#"{"five_hour":{"utilization":0,"resets_at":null},"seven_day":{"utilization":20,"resets_at":"2026-10-19T00:00:00Z"}}"#;
        let r = parse_usage_json(none, now).unwrap();
        assert!(!r.window_started());
        assert_eq!(r.session_pct(), 0);
    }

    #[test]
    fn spend_capped_body() {
        let body = r#"{"extra_usage":{"is_enabled":true,"monthly_limit":50000,"used_credits":27140,"utilization":54,"currency":"USD"},
            "limits":[{"kind":"spend","group":"monthly","percent":54,"resets_at":"2026-11-01T00:00:00.000Z","severity":"normal","is_active":true}]}"#;
        let r = parse_usage_json(body, at("2026-10-14T01:20:00Z")).unwrap();
        assert!(r.is_spend_only());
        assert_eq!(r.spend.as_ref().map(|s| (s.pct, s.period)), Some((54, Some(SpendPeriod::Monthly))));
        assert_eq!(r.session_pct(), 54);
    }

    #[test]
    fn a_429_parks_for_exactly_its_retry_after() {
        let mut st = crate::usage::AccountState::default();
        st.record(1000, Err(UsageError::Throttled { retry_after_secs: Some(1353) }));
        assert_eq!(st.backoff.until(), Some(2353));
        // Without a retry-after: the usual 10-minute first step.
        let mut st = crate::usage::AccountState::default();
        st.record(1000, Err(UsageError::Throttled { retry_after_secs: None }));
        assert_eq!(st.backoff.until(), Some(1600));
        // An expired token waits for Claude Code; no backoff, not "logged out".
        let mut st = crate::usage::AccountState::default();
        st.record(1000, Err(UsageError::TokenExpired));
        assert_eq!(st.backoff.until(), None);
        assert!(!UsageError::TokenExpired.is_auth());
    }

    #[test]
    fn status_codes() {
        let now = at("2026-10-14T01:20:00Z");
        assert_eq!(interpret(429, Some("1353"), "", now), Err(UsageError::Throttled { retry_after_secs: Some(1353) }));
        assert_eq!(interpret(429, None, "", now), Err(UsageError::Throttled { retry_after_secs: None }));
        assert_eq!(interpret(429, Some("soon"), "", now), Err(UsageError::Throttled { retry_after_secs: None }));
        assert_eq!(interpret(401, None, "", now), Err(UsageError::TokenRejected));
        assert!(matches!(interpret(500, None, "", now), Err(UsageError::BadOutput(_))));
        assert!(matches!(interpret(200, None, "{}", now), Err(UsageError::NoUsageLines(_))));
        assert!(matches!(interpret(200, None, "<html>", now), Err(UsageError::BadOutput(_))));
    }
}
