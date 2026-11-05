//! Core data models for Kuro anime engine.

use serde::{Deserialize, Serialize};

/// Video quality profile for an anime release.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum Quality {
    /// 1080p Full HD (Default profile).
    #[default]
    P1080,
    /// 720p HD.
    P720,
    /// 480p SD.
    P480,
}

impl Quality {
    /// Parse quality loosely from user input or release filename strings.
    #[must_use]
    pub fn from_str_loose(s: &str) -> Self {
        let lower = s.to_ascii_lowercase();
        if lower.contains("1080") {
            Self::P1080
        } else if lower.contains("720") {
            Self::P720
        } else if lower.contains("480") {
            Self::P480
        } else {
            Self::P1080
        }
    }

    /// Display string for the quality profile.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::P1080 => "1080p",
            Self::P720 => "720p",
            Self::P480 => "480p",
        }
    }
}

/// An anime show entry with metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimeShow {
    /// Unique identifier for the show.
    pub id: String,
    /// Canonical title of the show.
    pub title: String,
    /// English localized title, if available.
    pub english_title: Option<String>,
    /// Romaji / Japanese transliterated title, if available.
    pub romaji_title: Option<String>,
    /// Number of episodes available or known.
    pub episode_count: u32,
    /// Current broadcasting status.
    pub status: String,
    /// Badges to display in UI (e.g., "1080p", "`SubsPlease`", "24 Episodes").
    pub quality_badges: Vec<String>,
    /// Brief synopsis / description.
    pub synopsis: String,
}

/// An episode release belonging to an anime show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Episode {
    /// Unique episode identifier.
    pub id: String,
    /// Foreign key to parent `AnimeShow`.
    pub show_id: String,
    /// Numeric episode number.
    pub episode_number: u32,
    /// Display release title / file name.
    pub title: String,
    /// Release quality.
    pub quality: Quality,
    /// Subtitle / release group name (e.g. "`SubsPlease`").
    pub release_group: String,
    /// Total file size in bytes, if known.
    pub file_size_bytes: Option<u64>,
    /// Direct HTTP download URL (never magnet).
    pub direct_download_url: String,
    /// XDCC IRC bot nick, if available.
    pub xdcc_bot: Option<String>,
    /// XDCC pack number, if available.
    pub xdcc_pack: Option<u32>,
}

impl Episode {
    /// Format episode size into human-readable representation.
    #[must_use]
    pub fn formatted_size(&self) -> String {
        self.file_size_bytes
            .map_or_else(|| "Unknown size".to_string(), format_bytes)
    }
}

/// Download task status lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DownloadStatus {
    /// Queued for download.
    #[default]
    Pending,
    /// Establishing connection to XDCC / HTTP source.
    Connecting,
    /// Active download in progress.
    Downloading,
    /// Successfully finished and verified.
    Completed,
    /// Paused by user.
    Paused,
    /// Download terminated with an error description.
    Failed(String),
}

/// A tracked download task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadTask {
    /// Unique task ID.
    pub id: String,
    /// Target file name (e.g. "[`SubsPlease`] Solo Leveling - 01 (1080p).mkv").
    pub file_name: String,
    /// Output destination path on local disk.
    pub destination_path: String,
    /// Total file size in bytes.
    pub total_bytes: u64,
    /// Bytes received so far.
    pub downloaded_bytes: u64,
    /// Current download speed in bytes per second.
    pub speed_bps: u64,
    /// Current lifecycle state.
    pub status: DownloadStatus,
}

impl DownloadTask {
    /// Create a new pending download task.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        file_name: impl Into<String>,
        destination_path: impl Into<String>,
        total_bytes: u64,
    ) -> Self {
        Self {
            id: id.into(),
            file_name: file_name.into(),
            destination_path: destination_path.into(),
            total_bytes,
            downloaded_bytes: 0,
            speed_bps: 0,
            status: DownloadStatus::Pending,
        }
    }

    /// Progress fraction between 0.0 and 1.0.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn progress_fraction(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.downloaded_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0) as f32
        }
    }

    /// Progress percentage from 0 to 100.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn progress_percent(&self) -> u8 {
        (self.progress_fraction() * 100.0).round() as u8
    }

    /// Formatted total size.
    #[must_use]
    pub fn formatted_total_size(&self) -> String {
        format_bytes(self.total_bytes)
    }

    /// Formatted speed string (e.g. "15.4 MB/s").
    #[must_use]
    pub fn formatted_speed(&self) -> String {
        format!("{}/s", format_bytes(self.speed_bps))
    }

    /// Estimated time remaining formatted as a human-readable string.
    #[must_use]
    pub fn formatted_eta(&self) -> String {
        if self.status == DownloadStatus::Completed {
            return "Done".to_string();
        }
        if self.speed_bps == 0 || self.downloaded_bytes >= self.total_bytes {
            return "--".to_string();
        }
        let remaining_bytes = self.total_bytes.saturating_sub(self.downloaded_bytes);
        let seconds = remaining_bytes / self.speed_bps;
        if seconds < 60 {
            format!("{seconds}s")
        } else if seconds < 3600 {
            format!("{}m {}s", seconds / 60, seconds % 60)
        } else {
            format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
        }
    }
}

/// Helper function to format byte counts into human-readable SI/IEC strings.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}
