//! Anime data sources and API client.

pub mod aliases;
pub mod parser;

pub use aliases::{english_display_title, resolve_anime_aliases};
pub use parser::{parse_filename_metadata, ParsedReleaseMeta};

use crate::models::{AnimeShow, Episode, LocalVideoFile, Quality};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

/// Errors that can occur during anime searching and parsing.
#[derive(Debug, Error)]
pub enum SourceError {
    /// Network HTTP communication failure.
    #[error("HTTP request failed: {0}")]
    Network(#[from] reqwest::Error),
    /// JSON parsing failure.
    #[error("JSON deserialization failed: {0}")]
    Json(#[from] serde_json::Error),
    /// No anime found matching the given query.
    #[error("No anime results found for query: {0}")]
    NotFound(String),
    /// Release filename format not recognized.
    #[error("Invalid filename release format: {0}")]
    InvalidFormat(String),
}

/// High-level anime source client.
#[derive(Debug, Clone)]
pub struct AnimeSourceClient {
    client: reqwest::Client,
    base_url: String,
    local_server_url: String,
    cached_episodes: Arc<Mutex<HashMap<String, Vec<Episode>>>>,
}

impl Default for AnimeSourceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimeSourceClient {
    /// Create a new anime source client.
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Kuro/0.1.0 (Linux; x86_64; KuroAnime)")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            base_url: "https://subsplease.org/api/".to_string(),
            local_server_url: "http://127.0.0.1:8888".to_string(),
            cached_episodes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Parse `SubsPlease` search API JSON response into `AnimeShow` vector.
    ///
    /// # Errors
    /// Returns `SourceError::Json` if the payload cannot be deserialized.
    pub fn parse_subsplease_search_response(
        &self,
        json_str: &str,
    ) -> Result<Vec<AnimeShow>, SourceError> {
        let trimmed = json_str.trim();
        if trimmed.starts_with('[') {
            return Ok(Vec::new());
        }

        let raw_val: serde_json::Value = serde_json::from_str(trimmed)?;
        let Some(obj) = raw_val.as_object() else {
            return Ok(Vec::new());
        };

        // Group by show title
        let mut show_counts: HashMap<String, u32> = HashMap::new();

        for (key, val) in obj {
            let show_name = val
                .get("show")
                .and_then(|s| s.as_str())
                .unwrap_or_else(|| {
                    if let Some((show_part, _)) = key.split_once(" - ") {
                        show_part
                    } else {
                        key.as_str()
                    }
                })
                .trim();

            *show_counts.entry(show_name.to_string()).or_insert(0) += 1;
        }

        let mut shows = Vec::new();
        for (title, count) in show_counts {
            let slug = title.to_ascii_lowercase().replace(' ', "-");
            let eng = english_display_title(&title).or_else(|| Some(title.clone()));
            shows.push(AnimeShow {
                id: slug,
                title: title.clone(),
                english_title: eng,
                romaji_title: None,
                episode_count: count,
                status: "Available".to_string(),
                quality_badges: vec!["1080p".to_string(), "SubsPlease".to_string()],
                synopsis: "High-definition release from SubsPlease".to_string(),
            });
        }

        shows.sort_by(|a, b| a.title.cmp(&b.title));
        Ok(shows)
    }

    /// Search anime by query across local daemon and `SubsPlease` with pagination.
    ///
    /// # Errors
    /// Returns `SourceError` if network requests fail or no results are found.
    #[allow(clippy::too_many_lines)]
    pub async fn search(&self, query: &str) -> Result<Vec<AnimeShow>, SourceError> {
        let aliases = resolve_anime_aliases(query);

        // 1. First attempt: check local multi-strategy daemon (matching anime_server.py)
        for cand in &aliases {
            let encoded = urlencoding_simple(cand);
            let s_url = format!("{}/api/search?q={}", self.local_server_url, encoded);
            if let Ok(resp) = self.client.get(&s_url).send().await {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(arr) = data.get("results").and_then(|r| r.as_array()) {
                                if !arr.is_empty() {
                                    let mut shows = Vec::new();
                                    let mut ep_cache = self.cached_episodes.lock().await;

                                    for show_val in arr {
                                        let title = show_val
                                            .get("title")
                                            .and_then(|t| t.as_str())
                                            .unwrap_or("")
                                            .trim()
                                            .to_string();
                                        if title.is_empty() {
                                            continue;
                                        }
                                        #[allow(clippy::cast_possible_truncation)]
                                        let eps_count = show_val
                                            .get("episodes_count")
                                            .and_then(serde_json::Value::as_u64)
                                            .unwrap_or(0)
                                            as u32;
                                        let slug = title.to_ascii_lowercase().replace(' ', "-");
                                        let eng = english_display_title(&title)
                                            .or_else(|| Some(title.clone()));

                                        let mut eps = Vec::new();
                                        if let Some(ep_arr) =
                                            show_val.get("episodes").and_then(|e| e.as_array())
                                        {
                                            for ep_v in ep_arr {
                                                let ep_str = ep_v
                                                    .get("ep")
                                                    .and_then(|e| e.as_str())
                                                    .unwrap_or("0");
                                                let ep_num: u32 = ep_str.parse().unwrap_or(0);
                                                let res_str = ep_v
                                                    .get("res")
                                                    .and_then(|r| r.as_str())
                                                    .unwrap_or("1080p");
                                                let quality = Quality::from_str_loose(res_str);
                                                let group = ep_v
                                                    .get("group")
                                                    .and_then(|g| g.as_str())
                                                    .unwrap_or("SubsPlease")
                                                    .to_string();
                                                let filename = ep_v
                                                    .get("filename")
                                                    .and_then(|f| f.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let magnet = ep_v
                                                    .get("magnet")
                                                    .and_then(|m| m.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let bot = ep_v
                                                    .get("bot")
                                                    .and_then(|b| b.as_str())
                                                    .filter(|b| *b != "Archive")
                                                    .map(ToString::to_string);
                                                let pack = ep_v
                                                    .get("pack")
                                                    .and_then(|p| p.as_str())
                                                    .and_then(|p| p.parse::<u32>().ok())
                                                    .filter(|p| *p > 0);
                                                let sz_mb = ep_v
                                                    .get("size")
                                                    .and_then(|s| s.as_str())
                                                    .and_then(|s| s.parse::<u64>().ok())
                                                    .unwrap_or(1400);

                                                eps.push(Episode {
                                                    id: format!("{slug}-{ep_str}-{res_str}"),
                                                    show_id: title.clone(),
                                                    episode_number: ep_num,
                                                    title: filename,
                                                    quality,
                                                    release_group: group,
                                                    file_size_bytes: Some(sz_mb * 1024 * 1024),
                                                    direct_download_url: magnet,
                                                    xdcc_bot: bot,
                                                    xdcc_pack: pack,
                                                });
                                            }
                                        }

                                        ep_cache.insert(title.clone(), eps);

                                        shows.push(AnimeShow {
                                            id: slug,
                                            title: title.clone(),
                                            english_title: eng,
                                            romaji_title: None,
                                            episode_count: eps_count,
                                            status: "Available".to_string(),
                                            quality_badges: vec![
                                                "1080p".to_string(),
                                                "SubsPlease".to_string(),
                                                "Direct XDCC".to_string(),
                                            ],
                                            synopsis:
                                                "High-definition release from SubsPlease & Archive"
                                                    .to_string(),
                                        });
                                    }

                                    if !shows.is_empty() {
                                        return Ok(shows);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Direct SubsPlease API fallback with multi-page pagination
        let mut show_counts: HashMap<String, u32> = HashMap::new();
        let mut show_subtitles: HashMap<String, String> = HashMap::new();
        let mut seen_release_keys: HashSet<String> = HashSet::new();

        for candidate in aliases {
            let encoded = urlencoding_simple(&candidate);
            for p in 0..3 {
                let url = format!(
                    "{}?f=search&tz=Asia/Kolkata&s={}&p={}",
                    self.base_url, encoded, p
                );

                let Ok(resp) = self.client.get(&url).send().await else {
                    break;
                };
                if !resp.status().is_success() {
                    break;
                }
                let Ok(text) = resp.text().await else {
                    break;
                };
                let trimmed = text.trim();
                if trimmed.starts_with('[') {
                    break;
                }
                let Ok(raw_val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                    break;
                };
                let Some(obj) = raw_val.as_object() else {
                    break;
                };
                if obj.is_empty() {
                    break;
                }

                let page_size = obj.len();
                for (key, val) in obj {
                    if seen_release_keys.insert(key.clone()) {
                        let show_name = val
                            .get("show")
                            .and_then(|s| s.as_str())
                            .unwrap_or_else(|| {
                                if let Some((show_part, _)) = key.split_once(" - ") {
                                    show_part
                                } else {
                                    key.as_str()
                                }
                            })
                            .trim();

                        *show_counts.entry(show_name.to_string()).or_insert(0) += 1;
                        if let Some(eng) = english_display_title(show_name) {
                            show_subtitles.entry(show_name.to_string()).or_insert(eng);
                        }
                    }
                }

                if page_size < 30 {
                    break;
                }
            }
        }

        if show_counts.is_empty() {
            return Err(SourceError::NotFound(query.to_string()));
        }

        let mut shows = Vec::new();
        for (title, count) in show_counts {
            let slug = title.to_ascii_lowercase().replace(' ', "-");
            let eng = show_subtitles
                .get(&title)
                .cloned()
                .or_else(|| Some(title.clone()));
            shows.push(AnimeShow {
                id: slug,
                title: title.clone(),
                english_title: eng,
                romaji_title: None,
                episode_count: count,
                status: "Available".to_string(),
                quality_badges: vec!["1080p".to_string(), "SubsPlease".to_string()],
                synopsis: "High-definition release from SubsPlease".to_string(),
            });
        }

        shows.sort_by(|a, b| a.title.cmp(&b.title));
        Ok(shows)
    }

    /// Retrieve episode releases for a given anime show with multi-page pagination.
    ///
    /// # Errors
    /// Returns `SourceError` if network or parsing fails.
    #[allow(clippy::too_many_lines)]
    pub async fn get_episodes(
        &self,
        show_name: &str,
        preferred_quality: Quality,
    ) -> Result<Vec<Episode>, SourceError> {
        // 1. Check cached episodes from local daemon
        {
            let ep_cache = self.cached_episodes.lock().await;
            if let Some(cached) = ep_cache.get(show_name) {
                if !cached.is_empty() {
                    return Ok(cached.clone());
                }
            }
        }

        // 2. Query local daemon directly for this show
        let encoded = urlencoding_simple(show_name);
        let s_url = format!("{}/api/search?q={}", self.local_server_url, encoded);
        if let Ok(resp) = self.client.get(&s_url).send().await {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(arr) = data.get("results").and_then(|r| r.as_array()) {
                            for show_val in arr {
                                let title = show_val
                                    .get("title")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("")
                                    .trim();
                                if title.eq_ignore_ascii_case(show_name)
                                    || show_name.contains(title)
                                    || title.contains(show_name)
                                {
                                    if let Some(ep_arr) =
                                        show_val.get("episodes").and_then(|e| e.as_array())
                                    {
                                        let mut eps = Vec::new();
                                        for ep_v in ep_arr {
                                            let ep_str = ep_v
                                                .get("ep")
                                                .and_then(|e| e.as_str())
                                                .unwrap_or("0");
                                            let ep_num: u32 = ep_str.parse().unwrap_or(0);
                                            let res_str = ep_v
                                                .get("res")
                                                .and_then(|r| r.as_str())
                                                .unwrap_or("1080p");
                                            let quality = Quality::from_str_loose(res_str);
                                            let group = ep_v
                                                .get("group")
                                                .and_then(|g| g.as_str())
                                                .unwrap_or("SubsPlease")
                                                .to_string();
                                            let filename = ep_v
                                                .get("filename")
                                                .and_then(|f| f.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let magnet = ep_v
                                                .get("magnet")
                                                .and_then(|m| m.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let bot = ep_v
                                                .get("bot")
                                                .and_then(|b| b.as_str())
                                                .filter(|b| *b != "Archive")
                                                .map(ToString::to_string);
                                            let pack = ep_v
                                                .get("pack")
                                                .and_then(|p| p.as_str())
                                                .and_then(|p| p.parse::<u32>().ok())
                                                .filter(|p| *p > 0);
                                            let sz_mb = ep_v
                                                .get("size")
                                                .and_then(|s| s.as_str())
                                                .and_then(|s| s.parse::<u64>().ok())
                                                .unwrap_or(1400);

                                            eps.push(Episode {
                                                id: format!("{show_name}-{ep_str}-{res_str}"),
                                                show_id: show_name.to_string(),
                                                episode_number: ep_num,
                                                title: filename,
                                                quality,
                                                release_group: group,
                                                file_size_bytes: Some(sz_mb * 1024 * 1024),
                                                direct_download_url: magnet,
                                                xdcc_bot: bot,
                                                xdcc_pack: pack,
                                            });
                                        }
                                        if !eps.is_empty() {
                                            return Ok(eps);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Fallback to direct SubsPlease API with multi-page pagination
        let mut all_episodes = Vec::new();
        let mut seen_ids = HashSet::new();

        for p in 0..10 {
            let url = format!(
                "{}?f=search&tz=Asia/Kolkata&s={}&p={}",
                self.base_url, encoded, p
            );

            let resp = self.client.get(&url).send().await?;
            let text = resp.text().await?;
            let eps = self.parse_episodes_from_json(&text, show_name, preferred_quality)?;
            if eps.is_empty() {
                break;
            }

            let mut added = 0;
            for ep in eps {
                if seen_ids.insert(ep.id.clone()) {
                    all_episodes.push(ep);
                    added += 1;
                }
            }

            if added == 0 {
                break;
            }
        }

        all_episodes.sort_by(|a, b| {
            b.episode_number
                .cmp(&a.episode_number)
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(all_episodes)
    }

    /// Parse episodes from a search/show JSON response.
    ///
    /// # Errors
    /// Returns `SourceError::Json` on invalid JSON.
    #[allow(clippy::too_many_lines)]
    pub fn parse_episodes_from_json(
        &self,
        json_str: &str,
        show_name: &str,
        preferred_quality: Quality,
    ) -> Result<Vec<Episode>, SourceError> {
        let trimmed = json_str.trim();
        if trimmed.starts_with('[') {
            return Ok(Vec::new());
        }

        let raw_val: serde_json::Value = serde_json::from_str(trimmed)?;
        let Some(obj) = raw_val.as_object() else {
            return Ok(Vec::new());
        };

        let mut episodes = Vec::new();

        for (key, val) in obj {
            let item_show = val.get("show").and_then(|s| s.as_str()).unwrap_or("");
            if !item_show.is_empty()
                && !item_show.eq_ignore_ascii_case(show_name)
                && !show_name.contains(item_show)
                && !item_show.contains(show_name)
            {
                continue;
            }

            let ep_num: u32 = val
                .get("episode")
                .and_then(|e| {
                    if let Some(n) = e.as_u64() {
                        u32::try_from(n).ok()
                    } else {
                        e.as_str().and_then(|s| s.parse().ok())
                    }
                })
                .or_else(|| {
                    key.split_once(" - ")
                        .and_then(|(_, ep_part)| ep_part.trim().parse().ok())
                })
                .unwrap_or(0);

            if let Some(downloads) = val.get("downloads").and_then(|d| d.as_array()) {
                let mut best_dl = None;
                for dl in downloads {
                    let res_str = dl.get("res").and_then(|r| r.as_str()).unwrap_or("1080");
                    let quality = Quality::from_str_loose(res_str);
                    if quality == preferred_quality {
                        best_dl = Some((dl, quality, res_str));
                        break;
                    }
                    match best_dl {
                        None => best_dl = Some((dl, quality, res_str)),
                        Some((_, curr_q, _)) if quality.rank() > curr_q.rank() => {
                            best_dl = Some((dl, quality, res_str));
                        }
                        _ => {}
                    }
                }

                if let Some((dl, quality, res_str)) = best_dl {
                    let direct_url = dl
                        .get("xdcc")
                        .and_then(|u| u.as_str())
                        .or_else(|| val.get("xdcc").and_then(|u| u.as_str()))
                        .unwrap_or_default();

                    let magnet = dl
                        .get("magnet")
                        .and_then(|m| m.as_str())
                        .unwrap_or_default();

                    let dl_url = if !direct_url.is_empty() && !direct_url.starts_with("magnet:") {
                        direct_url
                    } else {
                        magnet
                    };

                    let pack_num = dl.get("pack").and_then(|p| {
                        if let Some(n) = p.as_u64() {
                            u32::try_from(n).ok()
                        } else {
                            p.as_str().and_then(|s| s.parse().ok())
                        }
                    });

                    let bot_name = dl
                        .get("bot")
                        .and_then(|b| b.as_str())
                        .map(ToString::to_string);

                    let ep_str = val.get("episode").and_then(|e| e.as_str()).unwrap_or("");
                    let file_name = if ep_str.eq_ignore_ascii_case("movie") {
                        format!(
                            "[SubsPlease] {} - Movie ({}).mkv",
                            show_name,
                            quality.as_str()
                        )
                    } else {
                        format!(
                            "[SubsPlease] {} - {:02} ({}).mkv",
                            show_name,
                            ep_num,
                            quality.as_str()
                        )
                    };

                    episodes.push(Episode {
                        id: format!("{show_name}-{ep_num}-{res_str}"),
                        show_id: show_name.to_string(),
                        episode_number: ep_num,
                        title: file_name,
                        quality,
                        release_group: "SubsPlease".to_string(),
                        file_size_bytes: Some(1_400_000_000),
                        direct_download_url: dl_url.to_string(),
                        xdcc_bot: bot_name,
                        xdcc_pack: pack_num,
                    });
                }
            }
        }

        episodes.sort_by(|a, b| {
            b.episode_number
                .cmp(&a.episode_number)
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(episodes)
    }
}

/// Scan local directory for downloaded anime video files (.mkv, .mp4, .webm).
#[must_use]
pub fn scan_local_downloads(dir: &Path) -> Vec<LocalVideoFile> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if matches!(ext, "mkv" | "mp4" | "webm" | "avi") {
                        let filename = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let size_mb = entry.metadata().map_or(0, |m| m.len() / (1024 * 1024));
                        files.push(LocalVideoFile {
                            filename,
                            path,
                            size_mb,
                        });
                    }
                }
            }
        }
    }
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    files
}

/// Check if an episode is already downloaded in the given directory.
#[must_use]
pub fn is_episode_downloaded(ep_title: &str, download_dir: &Path) -> Option<PathBuf> {
    let exact = download_dir.join(ep_title);
    if exact.exists() {
        return Some(exact);
    }

    if let Ok(entries) = std::fs::read_dir(download_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.eq_ignore_ascii_case(ep_title)
                    || (name.len() > 10 && ep_title.contains(&*name))
                    || (ep_title.len() > 10 && name.contains(ep_title))
                {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Simple percent-encoding for URL query parameters.
fn urlencoding_simple(input: &str) -> String {
    let mut encoded = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                let _ = write!(encoded, "%{b:02X}");
            }
        }
    }
    encoded
}
