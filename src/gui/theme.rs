//! T3-Code inspired minimalist aesthetic theme definition for Kuro.

use iced::theme::Palette;
use iced::widget::{button, container, text_input};
use iced::{Background, Color};

/// Canvas / Background `#0A0A0A` (ultra-dark deep void).
pub const CANVAS: Color = Color::from_rgb(0.039, 0.039, 0.039);

/// Surface / Card background `#111111`.
pub const SURFACE: Color = Color::from_rgb(0.067, 0.067, 0.067);

/// Surface Raised / Element hover `#161616`.
pub const SURFACE_RAISED: Color = Color::from_rgb(0.086, 0.086, 0.086);

/// Surface Overlay `#18181B`.
pub const SURFACE_OVERLAY: Color = Color::from_rgb(0.094, 0.094, 0.106);

/// Border `#1E1E1E` (sleek subtle hairline).
pub const BORDER: Color = Color::from_rgb(0.125, 0.125, 0.125);

/// Border Highlighted `#2E2E2E`.
pub const BORDER_HIGHLIGHT: Color = Color::from_rgb(0.180, 0.180, 0.180);

/// Primary Text `#F5F5F5` (soft pure white).
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.961, 0.961, 0.961);

/// Muted / Secondary Text `#818181`.
pub const TEXT_MUTED: Color = Color::from_rgb(0.506, 0.506, 0.506);

/// Subtle Placeholder Text `#555555`.
pub const TEXT_PLACEHOLDER: Color = Color::from_rgb(0.333, 0.333, 0.333);

/// Signature T3 Accent Blue `#346BF1`.
pub const ACCENT_BLUE: Color = Color::from_rgb(0.204, 0.420, 0.945);

/// Accent Blue Hover `#3061D9`.
pub const ACCENT_BLUE_HOVER: Color = Color::from_rgb(0.188, 0.380, 0.851);

/// Success Green `#10B981`.
pub const ACCENT_SUCCESS: Color = Color::from_rgb(0.063, 0.725, 0.506);

/// Error Red `#FB414A`.
pub const ACCENT_DANGER: Color = Color::from_rgb(0.984, 0.255, 0.290);

/// Build the custom T3-Code inspired palette for Iced.
#[must_use]
pub fn custom_t3_palette() -> Palette {
    Palette {
        background: CANVAS,
        text: TEXT_PRIMARY,
        primary: ACCENT_BLUE,
        success: ACCENT_SUCCESS,
        warning: Color::from_rgb(0.996, 0.604, 0.0),
        danger: ACCENT_DANGER,
    }
}

/// Primary button style: T3 electric blue with white text.
#[must_use]
pub fn button_primary_style(status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => ACCENT_BLUE_HOVER,
        button::Status::Pressed => Color::from_rgb(0.15, 0.30, 0.75),
        button::Status::Disabled => Color::from_rgb(0.12, 0.16, 0.28),
        button::Status::Active => ACCENT_BLUE,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: TEXT_PRIMARY,
        border: iced::Border {
            radius: 6.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

/// High-contrast white button style.
#[must_use]
pub fn button_white_style(status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color::from_rgb(0.88, 0.88, 0.88),
        button::Status::Pressed => Color::from_rgb(0.75, 0.75, 0.75),
        button::Status::Disabled => Color::from_rgb(0.25, 0.25, 0.25),
        button::Status::Active => Color::WHITE,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::BLACK,
        border: iced::Border {
            radius: 6.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

/// Secondary button style: Dark elevated surface with subtle border.
#[must_use]
pub fn button_secondary_style(status: button::Status) -> button::Style {
    let (bg, border_color) = match status {
        button::Status::Hovered => (Color::from_rgb(0.12, 0.12, 0.14), BORDER_HIGHLIGHT),
        button::Status::Pressed => (Color::from_rgb(0.15, 0.15, 0.18), BORDER_HIGHLIGHT),
        button::Status::Disabled => (SURFACE, BORDER),
        button::Status::Active => (SURFACE_RAISED, BORDER),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: TEXT_PRIMARY,
        border: iced::Border {
            radius: 6.0.into(),
            width: 1.0,
            color: border_color,
        },
        ..Default::default()
    }
}

/// Danger button style: subtle red outline.
#[must_use]
pub fn button_danger_style(status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color::from_rgb(0.35, 0.08, 0.10),
        button::Status::Pressed => Color::from_rgb(0.25, 0.05, 0.06),
        button::Status::Disabled => Color::TRANSPARENT,
        button::Status::Active => Color::from_rgb(0.20, 0.06, 0.08),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: ACCENT_DANGER,
        border: iced::Border {
            radius: 6.0.into(),
            width: 1.0,
            color: Color::from_rgb(0.50, 0.15, 0.18),
        },
        ..Default::default()
    }
}

/// Card container style.
#[must_use]
pub fn card_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: BORDER,
        },
        ..Default::default()
    }
}

/// Badge container style.
#[must_use]
pub fn badge_style(is_accent: bool) -> container::Style {
    if is_accent {
        container::Style {
            background: Some(Background::Color(Color::from_rgb(0.08, 0.14, 0.28))),
            border: iced::Border {
                radius: 4.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.16, 0.28, 0.55),
            },
            ..Default::default()
        }
    } else {
        container::Style {
            background: Some(Background::Color(SURFACE_RAISED)),
            border: iced::Border {
                radius: 4.0.into(),
                width: 1.0,
                color: BORDER,
            },
            ..Default::default()
        }
    }
}

/// Search text input styling.
#[must_use]
pub fn search_input_style(status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => ACCENT_BLUE,
        text_input::Status::Hovered => BORDER_HIGHLIGHT,
        text_input::Status::Active | text_input::Status::Disabled => BORDER,
    };
    text_input::Style {
        background: Background::Color(SURFACE),
        border: iced::Border {
            radius: 6.0.into(),
            width: 1.0,
            color: border_color,
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_PLACEHOLDER,
        value: TEXT_PRIMARY,
        selection: Color::from_rgb(0.20, 0.35, 0.70),
    }
}
