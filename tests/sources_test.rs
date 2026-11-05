use kuro::models::Quality;
use kuro::sources::{
    english_display_title, parse_filename_metadata, resolve_anime_aliases, AnimeSourceClient,
};

#[test]
fn test_title_aliases() {
    let aliases = resolve_anime_aliases("The Quintessential Quintuplets");
    assert!(aliases
        .iter()
        .any(|a| a.contains("Go-toubun no Hanayome") || a.contains("Gotoubun")));

    let quin_short = resolve_anime_aliases("Quintuplets");
    assert!(quin_short
        .iter()
        .any(|a| a.contains("Go-toubun no Hanayome") || a.contains("Gotoubun")));

    let aot_aliases = resolve_anime_aliases("Attack on Titan");
    assert!(aot_aliases.iter().any(|a| a.contains("Shingeki no Kyojin")));

    // Test partial keyword "Attack" expands to Shingeki no Kyojin
    let attack_aliases = resolve_anime_aliases("Attack");
    assert!(attack_aliases
        .iter()
        .any(|a| a.contains("Shingeki no Kyojin")));
    assert!(attack_aliases.iter().any(|a| a.contains("Shingeki")));

    // Test partial keyword "Titan" expands to Shingeki no Kyojin
    let titan_aliases = resolve_anime_aliases("Titan");
    assert!(titan_aliases
        .iter()
        .any(|a| a.contains("Shingeki no Kyojin")));

    // Test acronym "aot"
    let aot_acronym = resolve_anime_aliases("aot");
    assert!(aot_acronym.iter().any(|a| a.contains("Shingeki no Kyojin")));

    let solo = resolve_anime_aliases("Solo Leveling");
    assert!(solo.iter().any(|a| a.eq_ignore_ascii_case("Solo Leveling")));

    let demon = resolve_anime_aliases("Demon");
    assert!(demon.iter().any(|a| a.contains("Kimetsu no Yaiba")));
}

#[test]
fn test_english_display_titles() {
    let eng_snk = english_display_title("Shingeki no Kyojin (The Final Season)");
    assert_eq!(
        eng_snk.as_deref(),
        Some("Attack on Titan (The Final Season)")
    );

    let eng_movie = english_display_title("Shingeki no Kyojin Movie - The Last Attack");
    assert_eq!(
        eng_movie.as_deref(),
        Some("Attack on Titan Movie - The Last Attack")
    );

    let eng_kny = english_display_title("Kimetsu no Yaiba - Hashira Geiko-hen");
    assert_eq!(eng_kny.as_deref(), Some("Demon Slayer - Hashira Geiko-hen"));
}

#[test]
fn test_parse_filename_metadata() {
    let filename = "[SubsPlease] Solo Leveling - 01 (1080p) [12AB34CD].mkv";
    let meta = parse_filename_metadata(filename).expect("Should parse SubsPlease format");

    assert_eq!(meta.release_group, "SubsPlease");
    assert_eq!(meta.show_name, "Solo Leveling");
    assert_eq!(meta.episode_number, 1);
    assert_eq!(meta.quality, Quality::P1080);

    let s2_filename = "[SubsPlease] Go-toubun no Hanayome S2 - 12 (720p) [DEADBEEF].mkv";
    let s2_meta = parse_filename_metadata(s2_filename).expect("Should parse season format");
    assert_eq!(s2_meta.show_name, "Go-toubun no Hanayome S2");
    assert_eq!(s2_meta.episode_number, 12);
    assert_eq!(s2_meta.quality, Quality::P720);
}

#[tokio::test]
async fn test_subsplease_mock_json_parsing_and_grouping() {
    let sample_json = r#"{
        "Solo Leveling - 01": {
            "show": "Solo Leveling",
            "episode": "01",
            "downloads": [
                { "res": "1080", "xdcc": "https://subsplease.org/xdcc/solo-01.mkv", "bot": "Gattsu", "pack": 14630 }
            ]
        },
        "Solo Leveling - 02": {
            "show": "Solo Leveling",
            "episode": "02",
            "downloads": [
                { "res": "1080", "xdcc": "https://subsplease.org/xdcc/solo-02.mkv", "bot": "Gattsu", "pack": 14631 }
            ]
        }
    }"#;

    let client = AnimeSourceClient::new();
    let shows = client
        .parse_subsplease_search_response(sample_json)
        .expect("Failed to parse JSON");

    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0].title, "Solo Leveling");
    assert_eq!(shows[0].episode_count, 2);
}

#[tokio::test]
async fn test_live_attack_search_returns_all_shows() {
    let client = AnimeSourceClient::new();
    let shows = client
        .search("Attack")
        .await
        .expect("Search for Attack should succeed");

    // Must return all Attack on Titan / Shingeki no Kyojin entries, not just 1 movie!
    assert!(
        shows.len() >= 3,
        "Expected at least 3 shows for 'Attack', got {}",
        shows.len()
    );

    let titles: Vec<&str> = shows.iter().map(|s| s.title.as_str()).collect();
    println!("Found shows for 'Attack': {titles:?}");

    assert!(titles
        .iter()
        .any(|t| t.contains("The Final Season") || t.contains("Shingeki no Kyojin")));
    assert!(titles.iter().any(|t| t.contains("Last Attack")));
}

#[test]
fn test_local_downloads_scan_and_matching() {
    let temp_dir = std::env::temp_dir().join("kuro_test_local_downloads");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create test dir");

    let sample_file = temp_dir.join("[SubsPlease] Jujutsu Kaisen - 59 (1080p).mkv");
    std::fs::write(&sample_file, b"test content 123").expect("write sample file");

    let non_video = temp_dir.join("notes.txt");
    std::fs::write(&non_video, b"ignore me").expect("write non-video");

    let scanned = kuro::sources::scan_local_downloads(&temp_dir);
    assert_eq!(scanned.len(), 1);
    assert_eq!(
        scanned[0].filename,
        "[SubsPlease] Jujutsu Kaisen - 59 (1080p).mkv"
    );

    let match_exact = kuro::sources::is_episode_downloaded(
        "[SubsPlease] Jujutsu Kaisen - 59 (1080p).mkv",
        &temp_dir,
    );
    assert!(match_exact.is_some());

    let match_approx =
        kuro::sources::is_episode_downloaded("Jujutsu Kaisen - 59 (1080p)", &temp_dir);
    assert!(match_approx.is_some());

    let match_none = kuro::sources::is_episode_downloaded("Sousou no Frieren - 05", &temp_dir);
    assert!(match_none.is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_1080p_quality_priority_parsing() {
    let multi_res_json = r#"{
        "Jujutsu Kaisen - 59": {
            "show": "Jujutsu Kaisen",
            "episode": "59",
            "downloads": [
                { "res": "480", "xdcc": "480p_url", "bot": "Gattsu", "pack": 100 },
                { "res": "720", "xdcc": "720p_url", "bot": "Gattsu", "pack": 101 },
                { "res": "1080", "xdcc": "1080p_url", "bot": "Gattsu", "pack": 102 }
            ]
        }
    }"#;

    let client = AnimeSourceClient::new();
    let episodes = client
        .parse_episodes_from_json(multi_res_json, "Jujutsu Kaisen", Quality::P1080)
        .expect("Should parse episodes");

    assert_eq!(episodes.len(), 1);
    assert_eq!(episodes[0].quality, Quality::P1080);
    assert!(episodes[0].title.contains("1080p"));
    assert_eq!(episodes[0].xdcc_pack, Some(102));
}
