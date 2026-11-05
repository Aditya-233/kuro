use kuro::models::Quality;
use kuro::sources::{parse_filename_metadata, resolve_anime_aliases, AnimeSourceClient};

#[test]
fn test_title_aliases() {
    let aliases = resolve_anime_aliases("The Quintessential Quintuplets");
    assert!(aliases
        .iter()
        .any(|a| a.contains("Go-toubun no Hanayome") || a.contains("Gotoubun")));

    let aot_aliases = resolve_anime_aliases("Attack on Titan");
    assert!(aot_aliases.iter().any(|a| a.contains("Shingeki no Kyojin")));

    let solo = resolve_anime_aliases("Solo Leveling");
    assert!(solo.iter().any(|a| a.eq_ignore_ascii_case("Solo Leveling")));
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
