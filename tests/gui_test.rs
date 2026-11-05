use kuro::gui::{App, Message};
use kuro::models::{AnimeShow, Episode, Quality};

#[tokio::test]
async fn test_gui_state_machine() {
    let mut app = App::new();

    // 1. Initial State
    assert_eq!(app.shows.len(), 0);
    assert!(app.selected_show.is_none());
    assert!(!app.is_searching);

    // 2. Search Input Changed
    let _ = app.update(Message::SearchInputChanged("Solo Leveling".to_string()));
    assert_eq!(app.search_query, "Solo Leveling");

    // 3. Search Results Received
    let mock_show = AnimeShow {
        id: "solo-leveling".to_string(),
        title: "Solo Leveling".to_string(),
        english_title: Some("Solo Leveling".to_string()),
        romaji_title: None,
        episode_count: 24,
        status: "Airing".to_string(),
        quality_badges: vec!["1080p".to_string(), "SubsPlease".to_string()],
        synopsis: "Hunters anime".to_string(),
    };

    let _ = app.update(Message::SearchResultsReceived(Ok(vec![mock_show.clone()])));
    assert_eq!(app.shows.len(), 1);
    assert_eq!(app.shows[0].title, "Solo Leveling");
    assert!(!app.is_searching);

    // 4. Select Show (Opens Modal)
    let _ = app.update(Message::SelectShow(mock_show.clone()));
    assert!(app.selected_show.is_some());
    assert_eq!(
        app.selected_show.as_ref().map(|s| s.title.as_str()),
        Some("Solo Leveling")
    );

    // 5. Episodes Received
    let mock_ep = Episode {
        id: "solo-1".to_string(),
        show_id: "solo-leveling".to_string(),
        episode_number: 1,
        title: "[SubsPlease] Solo Leveling - 01 (1080p).mkv".to_string(),
        quality: Quality::P1080,
        release_group: "SubsPlease".to_string(),
        file_size_bytes: Some(1_400_000_000),
        direct_download_url: "https://subsplease.org/xdcc/solo-01.mkv".to_string(),
        xdcc_bot: Some("CR-ARUTHA".to_string()),
        xdcc_pack: Some(101),
    };

    let _ = app.update(Message::EpisodesReceived(Ok(vec![mock_ep.clone()])));
    assert_eq!(app.episodes.len(), 1);

    // 6. Trigger Download
    let _ = app.update(Message::TriggerDownload(mock_ep));
    assert_eq!(app.active_downloads.len(), 1);
    assert_eq!(
        app.active_downloads[0].file_name,
        "[SubsPlease] Solo Leveling - 01 (1080p).mkv"
    );

    // 7. Dismiss Modal
    let _ = app.update(Message::DismissModal);
    assert!(app.selected_show.is_none());

    // 8. Test Local Files Detection
    let temp_dir = std::env::temp_dir().join("kuro_gui_test_dir");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create test dir");
    let sample = temp_dir.join("[SubsPlease] Solo Leveling - 01 (1080p).mkv");
    std::fs::write(&sample, b"dummy").expect("write test file");

    let is_dl = kuro::sources::is_episode_downloaded(
        "[SubsPlease] Solo Leveling - 01 (1080p).mkv",
        &temp_dir,
    );
    assert!(is_dl.is_some());
    let _ = std::fs::remove_dir_all(&temp_dir);
}
