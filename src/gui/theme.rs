//! Monochrome aesthetic theme definition for Kuro GUI.

use iced::theme::Palette;
use iced::Color;

/// Obsidian dark background `#0B0B0C`.
pub const BG_OBSIDIAN: Color = Color::from_rgb(0.043, 0.043, 0.047);

/// Elevated card surface `#141416`.
pub const SURFACE_CARD: Color = Color::from_rgb(0.078, 0.078, 0.086);

/// Subtle border color `#232326`.
pub const BORDER_SLATE: Color = Color::from_rgb(0.137, 0.137, 0.149);

/// Stark primary white `#FFFFFF`.
pub const ACCENT_WHITE: Color = Color::from_rgb(1.0, 1.0, 1.0);

/// Muted secondary text `#8E8E93`.
pub const TEXT_MUTED: Color = Color::from_rgb(0.556, 0.556, 0.576);

/// Build the custom monochrome palette for Iced.
#[must_use]
pub fn custom_monochrome_palette() -> Palette {
    Palette {
        background: BG_OBSIDIAN,
        text: ACCENT_WHITE,
        primary: ACCENT_WHITE,
        success: Color::from_rgb(0.9, 0.9, 0.9),
        warning: Color::from_rgb(0.7, 0.7, 0.7),
        danger: Color::from_rgb(0.8, 0.2, 0.2),
    }
}
