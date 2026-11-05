//! Iced Monochrome Native Desktop GUI for Kuro.

pub mod theme;

use crate::downloader::{DownloadEngine, DownloadProgressEvent};
use crate::models::{AnimeShow, DownloadStatus, DownloadTask, Episode, Quality};
use crate::player::PlaybackSession;
use crate::sources::AnimeSourceClient;
use iced::widget::{
    button, center, column, container, mouse_area, opaque, row, scrollable, space, stack, text,
    text_input,
};
use iced::{Alignment, Background, Color, Element, Fill, Length, Task, Theme};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Main GUI application state.
#[derive(Debug, Clone)]
pub struct App {
    /// Active search text input.
    pub search_query: String,
    /// Loaded anime show results.
    pub shows: Vec<AnimeShow>,
    /// Currently selected show for episode modal viewing.
    pub selected_show: Option<AnimeShow>,
    /// Loaded episodes for currently selected show.
    pub episodes: Vec<Episode>,
    /// Active and completed download tasks.
    pub active_downloads: Vec<DownloadTask>,
    /// Search in-flight flag.
    pub is_searching: bool,
    /// Active in-app playback session.
    pub playback_session: Option<PlaybackSession>,
    /// Status message or error banner.
    pub status_message: Option<String>,
    source_client: Arc<AnimeSourceClient>,
    download_engine: Arc<DownloadEngine>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// GUI Message event variants.
#[derive(Debug, Clone)]
pub enum Message {
    /// User typed into search bar.
    SearchInputChanged(String),
    /// User pressed enter or clicked search button.
    SearchSubmitted,
    /// Background search task completed.
    SearchResultsReceived(Result<Vec<AnimeShow>, String>),
    /// User clicked on an anime card to view episodes.
    SelectShow(AnimeShow),
    /// User dismissed the episode modal.
    DismissModal,
    /// Background episode fetch completed.
    EpisodesReceived(Result<Vec<Episode>, String>),
    /// User clicked download on a specific episode.
    TriggerDownload(Episode),
    /// Real-time progress update from download task.
    DownloadProgress(DownloadProgressEvent),
    /// Launch in-app or system playback for an episode.
    PlayEpisode(PathBuf),
    /// Toggle play / pause during playback.
    TogglePlayback,
    /// Dismiss the playback overlay.
    ClosePlayer,
}

impl App {
    /// Create a new Kuro GUI application instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            shows: Vec::new(),
            selected_show: None,
            episodes: Vec::new(),
            active_downloads: Vec::new(),
            is_searching: false,
            playback_session: None,
            status_message: None,
            source_client: Arc::new(AnimeSourceClient::new()),
            download_engine: Arc::new(DownloadEngine::new()),
        }
    }

    /// Theme specification using our custom monochrome palette.
    #[must_use]
    pub fn theme(&self) -> Theme {
        Theme::custom("Kuro Monochrome", theme::custom_monochrome_palette())
    }

    /// Elm Architecture update cycle.
    #[allow(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SearchInputChanged(query) => {
                self.search_query = query;
                Task::none()
            }
            Message::SearchSubmitted => {
                let query = self.search_query.trim().to_string();
                if query.is_empty() {
                    return Task::none();
                }

                self.is_searching = true;
                self.status_message = Some(format!("Searching for \"{query}\"..."));
                let client = Arc::clone(&self.source_client);

                Task::perform(
                    async move { client.search(&query).await.map_err(|e| e.to_string()) },
                    Message::SearchResultsReceived,
                )
            }
            Message::SearchResultsReceived(result) => {
                self.is_searching = false;
                match result {
                    Ok(shows) => {
                        self.status_message = Some(format!("Found {} shows", shows.len()));
                        self.shows = shows;
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Error: {e}"));
                        self.shows.clear();
                    }
                }
                Task::none()
            }
            Message::SelectShow(show) => {
                self.selected_show = Some(show.clone());
                self.episodes.clear();
                self.status_message = Some(format!("Fetching episodes for {}...", show.title));
                let client = Arc::clone(&self.source_client);
                let title = show.title.clone();

                Task::perform(
                    async move {
                        client
                            .get_episodes(&title, Quality::P1080)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::EpisodesReceived,
                )
            }
            Message::DismissModal => {
                self.selected_show = None;
                self.episodes.clear();
                Task::none()
            }
            Message::EpisodesReceived(result) => {
                match result {
                    Ok(episodes) => {
                        self.status_message = Some(format!("Loaded {} episodes", episodes.len()));
                        self.episodes = episodes;
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Error loading episodes: {e}"));
                    }
                }
                Task::none()
            }
            Message::TriggerDownload(ep) => {
                let download_dir = directories::UserDirs::new()
                    .and_then(|u| u.download_dir().map(std::path::Path::to_path_buf))
                    .unwrap_or_else(|| PathBuf::from("."));

                let dest_path = download_dir.join(&ep.title);
                let task_id = format!("{}-{}", ep.show_id, ep.episode_number);

                let task = DownloadTask::new(
                    &task_id,
                    &ep.title,
                    dest_path.to_string_lossy().to_string(),
                    ep.file_size_bytes.unwrap_or(1_400_000_000),
                );

                self.active_downloads.push(task);
                self.status_message = Some(format!("Queued download for {}", ep.title));

                let engine = Arc::clone(&self.download_engine);
                let url = ep.direct_download_url.clone();
                let dest = dest_path;

                let (tx, rx) = mpsc::channel(16);

                tokio::spawn(async move {
                    let _ = engine.download_url(&task_id, &url, &dest, tx).await;
                });

                let stream = iced::futures::stream::unfold(rx, |mut rx| async move {
                    let event = rx.recv().await?;
                    Some((event, rx))
                });

                Task::run(stream, Message::DownloadProgress)
            }
            Message::DownloadProgress(event) => {
                match event {
                    DownloadProgressEvent::Progress {
                        id,
                        downloaded_bytes,
                        total_bytes,
                        speed_bps,
                    } => {
                        if let Some(task) = self.active_downloads.iter_mut().find(|t| t.id == id) {
                            task.downloaded_bytes = downloaded_bytes;
                            task.total_bytes = total_bytes;
                            task.speed_bps = speed_bps;
                            task.status = DownloadStatus::Downloading;
                        }
                    }
                    DownloadProgressEvent::Completed { id, final_path } => {
                        if let Some(task) = self.active_downloads.iter_mut().find(|t| t.id == id) {
                            task.status = DownloadStatus::Completed;
                            self.status_message = Some(format!("Completed: {}", task.file_name));
                        }
                        // Automatically make available for playback
                        self.playback_session = Some(PlaybackSession::new(final_path, 1440.0));
                    }
                    DownloadProgressEvent::Failed { id, reason } => {
                        if let Some(task) = self.active_downloads.iter_mut().find(|t| t.id == id) {
                            task.status = DownloadStatus::Failed(reason.clone());
                            self.status_message = Some(format!("Download failed: {reason}"));
                        }
                    }
                }
                Task::none()
            }
            Message::PlayEpisode(path) => {
                let session = PlaybackSession::new(path, 1440.0);
                self.playback_session = Some(session);
                Task::none()
            }
            Message::TogglePlayback => {
                if let Some(session) = &mut self.playback_session {
                    session.toggle();
                }
                Task::none()
            }
            Message::ClosePlayer => {
                self.playback_session = None;
                Task::none()
            }
        }
    }

    /// Elm Architecture view layout.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn view(&self) -> Element<'_, Message> {
        let header = row![
            text("KURO / 黒").size(24).color(theme::ACCENT_WHITE),
            space::horizontal(),
            text_input(
                "Search anime (e.g. Solo Leveling, Quintessential Quintuplets)...",
                &self.search_query
            )
            .on_input(Message::SearchInputChanged)
            .on_submit(Message::SearchSubmitted)
            .padding(10)
            .width(420),
            button(text(if self.is_searching { "..." } else { "Search" }))
                .on_press(Message::SearchSubmitted)
                .padding([10, 20]),
        ]
        .spacing(15)
        .align_y(Alignment::Center);

        let status_bar = if let Some(msg) = &self.status_message {
            row![text(msg).size(13).color(theme::TEXT_MUTED)]
        } else {
            row![
                text("Ready — Search for an anime show above to get started")
                    .size(13)
                    .color(theme::TEXT_MUTED)
            ]
        };

        // Shows Grid / Cards
        let mut cards_col = column![].spacing(16);
        let mut current_row = row![].spacing(16);

        for (i, show) in self.shows.iter().enumerate() {
            let card = container(
                column![
                    text(&show.title).size(18).color(theme::ACCENT_WHITE),
                    row![
                        container(
                            text(format!("{} Episodes", show.episode_count))
                                .size(12)
                                .color(theme::ACCENT_WHITE)
                        )
                        .padding([4, 8])
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgb(0.15, 0.15, 0.18))),
                            border: iced::Border {
                                radius: 4.0.into(),
                                width: 1.0,
                                color: theme::BORDER_SLATE,
                            },
                            ..Default::default()
                        }),
                        container(text("1080p").size(12).color(theme::TEXT_MUTED))
                            .padding([4, 8])
                            .style(|_| container::Style {
                                background: Some(Background::Color(Color::from_rgb(
                                    0.12, 0.12, 0.14
                                ))),
                                border: iced::Border {
                                    radius: 4.0.into(),
                                    width: 1.0,
                                    color: theme::BORDER_SLATE,
                                },
                                ..Default::default()
                            }),
                        container(text("SubsPlease").size(12).color(theme::TEXT_MUTED))
                            .padding([4, 8])
                            .style(|_| container::Style {
                                background: Some(Background::Color(Color::from_rgb(
                                    0.12, 0.12, 0.14
                                ))),
                                border: iced::Border {
                                    radius: 4.0.into(),
                                    width: 1.0,
                                    color: theme::BORDER_SLATE,
                                },
                                ..Default::default()
                            }),
                    ]
                    .spacing(8),
                    space::vertical(),
                    row![
                        text("Click to view episodes")
                            .size(12)
                            .color(theme::TEXT_MUTED),
                        space::horizontal(),
                        button(text("Episodes →")).on_press(Message::SelectShow(show.clone())),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(12)
                .height(130),
            )
            .width(360)
            .padding(16)
            .style(|_| container::Style {
                background: Some(Background::Color(theme::SURFACE_CARD)),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: theme::BORDER_SLATE,
                },
                ..Default::default()
            });

            current_row = current_row.push(card);
            if (i + 1) % 2 == 0 {
                cards_col = cards_col.push(current_row);
                current_row = row![].spacing(16);
            }
        }

        if !self.shows.is_empty() && !self.shows.len().is_multiple_of(2) {
            cards_col = cards_col.push(current_row);
        }

        // Active Downloads Panel
        let mut downloads_section = column![].spacing(8);
        if !self.active_downloads.is_empty() {
            downloads_section = downloads_section
                .push(text("Active Downloads").size(14).color(theme::ACCENT_WHITE));
            for task in &self.active_downloads {
                let item = row![
                    text(&task.file_name).size(13).width(350),
                    text(format!("{}%", task.progress_percent()))
                        .size(13)
                        .color(theme::ACCENT_WHITE),
                    text(task.formatted_speed())
                        .size(12)
                        .color(theme::TEXT_MUTED),
                    text(format!("ETA: {}", task.formatted_eta()))
                        .size(12)
                        .color(theme::TEXT_MUTED),
                    space::horizontal(),
                    if task.status == DownloadStatus::Completed {
                        button(text("Play"))
                            .on_press(Message::PlayEpisode(PathBuf::from(&task.destination_path)))
                    } else {
                        button(text("Downloading...")).padding([2, 8])
                    }
                ]
                .spacing(16)
                .align_y(Alignment::Center);

                downloads_section = downloads_section.push(item);
            }
        }

        let main_content = container(
            column![
                header,
                status_bar,
                scrollable(cards_col).height(Fill),
                downloads_section,
            ]
            .spacing(20),
        )
        .padding(24)
        .width(Fill)
        .height(Fill);

        // Episode Modal Overlay (Stack)
        if let Some(show) = &self.selected_show {
            let mut ep_list = column![].spacing(8);
            for ep in &self.episodes {
                let ep_row = row![
                    container(
                        text(format!("EP {:02}", ep.episode_number))
                            .size(12)
                            .color(theme::ACCENT_WHITE)
                    )
                    .padding([4, 8])
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgb(0.18, 0.18, 0.22))),
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 1.0,
                            color: theme::BORDER_SLATE,
                        },
                        ..Default::default()
                    }),
                    text(&ep.title).size(13).width(Length::FillPortion(3)),
                    text("1080p").size(12).color(theme::TEXT_MUTED),
                    // Strictly direct download button (NO magnet links!)
                    button(text("Download")).on_press(Message::TriggerDownload(ep.clone())),
                ]
                .spacing(12)
                .align_y(Alignment::Center);

                ep_list = ep_list.push(ep_row);
            }

            let modal_box = container(
                column![
                    row![
                        text(&show.title).size(20).color(theme::ACCENT_WHITE),
                        space::horizontal(),
                        button(text("✕")).on_press(Message::DismissModal),
                    ]
                    .align_y(Alignment::Center),
                    scrollable(ep_list).height(400),
                ]
                .spacing(16),
            )
            .width(680)
            .padding(20)
            .style(|_| container::Style {
                background: Some(Background::Color(theme::SURFACE_CARD)),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: theme::BORDER_SLATE,
                },
                ..Default::default()
            });

            stack![
                main_content,
                opaque(
                    mouse_area(center(opaque(modal_box)).style(|_| container::Style {
                        background: Some(Background::Color(Color {
                            a: 0.85,
                            ..Color::BLACK
                        })),
                        ..Default::default()
                    }))
                    .on_press(Message::DismissModal)
                )
            ]
            .into()
        } else if let Some(player) = &self.playback_session {
            // In-App Player Overlay
            let player_box = container(
                column![
                    row![
                        text(format!("Now Playing: {}", player.file_path().display()))
                            .size(16)
                            .color(theme::ACCENT_WHITE),
                        space::horizontal(),
                        button(text("✕ Close")).on_press(Message::ClosePlayer),
                    ]
                    .align_y(Alignment::Center),
                    container(center(
                        text("▶ Video Stream Active (wgpu render)")
                            .size(16)
                            .color(theme::TEXT_MUTED)
                    ))
                    .width(Fill)
                    .height(360)
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::BLACK)),
                        border: iced::Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: theme::BORDER_SLATE,
                        },
                        ..Default::default()
                    }),
                    row![
                        button(text(if player.is_playing() { "Pause" } else { "Play" }))
                            .on_press(Message::TogglePlayback),
                        text(player.formatted_time())
                            .size(13)
                            .color(theme::ACCENT_WHITE),
                        space::horizontal(),
                        button(text("External Player"))
                            .on_press(Message::PlayEpisode(player.file_path().to_path_buf())),
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center),
                ]
                .spacing(16),
            )
            .width(760)
            .padding(20)
            .style(|_| container::Style {
                background: Some(Background::Color(theme::SURFACE_CARD)),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: theme::BORDER_SLATE,
                },
                ..Default::default()
            });

            stack![
                main_content,
                opaque(
                    mouse_area(center(opaque(player_box)).style(|_| container::Style {
                        background: Some(Background::Color(Color {
                            a: 0.9,
                            ..Color::BLACK
                        })),
                        ..Default::default()
                    }))
                    .on_press(Message::ClosePlayer)
                )
            ]
            .into()
        } else {
            main_content.into()
        }
    }
}
