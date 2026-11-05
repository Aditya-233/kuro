//! Asynchronous download engine with atomic file writes and live progress streaming.

use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Instant;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Errors arising during download operations.
#[derive(Debug, Error)]
pub enum DownloadError {
    /// IO filesystem error.
    #[error("Filesystem IO error: {0}")]
    Io(#[from] std::io::Error),
    /// HTTP network error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// Download was cancelled by the user.
    #[error("Download cancelled")]
    Cancelled,
}

/// Real-time download progress events emitted to UI or CLI handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadProgressEvent {
    /// Incremental progress update with byte counts and throughput.
    Progress {
        /// Task identifier.
        id: String,
        /// Current bytes downloaded.
        downloaded_bytes: u64,
        /// Total file size.
        total_bytes: u64,
        /// Transfer speed in bytes per second.
        speed_bps: u64,
    },
    /// Download successfully completed and atomically moved to destination.
    Completed {
        /// Task identifier.
        id: String,
        /// Final verified file path on disk.
        final_path: PathBuf,
    },
    /// Download encountered an unrecoverable error.
    Failed {
        /// Task identifier.
        id: String,
        /// Description of the error.
        reason: String,
    },
}

/// Download engine orchestrating file streams and progress events.
#[derive(Debug, Clone, Default)]
pub struct DownloadEngine {
    client: reqwest::Client,
}

impl DownloadEngine {
    /// Create a new instance of the download engine.
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Kuro/0.1.0 (Linux; x86_64; KuroAnime)")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client }
    }

    /// Atomically write a test or in-memory byte buffer to the destination with live progress events.
    ///
    /// # Errors
    /// Returns `DownloadError::Io` on filesystem write or rename failure.
    pub async fn write_payload_atomically(
        &self,
        task_id: &str,
        dest: &Path,
        payload: &[u8],
        progress_tx: mpsc::Sender<DownloadProgressEvent>,
    ) -> Result<(), DownloadError> {
        let part_path = dest.with_extension("mkv.part");

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(&part_path).await?;
        let total_bytes = payload.len() as u64;
        let chunk_size = 8192;
        let mut downloaded = 0u64;

        for chunk in payload.chunks(chunk_size) {
            file.write_all(chunk).await?;
            downloaded += chunk.len() as u64;

            let _ = progress_tx
                .send(DownloadProgressEvent::Progress {
                    id: task_id.to_string(),
                    downloaded_bytes: downloaded,
                    total_bytes,
                    speed_bps: 10_000_000,
                })
                .await;
        }

        file.flush().await?;
        drop(file);

        // Atomic rename from .part to final destination
        tokio::fs::rename(&part_path, dest).await?;

        let _ = progress_tx
            .send(DownloadProgressEvent::Completed {
                id: task_id.to_string(),
                final_path: dest.to_path_buf(),
            })
            .await;

        Ok(())
    }

    /// Download a remote file URL directly into a target file path.
    ///
    /// # Errors
    /// Returns `DownloadError` if the network stream fails or disk writes fail.
    pub async fn download_url(
        &self,
        task_id: &str,
        url: &str,
        dest: &Path,
        progress_tx: mpsc::Sender<DownloadProgressEvent>,
    ) -> Result<(), DownloadError> {
        let response = match self.client.get(url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                let _ = progress_tx
                    .send(DownloadProgressEvent::Failed {
                        id: task_id.to_string(),
                        reason: e.to_string(),
                    })
                    .await;
                return Err(DownloadError::Http(e));
            }
        };

        if !response.status().is_success() {
            let reason = format!("HTTP error status: {}", response.status());
            let _ = progress_tx
                .send(DownloadProgressEvent::Failed {
                    id: task_id.to_string(),
                    reason: reason.clone(),
                })
                .await;
            return Err(DownloadError::Io(std::io::Error::other(reason)));
        }

        let total_bytes = response.content_length().unwrap_or(0);
        let part_path = dest.with_extension("mkv.part");

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(&part_path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded_bytes = 0u64;

        let start_time = Instant::now();
        let mut last_emit = Instant::now();

        while let Some(chunk_res) = stream.next().await {
            let chunk: bytes::Bytes = match chunk_res {
                Ok(c) => c,
                Err(e) => {
                    let _ = progress_tx
                        .send(DownloadProgressEvent::Failed {
                            id: task_id.to_string(),
                            reason: e.to_string(),
                        })
                        .await;
                    return Err(DownloadError::Http(e));
                }
            };

            file.write_all(&chunk).await?;
            downloaded_bytes += chunk.len() as u64;

            // Emit progress events at most once every 100ms
            if last_emit.elapsed().as_millis() >= 100 {
                let elapsed_secs = start_time.elapsed().as_secs_f64();
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss
                )]
                let speed_bps = if elapsed_secs > 0.0 {
                    (downloaded_bytes as f64 / elapsed_secs).max(0.0) as u64
                } else {
                    0
                };

                let _ = progress_tx
                    .send(DownloadProgressEvent::Progress {
                        id: task_id.to_string(),
                        downloaded_bytes,
                        total_bytes,
                        speed_bps,
                    })
                    .await;

                last_emit = Instant::now();
            }
        }

        file.flush().await?;
        drop(file);

        // Atomic move
        tokio::fs::rename(&part_path, dest).await?;

        let _ = progress_tx
            .send(DownloadProgressEvent::Completed {
                id: task_id.to_string(),
                final_path: dest.to_path_buf(),
            })
            .await;

        Ok(())
    }

    /// Download an episode by dispatching to the local XDCC daemon or fallback HTTP.
    ///
    /// # Errors
    /// Returns `DownloadError` on failure.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::too_many_lines
    )]
    pub async fn download_episode(
        &self,
        task_id: &str,
        episode: &crate::models::Episode,
        dest: &Path,
        progress_tx: mpsc::Sender<DownloadProgressEvent>,
    ) -> Result<(), DownloadError> {
        let server_url = "http://127.0.0.1:8888";
        let bot = episode.xdcc_bot.clone().unwrap_or_default();
        let pack = episode.xdcc_pack.map(|p| p.to_string()).unwrap_or_default();
        let filename = dest
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let magnet = episode.direct_download_url.clone();

        let payload = serde_json::json!({
            "bot": bot,
            "pack": pack,
            "filename": filename,
            "magnet": magnet
        });

        let dl_endpoint = format!("{server_url}/api/download");
        let post_res = self.client.post(&dl_endpoint).json(&payload).send().await;

        if let Ok(resp) = post_res {
            if resp.status().is_success() {
                let poll_url = format!("{server_url}/api/downloads");
                let mut retry_count = 0;

                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                    if let Ok(poll_resp) = self.client.get(&poll_url).send().await {
                        if let Ok(poll_val) = poll_resp.json::<serde_json::Value>().await {
                            if let Some(active) = poll_val.get("active").and_then(|a| a.as_object())
                            {
                                if let Some(t_info) = active.get(&filename) {
                                    let status =
                                        t_info.get("status").and_then(|s| s.as_str()).unwrap_or("");
                                    let speed_mb = t_info
                                        .get("speed_mb")
                                        .and_then(serde_json::Value::as_f64)
                                        .unwrap_or(0.0);
                                    let received_mb = t_info
                                        .get("received_mb")
                                        .and_then(serde_json::Value::as_f64)
                                        .unwrap_or(0.0);
                                    let total_mb = t_info
                                        .get("total_mb")
                                        .and_then(serde_json::Value::as_f64)
                                        .unwrap_or(0.0);

                                    let downloaded_bytes = (received_mb * 1024.0 * 1024.0) as u64;
                                    let total_bytes = (total_mb * 1024.0 * 1024.0) as u64;
                                    let speed_bps = (speed_mb * 1024.0 * 1024.0) as u64;

                                    let _ = progress_tx
                                        .send(DownloadProgressEvent::Progress {
                                            id: task_id.to_string(),
                                            downloaded_bytes,
                                            total_bytes,
                                            speed_bps,
                                        })
                                        .await;

                                    if status == "completed" {
                                        let _ = progress_tx
                                            .send(DownloadProgressEvent::Completed {
                                                id: task_id.to_string(),
                                                final_path: dest.to_path_buf(),
                                            })
                                            .await;
                                        return Ok(());
                                    }

                                    if status == "error" {
                                        let err = t_info
                                            .get("error")
                                            .and_then(|e| e.as_str())
                                            .unwrap_or("Download error");
                                        let _ = progress_tx
                                            .send(DownloadProgressEvent::Failed {
                                                id: task_id.to_string(),
                                                reason: err.to_string(),
                                            })
                                            .await;
                                        return Err(DownloadError::Io(std::io::Error::other(err)));
                                    }
                                } else if retry_count > 4 {
                                    // Check if moved to completed files array
                                    if let Some(files) =
                                        poll_val.get("files").and_then(|f| f.as_array())
                                    {
                                        let in_completed = files.iter().any(|f| {
                                            f.get("filename").and_then(|n| n.as_str())
                                                == Some(&filename)
                                        });
                                        if in_completed {
                                            let _ = progress_tx
                                                .send(DownloadProgressEvent::Completed {
                                                    id: task_id.to_string(),
                                                    final_path: dest.to_path_buf(),
                                                })
                                                .await;
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    retry_count += 1;
                    if retry_count > 600 {
                        break;
                    }
                }
            }
        }

        if episode.direct_download_url.starts_with("http://")
            || episode.direct_download_url.starts_with("https://")
        {
            return self
                .download_url(task_id, &episode.direct_download_url, dest, progress_tx)
                .await;
        }

        let reason = "Download service unavailable or no active bot".to_string();
        let _ = progress_tx
            .send(DownloadProgressEvent::Failed {
                id: task_id.to_string(),
                reason: reason.clone(),
            })
            .await;
        Err(DownloadError::Io(std::io::Error::other(reason)))
    }
}
