//! Talking to a board over USB: find ports, ask running firmware `info`,
//! probe the chip in ROM download mode, and write a merged image at 0x0.
//!
//! Every supported board enumerates as Espressif's native USB-Serial-JTAG
//! (`303A:1001`), which tells us only "an ESP32 is here". Detection is three
//! probes, cheapest first: the firmware's `info` serial command, then chip +
//! flash size from the ROM, then the user picks among what's left.

use std::io::{Read, Write};
use std::ops::Range;
use std::time::{Duration, Instant};

use espflash::connection::{Connection, ResetAfterOperation, ResetBeforeOperation};
use espflash::flasher::{FlashSize, Flasher};
use espflash::image_format::Segment;
use espflash::target::{Chip, ProgressCallbacks};
use serde::Deserialize;
use serialport::{SerialPortType, UsbPortInfo};

use crate::manifest::{BoardImage, Manifest};

/// Espressif USB vendor id.
pub const ESPRESSIF_VID: u16 = 0x303A;
/// Native USB-Serial-JTAG product id.
pub const USB_JTAG_PID: u16 = 0x1001;
/// The 2.06 is the only 32 MB S3 board.
pub const THIRTY_TWO_MB_S3_ENV: &str = "waveshare_amoled_206";

/// Device-side errors. `Display` is written for people: what happened and
/// what to do about it.
#[derive(Debug)]
pub enum DeviceError {
    /// The OS refused access to the port.
    PermissionDenied {
        /// The port.
        port: String,
    },
    /// Another program has the port open.
    Busy {
        /// The port.
        port: String,
    },
    /// The board went away (unplugged, or reset onto another port).
    Disconnected,
    /// The board didn't answer the ROM bootloader handshake.
    NoBootloader(String),
    /// The board's chip doesn't match the image.
    WrongChip {
        /// What the board has, e.g. `esp32c6`.
        found: String,
        /// What the image is built for.
        expected: String,
    },
    /// Some other serial failure.
    Serial(String),
    /// espflash failed while writing.
    Flash(String),
    /// The port isn't a USB port we can describe to espflash.
    NotUsb(String),
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceError::PermissionDenied { port } if cfg!(target_os = "linux") => write!(
                f,
                "No permission to open {port}. Add yourself to the dialout group (uucp on Arch): \
                 sudo usermod -aG dialout $USER, then log out and back in."
            ),
            DeviceError::PermissionDenied { port } => write!(f, "No permission to open {port}."),
            DeviceError::Busy { port } => {
                write!(f, "{port} is in use by another program, like a serial monitor. Close it and try again.")
            }
            DeviceError::Disconnected => write!(f, "The board disconnected. Plug it back in and try again."),
            DeviceError::NoBootloader(_) => {
                write!(f, "The board didn't respond. Unplug it, hold BOOT while you plug it back in, then try again.")
            }
            DeviceError::WrongChip { found, expected } => write!(
                f,
                "This board has an {}, but that firmware is for an {}. Go back and pick your board again.",
                pretty_chip(found),
                pretty_chip(expected)
            ),
            DeviceError::Serial(e) => write!(f, "Couldn't talk to the board ({e})."),
            DeviceError::Flash(e) => write!(f, "Flashing stopped ({e}). Unplug the board, plug it back in and retry."),
            DeviceError::NotUsb(p) => write!(f, "{p} isn't a USB serial port."),
        }
    }
}

/// `esp32s3` → `ESP32-S3`.
pub fn pretty_chip(id: &str) -> String {
    let up = id.to_ascii_uppercase();
    match up.strip_prefix("ESP32") {
        Some(rest) if !rest.is_empty() && !rest.starts_with('-') => format!("ESP32-{rest}"),
        _ => up,
    }
}

impl DeviceError {
    /// Classify a failure to open `port`.
    pub fn from_open(port: &str, e: serialport::Error) -> Self {
        use serialport::ErrorKind as K;
        match e.kind() {
            K::Io(std::io::ErrorKind::PermissionDenied) => DeviceError::PermissionDenied { port: port.to_owned() },
            K::Io(std::io::ErrorKind::NotFound) => DeviceError::Disconnected,
            // EBUSY / TIOCEXCL on POSIX, ACCESS_DENIED on Windows (another
            // handle holds it), or a vanished device.
            K::NoDevice if list_ports().iter().any(|p| p.path == port) => DeviceError::Busy { port: port.to_owned() },
            K::NoDevice => DeviceError::Disconnected,
            _ => DeviceError::Serial(e.to_string()),
        }
    }

    fn from_io(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::NotFound | std::io::ErrorKind::NotConnected => {
                DeviceError::Disconnected
            }
            _ => DeviceError::Serial(e.to_string()),
        }
    }
}

impl std::error::Error for DeviceError {}

/// An ESP32 USB port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspPort {
    /// OS path (`/dev/ttyACM0`, `COM5`, `/dev/cu.usbmodem101`).
    pub path: String,
    /// USB details.
    pub vid: u16,
    /// Product id.
    pub pid: u16,
    /// USB serial number (the chip MAC on USB-Serial-JTAG).
    pub serial_number: Option<String>,
}

/// Espressif USB ports currently attached.
pub fn list_ports() -> Vec<EspPort> {
    let ports = serialport::available_ports().unwrap_or_default();
    let mut out: Vec<EspPort> = ports
        .into_iter()
        .filter_map(|p| match p.port_type {
            SerialPortType::UsbPort(u) if u.vid == ESPRESSIF_VID => {
                Some(EspPort { path: p.port_name, vid: u.vid, pid: u.pid, serial_number: u.serial_number })
            }
            _ => None,
        })
        // macOS lists each device as /dev/tty.* and /dev/cu.*; use cu.
        .filter(|p| !p.path.starts_with("/dev/tty.usb"))
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// What running Clawdmeter firmware says about itself (`info` command).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FirmwareInfo {
    /// PlatformIO env.
    pub env: String,
    /// Firmware version.
    pub ver: String,
    /// Panel / board revision (may be empty).
    #[serde(default, deserialize_with = "string_or_number")]
    pub rev: String,
    /// Link mode, e.g. `ble`.
    #[serde(default)]
    pub link: String,
}

fn string_or_number<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    })
}

/// Find the `info` reply among serial output lines (boot logs, `{"ready":true}`
/// and other chatter are skipped).
pub fn parse_info_output(text: &str) -> Option<FirmwareInfo> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let start = l.find('{')?;
            serde_json::from_str::<FirmwareInfo>(&l[start..]).ok()
        })
        .find(|i| !i.env.is_empty())
}

/// Probe 1: send `info\n` to running firmware and wait for the JSON reply.
/// Opening the port doesn't reset the S3/C6 USB-Serial-JTAG (DTR/RTS are
/// left as the OS sets them, like `screenshot.sh`).
pub fn probe_info(port: &str, timeout: Duration) -> Result<Option<FirmwareInfo>, DeviceError> {
    let mut sp = serialport::new(port, 115_200)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| DeviceError::from_open(port, e))?;
    let _ = sp.clear(serialport::ClearBuffer::Input);
    sp.write_all(b"\ninfo\n").map_err(DeviceError::from_io)?;
    let _ = sp.flush();
    let start = Instant::now();
    let mut acc = String::new();
    let mut buf = [0u8; 512];
    while start.elapsed() < timeout {
        match sp.read(&mut buf) {
            Ok(n) if n > 0 => {
                acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                if let Some(info) = parse_info_output(&acc) {
                    return Ok(Some(info));
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(DeviceError::from_io(e)),
        }
    }
    Ok(None)
}

/// Send one firmware serial command (`minimal`, `minimal on`, …) and return
/// the first JSON reply line that has `key`. Like [`probe_info`], opening the
/// port doesn't reset the board.
pub fn query_json(
    port: &str,
    command: &str,
    key: &str,
    timeout: Duration,
) -> Result<Option<serde_json::Value>, DeviceError> {
    let mut sp = serialport::new(port, 115_200)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| DeviceError::from_open(port, e))?;
    let _ = sp.clear(serialport::ClearBuffer::Input);
    sp.write_all(format!("\n{command}\n").as_bytes()).map_err(DeviceError::from_io)?;
    let _ = sp.flush();
    let start = Instant::now();
    let mut acc = String::new();
    let mut buf = [0u8; 512];
    while start.elapsed() < timeout {
        match sp.read(&mut buf) {
            Ok(n) if n > 0 => {
                acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                if let Some(v) = find_json_with(&acc, key) {
                    return Ok(Some(v));
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(DeviceError::from_io(e)),
        }
    }
    Ok(None)
}

/// The first complete line of `text` that parses as a JSON object with `key`.
pub fn find_json_with(text: &str, key: &str) -> Option<serde_json::Value> {
    text.split_inclusive('\n').filter(|l| l.ends_with('\n')).find_map(|l| {
        let l = l.trim();
        let v: serde_json::Value = serde_json::from_str(&l[l.find('{')?..]).ok()?;
        v.get(key).is_some().then_some(v)
    })
}

/// Probe 2 result: what the ROM reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipInfo {
    /// Normalized chip id, e.g. `esp32s3`.
    pub chip: String,
    /// Flash size in MB.
    pub flash_mb: Option<u32>,
    /// Factory MAC.
    pub mac: Option<String>,
    /// Embedded PSRAM etc., as espflash words them.
    pub features: Vec<String>,
}

fn chip_id(c: Chip) -> String {
    crate::manifest::normalize_chip(&c.to_string())
}

fn flash_size_mb(f: FlashSize) -> Option<u32> {
    Some(match f {
        FlashSize::_1Mb => 1,
        FlashSize::_2Mb => 2,
        FlashSize::_4Mb => 4,
        FlashSize::_8Mb => 8,
        FlashSize::_16Mb => 16,
        FlashSize::_32Mb => 32,
        FlashSize::_64Mb => 64,
        FlashSize::_128Mb => 128,
        FlashSize::_256Mb => 256,
        _ => return None,
    })
}

fn usb_info(port: &str) -> Result<UsbPortInfo, DeviceError> {
    serialport::available_ports()
        .map_err(|e| DeviceError::Serial(e.to_string()))?
        .into_iter()
        .find(|p| p.port_name == port)
        .and_then(|p| match p.port_type {
            SerialPortType::UsbPort(u) => Some(u),
            _ => None,
        })
        .ok_or_else(|| DeviceError::NotUsb(port.to_owned()))
}

fn connect(port: &str) -> Result<Flasher, DeviceError> {
    let info = usb_info(port)?;
    let serial = serialport::new(port, 115_200).open_native().map_err(|e| DeviceError::from_open(port, e))?;
    let conn =
        Connection::new(serial, info, ResetAfterOperation::HardReset, ResetBeforeOperation::DefaultReset, 115_200);
    Flasher::connect(conn, true, true, false, None, None).map_err(|e| DeviceError::NoBootloader(e.to_string()))
}

/// Probe 2: enter the ROM bootloader, read chip and flash size, reset back
/// into the app.
pub fn probe_chip(port: &str) -> Result<ChipInfo, DeviceError> {
    let mut f = connect(port)?;
    let di = f.device_info().map_err(|e| DeviceError::NoBootloader(e.to_string()))?;
    let out = ChipInfo {
        chip: chip_id(di.chip),
        flash_mb: flash_size_mb(di.flash_size),
        mac: di.mac_address,
        features: di.features,
    };
    let _ = f.connection().reset();
    Ok(out)
}

/// Outcome of board detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identification {
    /// Probe 1 named the env (re-flash of a Clawdmeter).
    Firmware(FirmwareInfo),
    /// Probe 2 narrowed it to exactly one board.
    Unique(String),
    /// The user must pick among these envs (shown at true aspect ratio).
    Choose(Vec<String>),
    /// Nothing in the manifest fits this chip.
    Unsupported(ChipInfo),
}

/// Narrow the manifest with a chip probe (see [`Manifest::candidates`]: an
/// image fits when it needs no more flash than the board has, exact sizes
/// first). A 32 MB S3 is the 2.06: no other supported board ships with 32 MB.
pub fn identify_from_chip(manifest: &Manifest, chip: &ChipInfo) -> Identification {
    if chip.chip == "esp32s3" && chip.flash_mb == Some(32) && manifest.board(THIRTY_TWO_MB_S3_ENV).is_some() {
        return Identification::Unique(THIRTY_TWO_MB_S3_ENV.to_owned());
    }
    let c: Vec<String> = manifest.candidates(&chip.chip, chip.flash_mb).into_iter().map(|b| b.env.clone()).collect();
    match c.len() {
        0 => Identification::Unsupported(chip.clone()),
        1 => Identification::Unique(c.into_iter().next().expect("one")),
        _ => Identification::Choose(c),
    }
}

/// Run probes 1 and 2 on `port`.
pub fn identify(port: &str, manifest: &Manifest) -> Result<Identification, DeviceError> {
    if let Ok(Some(info)) = probe_info(port, Duration::from_millis(1500)) {
        if manifest.board(&info.env).is_some() {
            return Ok(Identification::Firmware(info));
        }
        log::warn!("firmware reports env {:?}, not in this release", info.env);
    }
    let chip = probe_chip(port)?;
    Ok(identify_from_chip(manifest, &chip))
}

/// Flash progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashProgress {
    /// Started writing `total` bytes.
    Start {
        /// Bytes to write.
        total: usize,
    },
    /// `done` bytes written so far.
    Write {
        /// Bytes written.
        done: usize,
    },
    /// Checking the written data.
    Verify,
    /// Finished.
    Done,
}

/// Reports espflash's per-segment progress as bytes of the whole image, so a
/// write split around NVS still reads as a single 0..total run.
struct Progress<'a, F: FnMut(FlashProgress)> {
    report: &'a mut F,
    /// `(flash address, image bytes before it, length)` per segment.
    segments: Vec<(u32, usize, usize)>,
    /// The segment being written, as `(bytes before it, length)`.
    current: (usize, usize),
    /// espflash's unit count for the current segment, from `init`.
    units: usize,
}

impl<F: FnMut(FlashProgress)> ProgressCallbacks for Progress<'_, F> {
    fn init(&mut self, addr: u32, total: usize) {
        self.units = total.max(1);
        if let Some(&(_, base, len)) = self.segments.iter().find(|s| s.0 == addr) {
            self.current = (base, len);
        }
    }
    fn update(&mut self, current: usize) {
        let (base, len) = self.current;
        let done = base + current.min(self.units) * len / self.units;
        (self.report)(FlashProgress::Write { done });
    }
    fn verifying(&mut self) {
        (self.report)(FlashProgress::Verify);
    }
    fn finish(&mut self, _skipped: bool) {}
}

/// Partition table offset in every supported layout.
const PARTITION_TABLE: usize = 0x8000;

/// Ranges of a merged image that must not be written: NVS (`data/nvs`) and
/// NVS keys (`data/nvs_keys`). They hold the BLE bonds and the device's
/// settings; a merged image only covers them with 0xFF padding, and writing
/// that would unpair the board from every host.
pub fn preserved_ranges(image: &[u8]) -> Vec<Range<usize>> {
    let table = image.get(PARTITION_TABLE..PARTITION_TABLE + 0xC00).unwrap_or(&[]);
    let mut out = Vec::new();
    for e in table.chunks_exact(32) {
        if e[..2] != [0xAA, 0x50] {
            break;
        }
        let (kind, subtype) = (e[2], e[3]);
        let off = u32::from_le_bytes([e[4], e[5], e[6], e[7]]) as usize;
        let size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
        if kind == 0x01 && (subtype == 0x02 || subtype == 0x04) {
            out.push(off..off.saturating_add(size));
        }
    }
    out.sort_by_key(|r| r.start);
    out
}

/// The parts of `0..len` outside `preserved` (sorted), as write segments.
pub fn write_segments(len: usize, preserved: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut at = 0;
    for r in preserved {
        if r.start >= len {
            break;
        }
        if r.start > at {
            out.push(at..r.start);
        }
        at = at.max(r.end);
    }
    if at < len {
        out.push(at..len);
    }
    out
}

/// Write a merged image from 0x0 and reset into it, skipping the NVS
/// partitions ([`preserved_ranges`]) so pairings and settings survive. The
/// caller verified the image's sha256 against the manifest
/// ([`crate::release::verify`]); the chip is checked against `expected_chip`.
pub fn flash_merged(
    port: &str,
    image: &[u8],
    expected_chip: &BoardImage,
    mut progress: impl FnMut(FlashProgress),
) -> Result<(), DeviceError> {
    let mut f = connect(port)?;
    let got = chip_id(f.chip());
    if got != expected_chip.chip_id() {
        let _ = f.connection().reset();
        return Err(DeviceError::WrongChip { found: got, expected: expected_chip.chip_id() });
    }
    // One write session for every segment: espflash reboots the chip when a
    // session ends, so a second session would find no bootloader.
    let ranges = write_segments(image.len(), &preserved_ranges(image));
    let total = ranges.iter().map(|r| r.len()).sum();
    let mut base = 0;
    let mut table = Vec::new();
    let mut segments = Vec::new();
    for r in ranges {
        table.push((r.start as u32, base, r.len()));
        base += r.len();
        let mut data = image[r.clone()].to_vec();
        data.resize(data.len().next_multiple_of(4), 0xFF);
        segments.push(Segment { addr: r.start as u32, data: data.into() });
    }
    progress(FlashProgress::Start { total });
    let mut cb = Progress { report: &mut progress, segments: table, current: (0, 0), units: 1 };
    f.write_bins_to_flash(&segments, &mut cb).map_err(|e| DeviceError::Flash(e.to_string()))?;
    progress(FlashProgress::Done);
    // With the stub loader the host does the reboot.
    let chip = f.chip();
    f.connection().reset_after(true, chip).map_err(|e| DeviceError::Flash(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: u8, subtype: u8, off: u32, size: u32) -> Vec<u8> {
        let mut e = vec![0xAA, 0x50, kind, subtype];
        e.extend(off.to_le_bytes());
        e.extend(size.to_le_bytes());
        e.resize(32, 0);
        e
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn nvs_is_left_out_of_the_write() {
        // Default 8 MB layout: nvs 0x9000+0x5000, otadata, app0.
        let mut image = vec![0xFF; 0x20000];
        let table =
            [entry(1, 2, 0x9000, 0x5000), entry(1, 0, 0xE000, 0x2000), entry(0, 0x10, 0x10000, 0x330000)].concat();
        image[PARTITION_TABLE..PARTITION_TABLE + table.len()].copy_from_slice(&table);
        let keep = preserved_ranges(&image);
        assert_eq!(keep, vec![0x9000..0xE000]);
        assert_eq!(write_segments(image.len(), &keep), vec![0..0x9000, 0xE000..0x20000]);
    }

    #[test]
    fn an_image_without_a_partition_table_is_written_whole() {
        let image = vec![0xFF; 0x10000];
        assert!(preserved_ranges(&image).is_empty());
        assert_eq!(write_segments(image.len(), &[]), vec![0..0x10000]);
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn a_short_image_stops_before_nvs() {
        assert_eq!(write_segments(0x9000, &[0x9000..0xE000]), vec![0..0x9000]);
        assert_eq!(write_segments(0xA000, &[0x9000..0xE000]), vec![0..0x9000]);
    }

    #[test]
    fn finds_minimal_reply_among_chatter() {
        let out = "boot...\n{\"ready\":true}\nminimal\n{\"minimal\":true,\"override\":\"on\"}\n";
        let v = find_json_with(out, "minimal").unwrap();
        assert_eq!(v["override"], "on");
        assert!(find_json_with("{\"minimal\":tr", "minimal").is_none(), "partial line ignored");
    }

    #[test]
    fn info_parsing() {
        let out = "ESP-ROM:esp32s3\n{\"ready\":true}\nBLE: init complete\n{\"env\":\"waveshare_amoled_18\",\"ver\":\"1.9.0\",\"rev\":\"co5300\",\"link\":\"ble\"}\r\n";
        let i = parse_info_output(out).unwrap();
        assert_eq!(
            i,
            FirmwareInfo {
                env: "waveshare_amoled_18".into(),
                ver: "1.9.0".into(),
                rev: "co5300".into(),
                link: "ble".into()
            }
        );
        assert!(parse_info_output("{\"ready\":true}\n").is_none());
        assert!(parse_info_output("garbage {\"env\":").is_none());
        assert_eq!(parse_info_output("x {\"env\":\"a\",\"ver\":\"1\",\"rev\":2}").unwrap().rev, "2");
    }

    #[test]
    fn chip_names() {
        assert_eq!(chip_id(Chip::Esp32s3), "esp32s3");
        assert_eq!(chip_id(Chip::Esp32c6), "esp32c6");
        assert_eq!(flash_size_mb(FlashSize::_32Mb), Some(32));
        assert_eq!(flash_size_mb(FlashSize::_256Kb), None);
    }

    fn manifest() -> Manifest {
        let b = |env: &str, chip: &str, fs: &str| BoardImage {
            env: env.into(),
            chip: chip.into(),
            flash_size: fs.into(),
            display_name: env.into(),
            file: format!("{env}.bin"),
            sha256: String::new(),
        };
        Manifest {
            version: "1.9.0".into(),
            boards: vec![
                b("waveshare_amoled_216", "esp32s3", "16MB"),
                b("waveshare_amoled_18", "esp32s3", "16MB"),
                b("waveshare_amoled_206", "esp32s3", "32MB"),
                b("waveshare_amoled_216_c6", "esp32c6", "16MB"),
                b("waveshare_amoled_18_c6", "esp32c6", "16MB"),
            ],
        }
    }

    #[test]
    fn identification() {
        let m = manifest();
        let chip = |c: &str, mb: Option<u32>| ChipInfo { chip: c.into(), flash_mb: mb, mac: None, features: vec![] };
        assert_eq!(
            identify_from_chip(&m, &chip("esp32s3", Some(32))),
            Identification::Unique("waveshare_amoled_206".into())
        );
        assert_eq!(
            identify_from_chip(&m, &chip("esp32s3", Some(16))),
            Identification::Choose(vec!["waveshare_amoled_216".into(), "waveshare_amoled_18".into()])
        );
        assert_eq!(
            identify_from_chip(&m, &chip("esp32c6", Some(16))),
            Identification::Choose(vec!["waveshare_amoled_216_c6".into(), "waveshare_amoled_18_c6".into()])
        );
        assert!(matches!(identify_from_chip(&m, &chip("esp32c3", Some(4))), Identification::Unsupported(_)));
        assert!(matches!(identify_from_chip(&m, &chip("esp32s3", Some(4))), Identification::Unsupported(_)));
    }

    #[test]
    fn sixteen_mb_original_216_is_offered_its_8mb_image() {
        let mut m = manifest();
        m.boards[0].flash_size = "8MB".into();
        let chip = ChipInfo { chip: "esp32s3".into(), flash_mb: Some(16), mac: None, features: vec![] };
        assert_eq!(
            identify_from_chip(&m, &chip),
            Identification::Choose(vec!["waveshare_amoled_18".into(), "waveshare_amoled_216".into()])
        );
        let eight = ChipInfo { flash_mb: Some(8), ..chip };
        assert_eq!(identify_from_chip(&m, &eight), Identification::Unique("waveshare_amoled_216".into()));
    }

    #[test]
    fn error_copy() {
        assert_eq!(pretty_chip("esp32s3"), "ESP32-S3");
        assert_eq!(pretty_chip("esp32"), "ESP32");
        let e = DeviceError::WrongChip { found: "esp32c6".into(), expected: "esp32s3".into() };
        assert_eq!(
            e.to_string(),
            "This board has an ESP32-C6, but that firmware is for an ESP32-S3. Go back and pick your board again."
        );
        let p = DeviceError::PermissionDenied { port: "/dev/ttyACM0".into() }.to_string();
        assert!(p.starts_with("No permission to open /dev/ttyACM0."));
        #[cfg(target_os = "linux")]
        assert!(p.contains("dialout"));
    }
}
