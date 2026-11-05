//! Agent diagnostics, automation handlers, and CLI execution.

use crate::downloader::{DownloadEngine, DownloadProgressEvent};
use crate::models::{DownloadTask, Quality};
use crate::player::{MediaInfo, PlaybackSession};
use crate::sources::{resolve_anime_aliases, AnimeSourceClient};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Result summary of an agent diagnostic check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    /// Name of subsystem tested.
    pub name: String,
    /// Whether the check succeeded.
    pub passed: bool,
    /// Diagnostic message or detail.
    pub detail: String,
}

/// Aggregated report from agent self-test suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    /// List of individual diagnostic checks.
    pub checks: Vec<DiagnosticCheck>,
}

impl DiagnosticReport {
    /// Returns true if every diagnostic check passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }
}

/// Run a complete diagnostic health check across all core subsystems.
pub async fn run_agent_diagnostics() -> DiagnosticReport {
    let mut checks = Vec::new();

    // 1. Title Aliases Check
    let aliases = resolve_anime_aliases("The Quintessential Quintuplets");
    let alias_ok = aliases.iter().any(|a| a.contains("Go-toubun no Hanayome"));
    checks.push(DiagnosticCheck {
        name: "Title Normalization & Aliases".to_string(),
        passed: alias_ok,
        detail: format!(
            "Resolved {} aliases for Quintessential Quintuplets",
            aliases.len()
        ),
    });

    // 2. Data Models & SI Unit Formatter Check
    let task = DownloadTask::new("diag-1", "test.mkv", "/tmp/test.mkv", 1_000_000_000);
    let model_ok = task.formatted_total_size() == "1.00 GB";
    checks.push(DiagnosticCheck {
        name: "Data Models & Size Formatting".to_string(),
        passed: model_ok,
        detail: format!("1GB formatted as {}", task.formatted_total_size()),
    });

    // 3. Mock SubsPlease Search Parser Check
    let sample_json = r#"{"Test": {"show": "Test Show", "episodes": {}}}"#;
    let client = AnimeSourceClient::new();
    let parse_ok = client.parse_subsplease_search_response(sample_json).is_ok();
    checks.push(DiagnosticCheck {
        name: "Search Response Parser".to_string(),
        passed: parse_ok,
        detail: "Successfully decoded SubsPlease JSON contract".to_string(),
    });

    // 4. Download Engine Atomic Writer Check
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("kuro_diag_test.mkv");
    let engine = DownloadEngine::new();
    let (tx, mut rx) = mpsc::channel(16);

    let write_ok = engine
        .write_payload_atomically("diag-task", &test_file, b"DIAG_DATA", tx)
        .await
        .is_ok();

    // Drain channel
    while rx.recv().await.is_some() {}
    let _ = tokio::fs::remove_file(&test_file).await;

    checks.push(DiagnosticCheck {
        name: "Download Engine Atomic Writer".to_string(),
        passed: write_ok,
        detail: "Atomic .part file buffer write and rename verified".to_string(),
    });

    // 5. Player State Machine Check
    let mut session = PlaybackSession::new(PathBuf::from("/tmp/diag.mkv"), 600.0);
    session.play();
    session.seek_to(120.0);
    let player_ok = session.is_playing() && (session.position_seconds() - 120.0).abs() < 0.001;

    checks.push(DiagnosticCheck {
        name: "In-App Playback State Machine".to_string(),
        passed: player_ok,
        detail: format!("Playback state: {}", session.formatted_time()),
    });

    DiagnosticReport { checks }
}

/// Execute CLI search command and print output.
///
/// # Errors
/// Returns error string on network failure.
pub async fn execute_search(query: &str, json: bool) -> Result<(), String> {
    let client = AnimeSourceClient::new();
    if !json {
        println!("Searching for \"{query}\"...");
    }

    match client.search(query).await {
        Ok(shows) => {
            if json {
                let serialized = serde_json::to_string_pretty(&shows)
                    .map_err(|e| format!("Serialization error: {e}"))?;
                println!("{serialized}");
            } else {
                println!("\nFound {} results for \"{}\":\n", shows.len(), query);
                println!("{:<4} {:<45} {:<15} QUALITY", "#", "SHOW TITLE", "EPISODES");
                println!("{}", "─".repeat(80));
                for (i, show) in shows.iter().enumerate() {
                    println!(
                        "{:<4} {:<45} {:<15} {}",
                        i + 1,
                        truncate_str(&show.title, 43),
                        format!("{} eps", show.episode_count),
                        show.quality_badges.join(", ")
                    );
                }
                println!();
            }
            Ok(())
        }
        Err(e) => Err(format!("Search failed: {e}")),
    }
}

/// Execute CLI episodes command.
///
/// # Errors
/// Returns error string on network failure.
pub async fn execute_episodes(show: &str, quality_str: &str, json: bool) -> Result<(), String> {
    let client = AnimeSourceClient::new();
    let quality = Quality::from_str_loose(quality_str);

    match client.get_episodes(show, quality).await {
        Ok(episodes) => {
            if json {
                let serialized = serde_json::to_string_pretty(&episodes)
                    .map_err(|e| format!("Serialization error: {e}"))?;
                println!("{serialized}");
            } else {
                println!("\nEpisodes for \"{}\" ({}):\n", show, quality.as_str());
                println!(
                    "{:<5} {:<55} {:<10} SIZE",
                    "EP", "RELEASE FILENAME", "QUALITY"
                );
                println!("{}", "─".repeat(80));
                for ep in &episodes {
                    println!(
                        "EP {:<2} {:<55} {:<10} {}",
                        ep.episode_number,
                        truncate_str(&ep.title, 53),
                        ep.quality.as_str(),
                        ep.formatted_size()
                    );
                }
                println!();
            }
            Ok(())
        }
        Err(e) => Err(format!("Failed to retrieve episodes: {e}")),
    }
}

/// Execute CLI download command.
///
/// # Errors
/// Returns error string on download failure.
pub async fn execute_download(
    show: &str,
    ep_num: u32,
    quality_str: &str,
    output_dir: Option<PathBuf>,
) -> Result<(), String> {
    let client = AnimeSourceClient::new();
    let quality = Quality::from_str_loose(quality_str);

    let episodes = client
        .get_episodes(show, quality)
        .await
        .map_err(|e| format!("Failed to find episodes for {show}: {e}"))?;

    let episode = episodes
        .iter()
        .find(|e| e.episode_number == ep_num)
        .ok_or_else(|| format!("Episode {ep_num} not found for {show}"))?;

    let dest_dir = output_dir.unwrap_or_else(|| {
        directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    });

    let dest_file = dest_dir.join(&episode.title);
    println!("Starting download:");
    println!("  Title:       {}", episode.title);
    println!("  Quality:     {}", episode.quality.as_str());
    println!("  Destination: {}", dest_file.display());

    let engine = DownloadEngine::new();
    let (tx, mut rx) = mpsc::channel(32);

    let task_id = format!("{show}-{ep_num}");
    let ep_clone = episode.clone();

    tokio::spawn(async move {
        let _ = engine
            .download_episode(&task_id, &ep_clone, &dest_file, tx)
            .await;
    });

    while let Some(event) = rx.recv().await {
        match event {
            DownloadProgressEvent::Progress {
                downloaded_bytes,
                total_bytes,
                speed_bps,
                ..
            } => {
                let percent = (downloaded_bytes * 100)
                    .checked_div(total_bytes)
                    .unwrap_or(0);
                print!(
                    "\rDownloading: {:>3}% ({}/s)",
                    percent,
                    crate::models::format_bytes(speed_bps)
                );
                let _ = std::io::stdout().flush();
            }
            DownloadProgressEvent::Completed { final_path, .. } => {
                println!("\n✓ Download completed: {}", final_path.display());
                return Ok(());
            }
            DownloadProgressEvent::Failed { reason, .. } => {
                println!("\n✗ Download failed: {reason}");
                return Err(format!("Download failed: {reason}"));
            }
        }
    }

    Ok(())
}

/// Execute CLI inspect command.
///
/// # Errors
/// Returns error string on probing failure.
pub fn execute_inspect(path: &Path, json: bool) -> Result<(), String> {
    let info = MediaInfo::probe(path).map_err(|e| format!("Probe failed: {e}"))?;

    if json {
        let val = serde_json::json!({
            "path": info.path,
            "container": info.container_type,
            "size_bytes": info.file_size_bytes,
        });
        println!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
    } else {
        println!("Media File Inspection:");
        println!("  Path:      {}", info.path.display());
        println!("  Container: {}", info.container_type);
        println!(
            "  Size:      {}",
            crate::models::format_bytes(info.file_size_bytes)
        );
    }
    Ok(())
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}
