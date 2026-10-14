//! The desktop chime: the device's ding (firmware `ding.h`), synthesized and
//! played through the default audio output. An ding: one sine
//! strike at C#5 with an instant attack and an exponential decay. Keep the
//! constants in step with the firmware.

use std::num::NonZero;
use std::time::Duration;

/// C#5.
pub const FREQ_HZ: f32 = 554.37;
/// The 3rd partial's share of the strike.
pub const PARTIAL3: f32 = 0.02;
/// Ramp up, so the strike doesn't click (s).
pub const ATTACK_S: f32 = 0.003;
/// Decay time constant (s).
pub const TAU_S: f32 = 0.24;
/// Length; the decay is about -50 dB by then (s).
pub const LENGTH_S: f32 = 1.4;
/// Fade to zero before the cut (s).
pub const FADE_S: f32 = 0.02;
/// Playback rate.
pub const RATE: u32 = 44_100;
/// Peak level on this computer (of full scale).
pub const GAIN: f32 = 0.5;

/// Frames in one ding at `rate` Hz.
pub fn frames(rate: u32) -> u32 {
    (LENGTH_S * rate as f32) as u32
}

/// Frame `n` of the ding at `rate` Hz, in -1..1 (0 past the end).
pub fn value(n: u32, rate: u32) -> f32 {
    if rate == 0 || n >= frames(rate) {
        return 0.0;
    }
    let t = n as f32 / rate as f32;
    let mut env = (-t / TAU_S).exp();
    if t < ATTACK_S {
        env *= t / ATTACK_S;
    }
    let left = LENGTH_S - t;
    if left < FADE_S {
        env *= left / FADE_S;
    }
    let ph = std::f32::consts::TAU * FREQ_HZ * t;
    env * (ph.sin() + PARTIAL3 * (3.0 * ph).sin()) / (1.0 + PARTIAL3)
}

/// One ding as a mono rodio source.
#[derive(Debug, Clone)]
pub struct Ding {
    n: u32,
    rate: u32,
    gain: f32,
}

impl Ding {
    /// A ding at `rate` Hz, peaking at `gain`.
    pub fn new(rate: u32, gain: f32) -> Self {
        Ding { n: 0, rate: rate.max(1), gain: gain.clamp(0.0, 1.0) }
    }
}

impl Iterator for Ding {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.n >= frames(self.rate) {
            return None;
        }
        let v = value(self.n, self.rate) * self.gain;
        self.n += 1;
        Some(v as rodio::Sample)
    }
}

impl rodio::Source for Ding {
    fn current_span_len(&self) -> Option<usize> {
        Some(frames(self.rate).saturating_sub(self.n) as usize)
    }

    fn channels(&self) -> rodio::ChannelCount {
        NonZero::<u16>::MIN
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(self.rate).unwrap_or(NonZero::<u32>::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(LENGTH_S))
    }
}

/// Play one ding on its own thread. No audio output: logged, nothing else.
pub fn play() {
    let spawned = std::thread::Builder::new().name("chime".into()).spawn(|| {
        match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink) => {
                sink.log_on_drop(false);
                let player = rodio::Player::connect_new(sink.mixer());
                player.append(Ding::new(RATE, GAIN));
                player.sleep_until_end();
            }
            Err(e) => log::warn!("chime: no audio output ({e})"),
        }
    });
    if let Err(e) = spawned {
        log::warn!("chime: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;

    fn peak(a: u32, b: u32) -> f32 {
        (a..b).map(|n| value(n, RATE).abs()).fold(0.0, f32::max)
    }

    #[test]
    fn the_ding_matches_the_device() {
        assert_eq!(frames(RATE), 61_740);
        assert_eq!(value(0, RATE), 0.0);
        assert!(peak(0, RATE / 1000) < 0.4, "no click");
        let p = peak(RATE * 3 / 1000, RATE * 6 / 1000);
        assert!(p > 0.9 && p <= 1.0, "{p}");
        let period = (RATE as f32 / FREQ_HZ) as u32 + 1;
        let (t, tau) = (RATE / 10, (TAU_S * RATE as f32) as u32);
        let ratio = peak(t + tau, t + tau + period) / peak(t, t + period);
        assert!((ratio - (-1.0f32).exp()).abs() < 0.03, "{ratio}");
        let rising = (1..RATE / 2).filter(|&n| value(n - 1, RATE) < 0.0 && value(n, RATE) >= 0.0).count();
        assert!((275..=279).contains(&rising), "{rising}");
        assert!(value(frames(RATE) - 1, RATE).abs() < 0.0005);
        assert_eq!(value(frames(RATE), RATE), 0.0);
    }

    #[test]
    fn the_source_is_one_mono_ding() {
        let d = Ding::new(RATE, 0.5);
        assert_eq!((d.channels().get(), d.sample_rate().get()), (1, RATE));
        assert_eq!(d.total_duration(), Some(Duration::from_secs_f32(LENGTH_S)));
        let samples: Vec<rodio::Sample> = d.collect();
        assert_eq!(samples.len(), frames(RATE) as usize);
        assert!(samples.iter().all(|s| s.abs() <= 0.5));
        assert!(Ding::new(RATE, 3.0).all(|s| s.abs() <= 1.0), "gain clamps");
    }
}
