//! Release filename parser for anime distributions.

use crate::models::Quality;
use crate::sources::SourceError;

/// Parsed metadata extracted from standard release filenames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedReleaseMeta {
    /// Subtitle / release group (e.g. `SubsPlease`).
    pub release_group: String,
    /// Canonical show title.
    pub show_name: String,
    /// Episode index number.
    pub episode_number: u32,
    /// Detected video quality.
    pub quality: Quality,
}

/// Parse metadata from standard release file names like `[SubsPlease] Show Name - 01 (1080p) [HASH].mkv`.
///
/// # Errors
/// Returns `SourceError::InvalidFormat` if the filename does not match expected release patterns.
pub fn parse_filename_metadata(filename: &str) -> Result<ParsedReleaseMeta, SourceError> {
    let trimmed = filename.trim();

    // Check for release group bracket: [Group]
    let (group, rest) = if trimmed.starts_with('[') {
        if let Some(close_idx) = trimmed.find(']') {
            let group_name = &trimmed[1..close_idx];
            let after = trimmed[close_idx + 1..].trim();
            (group_name.to_string(), after)
        } else {
            ("Unknown".to_string(), trimmed)
        }
    } else {
        ("Unknown".to_string(), trimmed)
    };

    // Remove file extension if present
    let without_ext = if let Some(dot_idx) = rest.rfind('.') {
        &rest[..dot_idx]
    } else {
        rest
    };

    // Strip CRC / hash bracket at end: [12AB34CD]
    let base = if without_ext.ends_with(']') {
        if let Some(open_idx) = without_ext.rfind('[') {
            without_ext[..open_idx].trim()
        } else {
            without_ext
        }
    } else {
        without_ext
    };

    // Quality check in parentheses: (1080p)
    let (base_no_res, quality) = if let Some(open_p) = base.rfind('(') {
        if let Some(close_p) = base[open_p..].find(')') {
            let res_str = &base[open_p + 1..open_p + close_p];
            let q = Quality::from_str_loose(res_str);
            (base[..open_p].trim(), q)
        } else {
            (base, Quality::P1080)
        }
    } else {
        (base, Quality::P1080)
    };

    // Now look for dash separator between show title and episode number: "Show Name - 01"
    if let Some(dash_idx) = base_no_res.rfind('-') {
        let show_part = base_no_res[..dash_idx].trim();
        let ep_part = base_no_res[dash_idx + 1..].trim();

        if let Ok(ep_num) = ep_part.parse::<u32>() {
            return Ok(ParsedReleaseMeta {
                release_group: group,
                show_name: show_part.to_string(),
                episode_number: ep_num,
                quality,
            });
        }
    }

    // Fallback if no clean episode dash was found
    Err(SourceError::InvalidFormat(filename.to_string()))
}
