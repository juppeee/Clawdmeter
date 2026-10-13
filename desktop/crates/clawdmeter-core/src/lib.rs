//! Pure logic for the Clawdmeter desktop daemon.
//!
//! Nothing in here talks to Bluetooth or draws a window. The modules:
//!
//! * [`paths`], [`config`]: the shared `~/.config/claude-usage-monitor` files.
//! * [`usage`]: run and parse `claude --output-format json -p /usage`.
//! * [`events`]: the hook events log (fast clock) and the activity tracker.
//! * [`attention`]: the attention hook's alert / clear files (written by
//!   `clawdmeter hook`; the installer lives in the app crate).
//! * [`cadence`]: when to poll, when to heartbeat.
//! * [`selector`]: which account to show.
//! * [`payload`]: the JSON the firmware parses.
//! * [`rate`]: the slow-clock mood.
//! * [`engine`]: all of the above as one I/O-free state machine.
//! * [`autostart`], [`platform`]: start-at-login and host queries.

#![warn(missing_docs)]

pub mod attention;
pub mod autostart;
pub mod cadence;
pub mod config;
pub mod engine;
pub mod events;
pub mod paths;
pub mod payload;
pub mod platform;
pub mod rate;
pub mod selector;
pub mod usage;

pub use config::{ConfigFile, Settings};
pub use engine::{Engine, EngineStatus, HostClock, Write, WriteKind};
pub use paths::Paths;
pub use payload::Payload;
pub use usage::{UsageError, UsageReading, Window};

/// The device's advertised name.
pub const DEVICE_NAME: &str = "Clawdmeter";

/// Unix seconds now.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}
