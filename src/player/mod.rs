//! Media player session, playback controls, and container inspection.

use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Player and media errors.
#[derive(Debug, Error)]
pub enum PlayerError {
    /// File not found on disk.
    #[error("Media file does not exist: {0}")]
    FileNotFound(PathBuf),
    /// File IO error.
    #[error("IO error while reading media file: {0}")]
    Io(#[from] std::io::Error),
    /// Unrecognized media container format.
    #[error("Unrecognized or corrupted media container")]
    UnrecognizedContainer,
}

/// Metadata information probed from a media file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    /// Target file path.
    pub path: PathBuf,
    /// Container format name (e.g. "Matroska", "MP4").
    pub container_type: String,
    /// Total file size in bytes.
    pub file_size_bytes: u64,
}

impl MediaInfo {
    /// Probe media header from file path on disk.
    ///
    /// # Errors
    /// Returns `PlayerError` if file cannot be read or container magic is invalid.
    pub fn probe(path: &Path) -> Result<Self, PlayerError> {
        if !path.exists() {
            return Err(PlayerError::FileNotFound(path.to_path_buf()));
        }

        let metadata = std::fs::metadata(path)?;
        let file_size_bytes = metadata.len();

        let mut header = [0u8; 4];
        let mut file = std::fs::File::open(path)?;
        file.read_exact(&mut header)?;

        // Matroska standard EBML ID magic bytes: 0x1A, 0x45, 0xDF, 0xA3
        let container_type = if header == [0x1A, 0x45, 0xDF, 0xA3] {
            "Matroska".to_string()
        } else if &header[..] == b"ftyp" || path.extension().is_some_and(|ext| ext == "mp4") {
            "MP4".to_string()
        } else {
            "Unknown".to_string()
        };

        Ok(Self {
            path: path.to_path_buf(),
            container_type,
            file_size_bytes,
        })
    }
}

/// In-app video playback session state machine.
#[derive(Debug, Clone)]
pub struct PlaybackSession {
    file_path: PathBuf,
    position_seconds: f64,
    duration_seconds: f64,
    is_playing: bool,
    volume: f32,
    speed: f32,
    current_frame: Option<Vec<u8>>,
}

impl PlaybackSession {
    /// Create a new playback session for a file.
    #[must_use]
    pub fn new(file_path: PathBuf, duration_seconds: f64) -> Self {
        let actual_duration = if duration_seconds > 0.0 {
            duration_seconds
        } else {
            Self::probe_duration(&file_path)
        };

        Self {
            file_path,
            position_seconds: 0.0,
            duration_seconds: actual_duration.max(1.0),
            is_playing: false,
            volume: 1.0,
            speed: 1.0,
            current_frame: None,
        }
    }

    /// Path to current media file.
    #[must_use]
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Whether playback is currently active.
    #[must_use]
    pub const fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Current volume level (0.0 to 1.0).
    #[must_use]
    pub const fn volume(&self) -> f32 {
        self.volume
    }

    /// Current playback speed multiplier.
    #[must_use]
    pub const fn speed(&self) -> f32 {
        self.speed
    }

    /// Current playback timestamp in seconds.
    #[must_use]
    pub const fn position_seconds(&self) -> f64 {
        self.position_seconds
    }

    /// Total media duration in seconds.
    #[must_use]
    pub const fn duration_seconds(&self) -> f64 {
        self.duration_seconds
    }

    /// Current in-memory frame image bytes (JPEG / PNG).
    #[must_use]
    pub fn current_frame(&self) -> Option<&[u8]> {
        self.current_frame.as_deref()
    }

    /// Set decoded frame data.
    pub fn set_frame(&mut self, frame: Vec<u8>) {
        self.current_frame = Some(frame);
    }

    /// Start playback.
    pub fn play(&mut self) {
        self.is_playing = true;
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    /// Toggle play and pause.
    pub fn toggle(&mut self) {
        self.is_playing = !self.is_playing;
    }

    /// Seek to absolute position in seconds.
    pub fn seek_to(&mut self, seconds: f64) {
        self.position_seconds = seconds.clamp(0.0, self.duration_seconds);
    }

    /// Seek relative to current position.
    pub fn seek_by(&mut self, delta_seconds: f64) {
        self.seek_to(self.position_seconds + delta_seconds);
    }

    /// Set volume with clamping between 0.0 and 1.0.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Set playback speed (clamped between 0.25x and 4.0x).
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.25, 4.0);
    }

    /// Formatted time string (e.g. "04:12 / 24:30").
    #[must_use]
    pub fn formatted_time(&self) -> String {
        format!(
            "{} / {}",
            format_time_hms(self.position_seconds),
            format_time_hms(self.duration_seconds)
        )
    }

    /// Probe duration in seconds using ffprobe if available.
    #[must_use]
    pub fn probe_duration(path: &Path) -> f64 {
        let output = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(s) = String::from_utf8(out.stdout) {
                    if let Ok(val) = s.trim().parse::<f64>() {
                        return val;
                    }
                }
            }
        }

        1440.0
    }

    /// Extract a single frame at the given timestamp using native `FFmpeg`.
    ///
    /// # Errors
    /// Returns `PlayerError` if `FFmpeg` fails or outputs zero bytes.
    pub fn extract_frame(path: &Path, position_seconds: f64) -> Result<Vec<u8>, PlayerError> {
        let pos_str = format_time_hms(position_seconds);
        let path_str = path.to_string_lossy().to_string();

        let output = std::process::Command::new("ffmpeg")
            .args([
                "-ss",
                &pos_str,
                "-i",
                &path_str,
                "-vf",
                "scale=720:-1",
                "-vframes",
                "1",
                "-f",
                "image2pipe",
                "-vcodec",
                "mjpeg",
                "-q:v",
                "3",
                "-",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()?;

        if output.status.success() && !output.stdout.is_empty() {
            Ok(output.stdout)
        } else {
            Err(PlayerError::UnrecognizedContainer)
        }
    }

    /// Launch full native Wayland player via self-contained MPV bundle.
    ///
    /// # Errors
    /// Returns `PlayerError::Io` if launching fails.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn launch_native_player(&self) -> Result<(), PlayerError> {
        let start_sec = self.position_seconds.round() as u64;
        let mut cmd = std::process::Command::new("mpv");
        cmd.args([
            "--gpu-context=wayland",
            "--title=Kuro Native Anime Player",
            &format!("--start={start_sec}"),
        ]);
        cmd.arg(&self.file_path);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        cmd.spawn()?;
        Ok(())
    }

    /// Launch native player.
    ///
    /// # Errors
    /// Returns `PlayerError::Io` if launching fails.
    pub fn launch_external(&self) -> Result<(), PlayerError> {
        self.launch_native_player()
    }
}

/// Helper function to format seconds into mm:ss or hh:mm:ss string.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn format_time_hms(seconds: f64) -> String {
    let total_secs = seconds.round() as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{hours:02}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    }
}
