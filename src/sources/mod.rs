//! Anime data sources and API client.

pub mod aliases;
pub mod parser;

pub use aliases::{english_display_title, resolve_anime_aliases};
pub use parser::{parse_filename_metadata, ParsedReleaseMeta};

use crate::models::{AnimeShow, Episode, Quality};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use thiserror::Error;

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

    /// Search anime by query across `SubsPlease` and known aliases with pagination.
    ///
    /// # Errors
    /// Returns `SourceError` if network requests fail or no results are found.
    pub async fn search(&self, query: &str) -> Result<Vec<AnimeShow>, SourceError> {
        let aliases = resolve_anime_aliases(query);
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
    pub async fn get_episodes(
        &self,
        show_name: &str,
        preferred_quality: Quality,
    ) -> Result<Vec<Episode>, SourceError> {
        let encoded = urlencoding_simple(show_name);
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
                for dl in downloads {
                    let res_str = dl.get("res").and_then(|r| r.as_str()).unwrap_or("1080");
                    let quality = Quality::from_str_loose(res_str);

                    if quality == preferred_quality || preferred_quality == Quality::P1080 {
                        let direct_url = dl
                            .get("xdcc")
                            .and_then(|u| u.as_str())
                            .or_else(|| val.get("xdcc").and_then(|u| u.as_str()))
                            .unwrap_or_default();

                        if direct_url.starts_with("magnet:?") {
                            continue;
                        }

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
                            direct_download_url: direct_url.to_string(),
                            xdcc_bot: bot_name,
                            xdcc_pack: pack_num,
                        });
                        break;
                    }
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
