//! What the flasher knows about each board for its picker: a name, the panel
//! resolution and size (for the true-aspect thumbnail), and a short summary.
//! The release manifest's `display_name` wins when it has one.

/// One supported board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardInfo {
    /// PlatformIO env.
    pub env: &'static str,
    /// Short name.
    pub name: &'static str,
    /// Panel width, px.
    pub width: u32,
    /// Panel height, px.
    pub height: u32,
    /// Panel diagonal, inches (thumbnail scale).
    pub diagonal: f32,
    /// One-line summary.
    pub summary: &'static str,
    /// SoC.
    pub chip: &'static str,
}

/// Every hardware env in `firmware/platformio.ini` (not `sim` / `native`).
pub const BOARDS: [BoardInfo; 7] = [
    BoardInfo {
        env: "waveshare_amoled_216",
        name: "AMOLED 2.16",
        width: 480,
        height: 480,
        diagonal: 2.16,
        summary: "480×480 · 3 buttons",
        chip: "ESP32-S3",
    },
    BoardInfo {
        env: "waveshare_amoled_18",
        name: "AMOLED 1.8",
        width: 368,
        height: 448,
        diagonal: 1.8,
        summary: "368×448 · 2 buttons",
        chip: "ESP32-S3",
    },
    BoardInfo {
        env: "waveshare_amoled_206",
        name: "AMOLED 2.06",
        width: 410,
        height: 502,
        diagonal: 2.06,
        summary: "410×502 · watch",
        chip: "ESP32-S3",
    },
    BoardInfo {
        env: "waveshare_lcd_154",
        name: "LCD 1.54",
        width: 240,
        height: 240,
        diagonal: 1.54,
        summary: "240×240 · SPI TFT",
        chip: "ESP32-S3",
    },
    BoardInfo {
        env: "waveshare_lcd_4",
        name: "LCD 4\"",
        width: 480,
        height: 480,
        diagonal: 4.0,
        summary: "480×480 · RGB TFT",
        chip: "ESP32-S3",
    },
    BoardInfo {
        env: "waveshare_amoled_216_c6",
        name: "AMOLED 2.16 (C6)",
        width: 480,
        height: 480,
        diagonal: 2.16,
        summary: "480×480 · ESP32-C6",
        chip: "ESP32-C6",
    },
    BoardInfo {
        env: "waveshare_amoled_18_c6",
        name: "AMOLED 1.8 (C6)",
        width: 368,
        height: 448,
        diagonal: 1.8,
        summary: "368×448 · ESP32-C6",
        chip: "ESP32-C6",
    },
];

/// Look up a board by env.
pub fn board(env: &str) -> Option<&'static BoardInfo> {
    BOARDS.iter().find(|b| b.env == env)
}

impl BoardInfo {
    /// Thumbnail panel size inside a `box_px` square: true aspect ratio,
    /// scaled by diagonal relative to the 2.16 (clamped so the 1.54 stays
    /// visible and the 4" still fits).
    pub fn thumb(&self, box_px: f32) -> (f32, f32) {
        let scale = (self.diagonal / 2.16).clamp(0.62, 1.1);
        let max = box_px * 0.84 * scale / 1.1;
        let (w, h) = (self.width as f32, self.height as f32);
        if w >= h {
            (max, max * h / w)
        } else {
            (max * w / h, max)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_firmware_env_is_listed() {
        let ini = include_str!("../../../../firmware/platformio.ini");
        for line in ini.lines() {
            if let Some(env) = line.strip_prefix("[env:").and_then(|l| l.strip_suffix(']')) {
                if env == "sim" || env == "native" {
                    continue;
                }
                assert!(board(env).is_some(), "{env} missing from BOARDS");
            }
        }
    }

    #[test]
    fn thumbnails_keep_aspect() {
        let (w, h) = board("waveshare_amoled_18").unwrap().thumb(48.0);
        assert!((w / h - 368.0 / 448.0).abs() < 1e-3);
        let small = board("waveshare_lcd_154").unwrap().thumb(48.0).0;
        let big = board("waveshare_lcd_4").unwrap().thumb(48.0).0;
        assert!(small < big && big <= 48.0);
    }
}
