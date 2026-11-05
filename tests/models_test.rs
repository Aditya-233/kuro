use kuro::models::{AnimeShow, DownloadStatus, DownloadTask, Episode, Quality};

#[test]
fn test_quality_parsing() {
    assert_eq!(Quality::from_str_loose("1080p"), Quality::P1080);
    assert_eq!(Quality::from_str_loose("1080"), Quality::P1080);
    assert_eq!(Quality::from_str_loose("720p"), Quality::P720);
    assert_eq!(Quality::from_str_loose("480p"), Quality::P480);
    assert_eq!(Quality::from_str_loose("unknown"), Quality::P1080); // default
}

#[test]
fn test_download_progress_calculation() {
    let task = DownloadTask::new(
        "test-task-1",
        "Solo Leveling - 01 (1080p).mkv",
        "/tmp/solo.mkv",
        1_000_000_000, // 1 GB
    );

    assert_eq!(task.progress_fraction(), 0.0);
    assert_eq!(task.progress_percent(), 0);
    assert_eq!(task.formatted_total_size(), "1.00 GB");

    let mut updated = task;
    updated.downloaded_bytes = 500_000_000;
    updated.speed_bps = 25_000_000; // 25 MB/s
    updated.status = DownloadStatus::Downloading;

    assert!((updated.progress_fraction() - 0.5).abs() < 0.001);
    assert_eq!(updated.progress_percent(), 50);
    assert_eq!(updated.formatted_speed(), "25.0 MB/s");
    assert_eq!(updated.formatted_eta(), "20s");
}

#[test]
fn test_anime_show_and_episode_serialization() {
    let show = AnimeShow {
        id: "solo-leveling".to_string(),
        title: "Solo Leveling".to_string(),
        english_title: Some("Solo Leveling".to_string()),
        romaji_title: Some("Na Honjaman Rebeleop".to_string()),
        episode_count: 24,
        status: "Currently Airing".to_string(),
        quality_badges: vec!["1080p".to_string(), "SubsPlease".to_string()],
        synopsis: "In a world where hunters must battle deadly monsters...".to_string(),
    };

    let serialized = serde_json::to_string(&show).expect("Failed to serialize AnimeShow");
    let deserialized: AnimeShow =
        serde_json::from_str(&serialized).expect("Failed to deserialize AnimeShow");

    assert_eq!(show.id, deserialized.id);
    assert_eq!(show.title, deserialized.title);
    assert_eq!(show.quality_badges.len(), 2);
}

#[test]
fn test_episode_no_magnet_invariant() {
    let ep = Episode {
        id: "solo-leveling-01".to_string(),
        show_id: "solo-leveling".to_string(),
        episode_number: 1,
        title: "[SubsPlease] Solo Leveling - 01 (1080p).mkv".to_string(),
        quality: Quality::P1080,
        release_group: "SubsPlease".to_string(),
        file_size_bytes: Some(1_450_000_000),
        direct_download_url: "https://subsplease.org/xdcc/solo-01.mkv".to_string(),
        xdcc_bot: Some("CR-ARUTHA".to_string()),
        xdcc_pack: Some(1042),
    };

    // Verify it contains direct download parameters and NO magnet protocol
    assert!(ep.direct_download_url.starts_with("https://"));
    assert!(!ep.direct_download_url.starts_with("magnet:?"));
    assert_eq!(ep.formatted_size(), "1.45 GB");
}
