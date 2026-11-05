use kuro::downloader::{DownloadEngine, DownloadProgressEvent};
use kuro::models::DownloadStatus;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_download_engine_file_writing_and_atomic_rename() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let dest_file = tmp.path().join("test_episode.mkv");

    let engine = DownloadEngine::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);

    // Write a mock test payload
    let mock_bytes = b"MOCK_MATROSKA_VIDEO_HEADER_AND_STREAM_DATA_0123456789";
    let task_id = "test-task-42";

    let handle = tokio::spawn({
        let dest = dest_file.clone();
        let payload = mock_bytes.to_vec();
        async move {
            engine
                .write_payload_atomically(task_id, &dest, &payload, tx)
                .await
        }
    });

    // Check progress events
    let mut received_progress = false;
    let mut completed = false;

    while let Some(event) = rx.recv().await {
        match event {
            DownloadProgressEvent::Progress {
                downloaded_bytes,
                total_bytes,
                ..
            } => {
                assert!(downloaded_bytes <= total_bytes);
                received_progress = true;
            }
            DownloadProgressEvent::Completed { id, final_path } => {
                assert_eq!(id, task_id);
                assert_eq!(final_path, dest_file);
                completed = true;
            }
            DownloadProgressEvent::Failed { reason, .. } => {
                unreachable!("Download failed unexpectedly: {reason}");
            }
        }
    }

    handle
        .await
        .expect("Task panicked")
        .expect("Download write failed");

    assert!(received_progress);
    assert!(completed);
    assert!(dest_file.exists());
    assert!(!tmp.path().join("test_episode.mkv.part").exists());

    let content = fs::read(&dest_file).expect("Failed to read downloaded file");
    assert_eq!(content, mock_bytes);
}

#[test]
fn test_download_status_transitions() {
    let status = DownloadStatus::Pending;
    assert_eq!(status, DownloadStatus::Pending);

    let active = DownloadStatus::Downloading;
    assert_ne!(status, active);

    let failed = DownloadStatus::Failed("Timeout".to_string());
    assert_eq!(failed, DownloadStatus::Failed("Timeout".to_string()));
}
