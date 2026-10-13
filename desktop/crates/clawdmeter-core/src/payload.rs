//! The JSON written to the device's RX characteristic.
//!
//! Keys match `parse_json()` in `firmware/src/main.cpp`, in the order the old
//! daemons wrote them, plus the additive fast-clock fields `a`, `e`, `es`,
//! `n`, `rp` (old firmware ignores unknown keys).

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::config::ClockMode;
use crate::usage::UsageReading;

/// One device payload. `None` fields are omitted.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Payload {
    /// Session % used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s: Option<u32>,
    /// Minutes until the session resets; -1 = window not started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sr: Option<i64>,
    /// Weekly % used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<u32>,
    /// Minutes until the weekly reset; -1 = unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wr: Option<i64>,
    /// Status string ("allowed" / "limited").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub st: Option<String>,
    /// Account kind: "pro" (5h/7d windows) or "ent" (spending limit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acct: Option<String>,
    /// Enterprise: % of the billing period elapsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tp: Option<u32>,
    /// Enterprise: billing period length in days.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pd: Option<u32>,
    /// Enterprise: reset date text, e.g. "Jul 1".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rd: Option<String>,
    /// Local wall-clock epoch (UTC epoch shifted by the local offset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<i64>,
    /// Hour format, 12 or 24.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tf: Option<u8>,
    /// 1 = the session-reset chime may sound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<u8>,
    /// Data is valid. `false` alone means "no data" (e.g. logged out).
    pub ok: bool,
    /// 1 = beep for this alert.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<u8>,
    /// Attention message (ASCII).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m: Option<String>,
    /// 1 = end the alert.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<u8>,
    /// Activity level 0-3 (PostToolUse per minute).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<u8>,
    /// Latest event: "p" prompt, "s" stop, "b" back after a gap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub e: Option<String>,
    /// Sequence number of `e`, so the device reacts once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub es: Option<u32>,
    /// Sessions with a hook event in the last 5 minutes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// 1 = heartbeat/replay (no fresh poll): the rate math ignores it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rp: Option<u8>,
}

/// Every key [`Payload`] can emit, in serialization order.
pub const PAYLOAD_KEYS: &[&str] = &[
    "s", "sr", "w", "wr", "st", "acct", "tp", "pd", "rd", "t", "tf", "c", "ok", "b", "m", "x", "a", "e", "es", "n",
    "rp",
];
/// The fast-clock keys added in v0.4 (old firmware ignores them).
pub const ADDITIVE_KEYS: &[&str] = &["a", "e", "es", "n", "rp"];

fn minutes_until(at: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    // Round like the bash daemon's sprintf("%.0f"), clamp at 0.
    let secs = (at - now).num_seconds();
    if secs <= 0 {
        0
    } else {
        (secs + 30) / 60
    }
}

/// Wall-clock fields for the `clock` setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockFields {
    /// Local epoch (`t`).
    pub local_epoch: i64,
    /// 12 or 24 (`tf`).
    pub hour_format: u8,
}

/// Compute `t`/`tf` for `mode` (None when the clock is off).
pub fn clock_fields(
    mode: ClockMode,
    now: DateTime<Utc>,
    utc_offset_secs: i64,
    locale_12h: bool,
) -> Option<ClockFields> {
    let hour_format = match mode {
        ClockMode::Off => return None,
        ClockMode::H12 => 12,
        ClockMode::H24 => 24,
        ClockMode::Auto => {
            if locale_12h {
                12
            } else {
                24
            }
        }
    };
    Some(ClockFields { local_epoch: now.timestamp() + utc_offset_secs, hour_format })
}

impl Payload {
    /// The usage part for a reading, as of `now`. Reset countdowns are
    /// recomputed from the absolute reset time, so a replay is exact.
    pub fn from_reading(r: &UsageReading, now: DateTime<Utc>) -> Self {
        let (s, sr) = match &r.session {
            Some(w) => (w.pct, w.resets_at.map_or(-1, |t| minutes_until(t, now))),
            None => (0, -1),
        };
        let (w, wr) = match &r.week {
            Some(win) => (win.pct, win.resets_at.map_or(-1, |t| minutes_until(t, now))),
            None => (0, -1),
        };
        Payload {
            s: Some(s),
            sr: Some(sr),
            w: Some(w),
            wr: Some(wr),
            st: Some(if s >= 100 { "limited" } else { "allowed" }.to_owned()),
            acct: Some("pro".to_owned()),
            ok: true,
            ..Default::default()
        }
    }

    /// The "no data" payload (every account logged out).
    pub fn no_data() -> Self {
        Payload { ok: false, ..Default::default() }
    }

    /// Age a payload by `secs` like the bash daemon's `age_payload`: positive
    /// `sr`/`wr` tick down by whole minutes (clamped at 0) and `t` advances.
    pub fn aged(&self, secs: i64) -> Self {
        let mins = secs / 60;
        let mut p = self.clone();
        for x in [&mut p.sr, &mut p.wr].into_iter().flatten() {
            if *x > 0 {
                *x = (*x - mins).max(0);
            }
        }
        if let Some(t) = &mut p.t {
            if *t > 0 {
                *t += secs;
            }
        }
        p
    }

    /// Compact JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("payload serializes")
    }

    /// JSON for one write of at most `limit` bytes: shorten the alert message,
    /// then drop it, until it fits (Python daemon's `encode_payload`).
    pub fn encode(&self, limit: usize) -> Vec<u8> {
        let mut data = self.to_json().into_bytes();
        let mut p = self.clone();
        let mut msg = p.m.clone().unwrap_or_default();
        while data.len() > limit && !msg.is_empty() {
            let over = data.len() - limit;
            let keep = msg.len().saturating_sub(over + 3);
            msg.truncate(keep);
            let trimmed = msg.trim_end().to_owned();
            msg = trimmed;
            p.m = Some(if msg.is_empty() { String::new() } else { format!("{msg}...") });
            data = p.to_json().into_bytes();
        }
        if data.len() > limit {
            p.m = None;
            data = p.to_json().into_bytes();
        }
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::Window;
    use std::collections::BTreeSet;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn p(json: &str) -> Payload {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let g = |k: &str| v.get(k);
        Payload {
            s: g("s").and_then(|x| x.as_u64()).map(|x| x as u32),
            sr: g("sr").and_then(|x| x.as_i64()),
            w: g("w").and_then(|x| x.as_u64()).map(|x| x as u32),
            wr: g("wr").and_then(|x| x.as_i64()),
            st: g("st").and_then(|x| x.as_str()).map(Into::into),
            acct: g("acct").and_then(|x| x.as_str()).map(Into::into),
            tp: g("tp").and_then(|x| x.as_u64()).map(|x| x as u32),
            pd: g("pd").and_then(|x| x.as_u64()).map(|x| x as u32),
            rd: g("rd").and_then(|x| x.as_str()).map(Into::into),
            t: g("t").and_then(|x| x.as_i64()),
            tf: g("tf").and_then(|x| x.as_u64()).map(|x| x as u8),
            c: g("c").and_then(|x| x.as_u64()).map(|x| x as u8),
            ok: g("ok").and_then(|x| x.as_bool()).unwrap_or(false),
            ..Default::default()
        }
    }

    // Ported from daemon/tests/test_bash_heartbeat.sh (payload aging).
    #[test]
    fn aging_like_bash() {
        assert_eq!(
            p(r#"{"s":13,"sr":73,"w":21,"wr":6913,"st":"allowed","acct":"pro","ok":true}"#).aged(300).to_json(),
            r#"{"s":13,"sr":68,"w":21,"wr":6908,"st":"allowed","acct":"pro","ok":true}"#
        );
        assert_eq!(
            p(r#"{"s":13,"sr":73,"w":21,"wr":6913,"st":"allowed","ok":true}"#).aged(59).to_json(),
            r#"{"s":13,"sr":73,"w":21,"wr":6913,"st":"allowed","ok":true}"#
        );
        assert_eq!(
            p(r#"{"s":13,"sr":2,"w":21,"wr":0,"st":"allowed","c":1,"t":1758300000,"tf":12,"ok":true}"#)
                .aged(245)
                .to_json(),
            r#"{"s":13,"sr":0,"w":21,"wr":0,"st":"allowed","t":1758300245,"tf":12,"c":1,"ok":true}"#
        );
        assert_eq!(
            p(r#"{"s":99,"sr":61,"w":0,"wr":0,"st":"allowed","acct":"ent","tp":40,"pd":30,"rd":"Oct 1","ok":true}"#)
                .aged(60)
                .to_json(),
            r#"{"s":99,"sr":60,"w":0,"wr":0,"st":"allowed","acct":"ent","tp":40,"pd":30,"rd":"Oct 1","ok":true}"#
        );
        // Not started (-1) is never aged.
        assert_eq!(p(r#"{"sr":-1,"ok":true}"#).aged(600).sr, Some(-1));
    }

    #[test]
    fn from_reading() {
        let now = at("2026-10-13T05:40:00Z");
        let r = UsageReading {
            session: Some(Window { pct: 15, resets_at: Some(at("2026-10-13T07:00:00Z")) }),
            week: Some(Window { pct: 6, resets_at: Some(at("2026-10-18T22:00:00Z")) }),
            models: vec![],
        };
        assert_eq!(
            Payload::from_reading(&r, now).to_json(),
            r#"{"s":15,"sr":80,"w":6,"wr":8180,"st":"allowed","acct":"pro","ok":true}"#
        );
        let not_started = UsageReading { session: None, ..r.clone() };
        let pl = Payload::from_reading(&not_started, now);
        assert_eq!((pl.s, pl.sr), (Some(0), Some(-1)));
        // Rounds to the nearest minute, clamps at 0.
        let soon = UsageReading {
            session: Some(Window { pct: 100, resets_at: Some(at("2026-10-13T05:40:31Z")) }),
            ..Default::default()
        };
        let pl = Payload::from_reading(&soon, now);
        assert_eq!((pl.sr, pl.st.as_deref()), (Some(1), Some("limited")));
        assert_eq!(Payload::from_reading(&soon, at("2026-10-13T06:00:00Z")).sr, Some(0));
        assert_eq!(Payload::no_data().to_json(), r#"{"ok":false}"#);
    }

    #[test]
    fn clock() {
        let now = at("2026-10-13T12:00:00Z");
        assert_eq!(clock_fields(ClockMode::Off, now, 3600, true), None);
        let f = clock_fields(ClockMode::Auto, now, 3600, true).unwrap();
        assert_eq!((f.local_epoch, f.hour_format), (now.timestamp() + 3600, 12));
        assert_eq!(clock_fields(ClockMode::H24, now, 0, true).unwrap().hour_format, 24);
    }

    // Ported from daemon/tests/test_attention.py::test_encode_payload_fits.
    #[test]
    fn encode_fits() {
        let base = Payload { s: Some(4), ok: true, b: Some(1), ..Default::default() };
        let short = Payload { m: Some("hi".into()), ..base.clone() }.encode(180);
        let v: serde_json::Value = serde_json::from_slice(&short).unwrap();
        assert_eq!(v["m"], "hi");
        let long = Payload { m: Some("x".repeat(200)), ..base.clone() }.encode(120);
        assert!(long.len() <= 120);
        let v: serde_json::Value = serde_json::from_slice(&long).unwrap();
        assert!(v["m"].as_str().unwrap().ends_with("..."));
        let tiny = Payload { m: Some("x".repeat(200)), ..base }.encode(26);
        let v: serde_json::Value = serde_json::from_slice(&tiny).unwrap();
        assert!(v.get("m").is_none());
    }

    #[test]
    fn full_payload_key_order() {
        let pl = Payload {
            s: Some(1),
            sr: Some(2),
            w: Some(3),
            wr: Some(4),
            st: Some("allowed".into()),
            acct: Some("pro".into()),
            tp: Some(1),
            pd: Some(30),
            rd: Some("Oct 1".into()),
            t: Some(5),
            tf: Some(24),
            c: Some(1),
            ok: true,
            b: Some(1),
            m: Some("hi".into()),
            x: Some(1),
            a: Some(2),
            e: Some("p".into()),
            es: Some(7),
            n: Some(1),
            rp: Some(1),
        };
        let v: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&pl.to_json()).unwrap();
        // serde_json's Map is sorted without preserve_order; check membership.
        let got: BTreeSet<&str> = v.keys().map(String::as_str).collect();
        let want: BTreeSet<&str> = PAYLOAD_KEYS.iter().copied().collect();
        assert_eq!(got, want);
        let json = pl.to_json();
        let positions: Vec<usize> = PAYLOAD_KEYS.iter().map(|k| json.find(&format!("\"{k}\":")).unwrap()).collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "keys serialize in PAYLOAD_KEYS order");
    }

    /// Every key the firmware's JSON parser reads is one we can send, and
    /// every key we send is either read by the firmware or one of the additive
    /// fast-clock keys. Scans every shared `firmware/src/*.cpp` (the parser
    /// lives in `payload.cpp`; older trees had it in `main.cpp`).
    #[test]
    fn keys_match_firmware_parser() {
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../firmware/src");
        let Ok(rd) = std::fs::read_dir(&src_dir) else {
            eprintln!("firmware source not found at {}; skipping", src_dir.display());
            return;
        };
        let mut fw = BTreeSet::new();
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "cpp") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap_or_default();
            for part in src.split("doc[\"").skip(1) {
                if let Some(end) = part.find('"') {
                    fw.insert(part[..end].to_owned());
                }
            }
        }
        assert!(fw.contains("s") && fw.contains("ok"), "parser keys found: {fw:?}");
        let ours: BTreeSet<String> = PAYLOAD_KEYS.iter().map(|s| s.to_string()).collect();
        for k in &fw {
            assert!(ours.contains(k), "firmware reads {k:?}, which the payload never sends");
        }
        for k in &ours {
            assert!(
                fw.contains(k) || ADDITIVE_KEYS.contains(&k.as_str()),
                "payload sends {k:?}, which the firmware doesn't read"
            );
        }
    }
}
