//! Design tokens (spec §01). Every colour maps to a `THEME_*` constant in
//! `firmware/src/theme.h`, except the desktop-only surface, hairline, mute,
//! warning and focus colours. Light and dark follow the OS appearance.

use gpui::{rgb, Rgba, WindowAppearance};

/// Display face (Tiempos Text).
pub const FONT_DISPLAY: &str = "Tiempos Text";
/// Body face (Styrene B).
pub const FONT_BODY: &str = "Styrene B";
/// Data face (DejaVu Sans Mono).
pub const FONT_MONO: &str = "DejaVu Sans Mono";

/// Radius for inputs.
pub const R_S: f32 = 6.0;
/// Radius for panels (the device's zone radius).
pub const R_M: f32 = 12.0;
/// Radius for the window chrome.
pub const R_L: f32 = 20.0;
/// Pill radius.
pub const R_PILL: f32 = 999.0;

/// Resolved colour tokens.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// `--bg`: THEME_BG (true black on dark).
    pub bg: Rgba,
    /// `--surface`: window background (desktop).
    pub surface: Rgba,
    /// `--panel`: THEME_PANEL.
    pub panel: Rgba,
    /// `--line`: hairlines (desktop).
    pub line: Rgba,
    /// `--track`: THEME_BAR_BG.
    pub track: Rgba,
    /// `--text`: THEME_TEXT.
    pub text: Rgba,
    /// `--dim`: THEME_DIM.
    pub dim: Rgba,
    /// `--mute`: tertiary text (desktop).
    pub mute: Rgba,
    /// `--accent`: THEME_ACCENT, Clawd and primary actions.
    pub accent: Rgba,
    /// `--accent-ink`: text on accent.
    pub accent_ink: Rgba,
    /// `--ok`: THEME_GREEN.
    pub ok: Rgba,
    /// `--warn`: desktop amber (the firmware's THEME_AMBER is the accent).
    pub warn: Rgba,
    /// `--bad`: THEME_RED family.
    pub bad: Rgba,
    /// `--focus`.
    pub focus: Rgba,
    /// Dark appearance.
    pub dark: bool,
}

impl Theme {
    /// The dark tokens (the device's own palette).
    pub fn dark() -> Self {
        Theme {
            bg: rgb(0x000000),
            surface: rgb(0x121211),
            panel: rgb(0x1f1f1e),
            line: rgb(0x34332f),
            track: rgb(0x2a2a28),
            text: rgb(0xfaf9f5),
            dim: rgb(0xb0aea5),
            mute: rgb(0x8a887f),
            accent: rgb(0xd97757),
            accent_ink: rgb(0x141413),
            ok: rgb(0x788c5d),
            warn: rgb(0xd4a24c),
            bad: rgb(0xd4553f),
            focus: rgb(0xe8a48b),
            dark: true,
        }
    }

    /// The light tokens.
    pub fn light() -> Self {
        Theme {
            bg: rgb(0xfaf9f5),
            surface: rgb(0xffffff),
            panel: rgb(0xf0eee6),
            line: rgb(0xdcd9ce),
            track: rgb(0xe3e0d6),
            text: rgb(0x141413),
            dim: rgb(0x5e5d59),
            mute: rgb(0x7a7870),
            accent: rgb(0xc6613f),
            accent_ink: rgb(0xffffff),
            ok: rgb(0x5f7348),
            warn: rgb(0xa87a1f),
            bad: rgb(0xb33a26),
            focus: rgb(0xc6613f),
            dark: false,
        }
    }

    /// Follow the window's appearance.
    pub fn for_appearance(a: WindowAppearance) -> Self {
        match a {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Theme::light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Theme::dark(),
        }
    }
}
