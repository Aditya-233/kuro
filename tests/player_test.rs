use kuro::player::{DecodedFrame, MediaInfo, PlaybackSession};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_playback_session_controls() {
    let mut session = PlaybackSession::new(PathBuf::from("/tmp/anime.mkv"), 1470.0);

    assert_eq!(session.formatted_time(), "00:00 / 24:30");
    assert!(!session.is_playing());

    session.play();
    assert!(session.is_playing());

    session.pause();
    assert!(!session.is_playing());

    session.toggle();
    assert!(session.is_playing());

    session.seek_to(120.0);
    assert_eq!(session.formatted_time(), "02:00 / 24:30");

    session.seek_by(30.0);
    assert_eq!(session.formatted_time(), "02:30 / 24:30");

    session.seek_by(-1000.0);
    assert_eq!(session.formatted_time(), "00:00 / 24:30");

    session.seek_by(9999.0);
    assert_eq!(session.formatted_time(), "24:30 / 24:30");

    session.set_volume(0.85);
    assert!((session.volume() - 0.85).abs() < 0.001);

    session.set_volume(1.5);
    assert_eq!(session.volume(), 1.0);

    session.set_volume(-0.5);
    assert_eq!(session.volume(), 0.0);

    let dummy_frame = DecodedFrame {
        width: 100,
        height: 100,
        rgba: vec![0u8; 40000],
    };
    session.set_frame(dummy_frame.clone());
    assert_eq!(session.current_frame(), Some(&dummy_frame));
}

#[test]
fn test_media_inspector() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let test_file = tmp.path().join("episode.mkv");

    let mut f = File::create(&test_file).expect("Failed to create file");
    f.write_all(b"\x1A\x45\xDF\xA3MATROSKA_HEADER_MOCK")
        .expect("Failed to write mock MKV");
    f.flush().expect("Failed to flush");

    let info = MediaInfo::probe(&test_file).expect("Failed to probe media file");
    assert_eq!(info.container_type, "Matroska");
    assert!(info.file_size_bytes > 0);
}

#[test]
fn test_pure_rust_frame_extraction() {
    let path = std::path::Path::new("/home/aditya/Downloads/[SubsPlease] Sousou no Frieren S2 - 01 (1080p) [4277EF46].mkv");
    if path.exists() {
        let frame = PlaybackSession::extract_frame(path, 0.0).expect("Extract frame failed");
        assert!(frame.width > 0);
        assert!(frame.height > 0);
        assert_eq!(frame.rgba.len(), (frame.width * frame.height * 4) as usize);
    }
}
