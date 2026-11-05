//! Iced T3-Code Aesthetic Native Desktop GUI for Kuro.

pub mod theme;

use crate::downloader::{DownloadEngine, DownloadProgressEvent};
use crate::models::{AnimeShow, DownloadStatus, DownloadTask, Episode, LocalVideoFile, Quality};
use crate::player::PlaybackSession;
use crate::sources::AnimeSourceClient;
use iced::widget::{
    button, center, column, container, image, mouse_area, opaque, row, scrollable, space, stack,
    text, text_input,
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
    /// Local downloaded video files in ~/Downloads.
    pub local_files: Vec<LocalVideoFile>,
    /// Search in-flight flag.
    pub is_searching: bool,
    /// Automatically open the episodes modal for the first search result.
    pub auto_select_first: bool,
    /// Active in-app playback session.
    pub playback_session: Option<PlaybackSession>,
    /// Status message or error banner.
    pub status_message: Option<String>,
    /// Whether automated UI tour is active.
    pub is_tour: bool,
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
    /// Seek relative in seconds (+/- seconds).
    SeekRelative(f64),
    /// Native frame extracted from video file.
    FrameExtracted(Result<Vec<u8>, String>),
    /// Periodic tick for playback.
    PlayerTick,
    /// Launch external native Wayland player via MPV.
    LaunchNativePopout,
    /// Automated walkthrough step trigger.
    TourStep(u32),
    /// Dismiss the playback overlay.
    ClosePlayer,
}

fn capture_tour_snap(name: &str) {
    let path = format!(
        "/home/aditya/.gemini/antigravity-cli/brain/76542c33-9796-42c0-9b04-6b1293c571b3/walkthrough/{name}"
    );
    let _ = std::process::Command::new("grim")
        .env("WAYLAND_DISPLAY", "wayland-0")
        .env("XDG_RUNTIME_DIR", "/run/user/1000")
        .arg(&path)
        .output();
}

impl App {
    /// Create a new Kuro GUI application instance.
    #[must_use]
    pub fn new() -> Self {
        let dl_dir = directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("/home/aditya/Downloads"));
        let local_files = crate::sources::scan_local_downloads(&dl_dir);

        Self {
            search_query: String::new(),
            shows: Vec::new(),
            selected_show: None,
            episodes: Vec::new(),
            active_downloads: Vec::new(),
            local_files,
            is_searching: false,
            auto_select_first: false,
            playback_session: None,
            status_message: None,
            is_tour: false,
            source_client: Arc::new(AnimeSourceClient::new()),
            download_engine: Arc::new(DownloadEngine::new()),
        }
    }

    /// Check whether an episode is already downloaded in ~/Downloads.
    #[must_use]
    pub fn is_file_downloaded(&self, ep_title: &str) -> Option<PathBuf> {
        let dl_dir = directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("/home/aditya/Downloads"));
        crate::sources::is_episode_downloaded(ep_title, &dl_dir)
    }

    /// Refresh list of locally downloaded files.
    pub fn refresh_local_files(&mut self) {
        let dl_dir = directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("/home/aditya/Downloads"));
        self.local_files = crate::sources::scan_local_downloads(&dl_dir);
    }

    /// Create an instance with optional initial search query, auto-select, immediate play file, or automated tour.
    pub fn with_initial_options(
        query: Option<String>,
        select_first: bool,
        play: Option<PathBuf>,
        tour: bool,
    ) -> (Self, Task<Message>) {
        let mut app = Self::new();
        app.auto_select_first = select_first;
        app.is_tour = tour;

        if tour {
            let task = Task::perform(
                async {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                },
                |()| Message::TourStep(1),
            );
            return (app, task);
        }

        if let Some(path) = play {
            let task = app.update(Message::PlayEpisode(path));
            return (app, task);
        }

        if let Some(q) = query {
            let _ = app.update(Message::SearchInputChanged(q));
            let task = app.update(Message::SearchSubmitted);
            (app, task)
        } else {
            (app, Task::none())
        }
    }

    /// Backwards compatible wrapper for `with_initial_query`.
    pub fn with_initial_query(query: Option<String>, select_first: bool) -> (Self, Task<Message>) {
        Self::with_initial_options(query, select_first, None, false)
    }

    /// Theme specification using T3-Code aesthetic palette.
    #[must_use]
    pub fn theme(&self) -> Theme {
        Theme::custom("T3 Dark", theme::custom_t3_palette())
    }

    /// UI Scale factor (2.0x scaling).
    #[must_use]
    pub fn scale_factor(&self) -> f32 {
        2.0
    }

    /// Background event subscriptions (e.g. playback ticking).
    pub fn subscription(&self) -> iced::Subscription<Message> {
        if let Some(session) = &self.playback_session {
            if session.is_playing() {
                return iced::time::every(std::time::Duration::from_millis(1500))
                    .map(|_| Message::PlayerTick);
            }
        }
        iced::Subscription::none()
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
                self.refresh_local_files();
                match result {
                    Ok(shows) => {
                        self.status_message = Some(format!("Found {} shows", shows.len()));
                        self.shows = shows;
                        if self.is_tour && !self.shows.is_empty() {
                            return Task::perform(
                                async {
                                    tokio::time::sleep(std::time::Duration::from_millis(1200))
                                        .await;
                                },
                                |()| Message::TourStep(3),
                            );
                        }
                        if self.auto_select_first && !self.shows.is_empty() {
                            self.auto_select_first = false;
                            return self.update(Message::SelectShow(self.shows[0].clone()));
                        }
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Error: {e}"));
                        self.shows.clear();
                    }
                }
                Task::none()
            }
            Message::SelectShow(show) => {
                self.refresh_local_files();
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
                        if self.is_tour && !self.episodes.is_empty() {
                            return Task::perform(
                                async {
                                    tokio::time::sleep(std::time::Duration::from_millis(1500))
                                        .await;
                                },
                                |()| Message::TourStep(4),
                            );
                        }
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
                    .unwrap_or_else(|| PathBuf::from("/home/aditya/Downloads"));

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
                let episode = ep.clone();
                let dest = dest_path;

                let (tx, rx) = mpsc::channel(16);

                tokio::spawn(async move {
                    let _ = engine.download_episode(&task_id, &episode, &dest, tx).await;
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
                        self.refresh_local_files();
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
                let mut session = PlaybackSession::new(path.clone(), 0.0);
                session.play();
                self.playback_session = Some(session);
                let p = path;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            PlaybackSession::extract_frame(&p, 0.0).map_err(|e| e.to_string())
                        })
                        .await
                        .map_err(|e| e.to_string())?
                    },
                    Message::FrameExtracted,
                )
            }
            Message::TogglePlayback => {
                if let Some(session) = &mut self.playback_session {
                    session.toggle();
                    if session.is_playing() && session.current_frame().is_none() {
                        let path = session.file_path().to_path_buf();
                        let pos = session.position_seconds();
                        return Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || {
                                    PlaybackSession::extract_frame(&path, pos)
                                        .map_err(|e| e.to_string())
                                })
                                .await
                                .map_err(|e| e.to_string())?
                            },
                            Message::FrameExtracted,
                        );
                    }
                }
                Task::none()
            }
            Message::SeekRelative(delta) => {
                if let Some(session) = &mut self.playback_session {
                    session.seek_by(delta);
                    let path = session.file_path().to_path_buf();
                    let pos = session.position_seconds();
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                PlaybackSession::extract_frame(&path, pos)
                                    .map_err(|e| e.to_string())
                            })
                            .await
                            .map_err(|e| e.to_string())?
                        },
                        Message::FrameExtracted,
                    );
                }
                Task::none()
            }
            Message::FrameExtracted(result) => {
                if let Ok(frame) = result {
                    if let Some(session) = &mut self.playback_session {
                        session.set_frame(frame);
                    }
                }
                Task::none()
            }
            Message::PlayerTick => {
                if let Some(session) = &mut self.playback_session {
                    if session.is_playing() {
                        session.seek_by(1.5);
                        let path = session.file_path().to_path_buf();
                        let pos = session.position_seconds();
                        return Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || {
                                    PlaybackSession::extract_frame(&path, pos)
                                        .map_err(|e| e.to_string())
                                })
                                .await
                                .map_err(|e| e.to_string())?
                            },
                            Message::FrameExtracted,
                        );
                    }
                }
                Task::none()
            }
            Message::LaunchNativePopout => {
                if let Some(session) = &self.playback_session {
                    let _ = session.launch_native_player();
                }
                Task::none()
            }
            Message::ClosePlayer => {
                self.playback_session = None;
                Task::none()
            }
            Message::TourStep(1) => {
                capture_tour_snap("01_initial_screen.png");
                self.search_query = "Sousou no Frieren".to_string();
                Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    },
                    |()| Message::TourStep(2),
                )
            }
            Message::TourStep(2) => {
                capture_tour_snap("02_typed_search.png");
                self.update(Message::SearchSubmitted)
            }
            Message::TourStep(3) => {
                capture_tour_snap("03_search_results.png");
                let target_show = self
                    .shows
                    .iter()
                    .find(|s| s.title.contains("S2"))
                    .or_else(|| self.shows.first())
                    .cloned();
                if let Some(show) = target_show {
                    self.update(Message::SelectShow(show))
                } else {
                    Task::none()
                }
            }
            Message::TourStep(4) => {
                capture_tour_snap("04_episodes_modal.png");
                let target_ep = self
                    .episodes
                    .iter()
                    .find(|e| e.episode_number == 2)
                    .or_else(|| self.episodes.get(1))
                    .cloned();
                let dl_task = if let Some(ep) = target_ep {
                    self.update(Message::TriggerDownload(ep))
                } else {
                    Task::none()
                };
                let next_task = Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                    },
                    |()| Message::TourStep(5),
                );
                Task::batch([dl_task, next_task])
            }
            Message::TourStep(5) => {
                capture_tour_snap("05_download_progress.png");
                let dismiss_task = self.update(Message::DismissModal);
                let next_task = Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                    },
                    |()| Message::TourStep(6),
                );
                Task::batch([dismiss_task, next_task])
            }
            Message::TourStep(6) => {
                capture_tour_snap("06_modal_closed.png");
                let target = PathBuf::from(
                    "/home/aditya/Downloads/[SubsPlease] Sousou no Frieren S2 - 01 (1080p) [4277EF46].mkv",
                );
                let play_task = self.update(Message::PlayEpisode(target));
                let next_task = Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
                    },
                    |()| Message::TourStep(7),
                );
                Task::batch([play_task, next_task])
            }
            Message::TourStep(7) => {
                capture_tour_snap("07_player_in_app.png");
                let pause_task = self.update(Message::TogglePlayback);
                let next_task = Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    },
                    |()| Message::TourStep(8),
                );
                Task::batch([pause_task, next_task])
            }
            Message::TourStep(8) => {
                capture_tour_snap("08_player_paused.png");
                let seek_task = self.update(Message::SeekRelative(10.0));
                let next_task = Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
                    },
                    |()| Message::TourStep(9),
                );
                Task::batch([seek_task, next_task])
            }
            Message::TourStep(9) => {
                capture_tour_snap("09_player_seeked.png");
                let close_task = self.update(Message::ClosePlayer);
                let next_task = Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    },
                    |()| Message::TourStep(10),
                );
                Task::batch([close_task, next_task])
            }
            Message::TourStep(10) => {
                capture_tour_snap("10_player_closed.png");
                std::process::exit(0);
            }
            Message::TourStep(_) => Task::none(),
        }
    }

    /// Elm Architecture view layout.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn view(&self) -> Element<'_, Message> {
        let brand_badge = container(text("ANIME").size(10).color(theme::ACCENT_BLUE))
            .padding([2, 6])
            .style(|_| theme::badge_style(true));

        let brand = row![
            text("KURO").size(22).color(theme::TEXT_PRIMARY),
            brand_badge,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let search_bar = row![
            text_input(
                "Search anime (e.g. Solo Leveling, Attack on Titan, Quintuplets)...",
                &self.search_query
            )
            .on_input(Message::SearchInputChanged)
            .on_submit(Message::SearchSubmitted)
            .padding([8, 14])
            .width(420)
            .style(|_, s| theme::search_input_style(s)),
            button(
                text(if self.is_searching {
                    "Searching..."
                } else {
                    "Search"
                })
                .size(13)
                .color(theme::TEXT_PRIMARY)
            )
            .on_press(Message::SearchSubmitted)
            .padding([8, 18])
            .style(|_, s| theme::button_primary_style(s)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let header = row![brand, space::horizontal(), search_bar]
            .spacing(12)
            .align_y(Alignment::Center);

        let status_bar = if let Some(msg) = &self.status_message {
            row![text(msg).size(13).color(theme::TEXT_MUTED)]
        } else {
            row![
                text("Search for any anime series above to view episodes and direct downloads.")
                    .size(13)
                    .color(theme::TEXT_MUTED)
            ]
        };

        // Shows Grid / Cards
        let mut cards_col = column![].spacing(16);
        let mut current_row = row![].spacing(16);

        for (i, show) in self.shows.iter().enumerate() {
            let mut title_col =
                column![text(&show.title).size(15).color(theme::TEXT_PRIMARY)].spacing(2);
            if let Some(eng) = &show.english_title {
                if !eng.eq_ignore_ascii_case(&show.title) {
                    title_col = title_col.push(text(eng).size(12).color(theme::TEXT_MUTED));
                }
            }

            let card_inner = column![
                title_col,
                row![
                    container(
                        text(format!("{} Episodes", show.episode_count))
                            .size(11)
                            .color(theme::ACCENT_BLUE)
                    )
                    .padding([2, 8])
                    .style(|_| theme::badge_style(true)),
                    container(text("1080p").size(11).color(theme::TEXT_MUTED))
                        .padding([2, 8])
                        .style(|_| theme::badge_style(false)),
                    container(text("Direct CDN").size(11).color(theme::ACCENT_SUCCESS))
                        .padding([2, 8])
                        .style(|_| theme::badge_style(false)),
                ]
                .spacing(8),
                space::vertical(),
                row![
                    text("Click to browse episodes")
                        .size(11)
                        .color(theme::TEXT_MUTED),
                    space::horizontal(),
                    button(text("Episodes").size(12).color(theme::TEXT_PRIMARY))
                        .on_press(Message::SelectShow(show.clone()))
                        .padding([4, 12])
                        .style(|_, s| theme::button_secondary_style(s)),
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(10);

            let card = mouse_area(
                container(card_inner)
                    .width(360)
                    .height(145)
                    .padding(16)
                    .style(|_| theme::card_style()),
            )
            .on_press(Message::SelectShow(show.clone()));

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
                .push(text("Active Downloads").size(14).color(theme::TEXT_PRIMARY));
            for task in &self.active_downloads {
                let item = row![
                    text(&task.file_name)
                        .size(13)
                        .color(theme::TEXT_PRIMARY)
                        .width(350),
                    text(format!("{}%", task.progress_percent()))
                        .size(13)
                        .color(theme::ACCENT_BLUE),
                    text(task.formatted_speed())
                        .size(12)
                        .color(theme::TEXT_MUTED),
                    text(format!("ETA: {}", task.formatted_eta()))
                        .size(12)
                        .color(theme::TEXT_MUTED),
                    space::horizontal(),
                    if task.status == DownloadStatus::Completed {
                        button(text("▶ Play").size(12).color(theme::TEXT_PRIMARY))
                            .on_press(Message::PlayEpisode(PathBuf::from(&task.destination_path)))
                            .padding([4, 12])
                            .style(|_, s| theme::button_primary_style(s))
                    } else {
                        button(text("Downloading...").size(12).color(theme::ACCENT_BLUE))
                            .padding([4, 10])
                            .style(|_, s| theme::button_secondary_style(s))
                    },
                ]
                .spacing(16)
                .align_y(Alignment::Center);

                downloads_section = downloads_section.push(item);
            }
        }

        // Local Downloaded Videos Panel (~/Downloads)
        let mut local_downloads_section = column![].spacing(8);
        if !self.local_files.is_empty() {
            local_downloads_section = local_downloads_section.push(
                row![
                    text("Downloaded Videos (~/Downloads)")
                        .size(14)
                        .color(theme::TEXT_PRIMARY),
                    container(
                        text(format!("{} files", self.local_files.len()))
                            .size(10)
                            .color(theme::ACCENT_BLUE)
                    )
                    .padding([2, 6])
                    .style(|_| theme::badge_style(true)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );

            for f in &self.local_files {
                let f_item = row![
                    text("▶").size(11).color(theme::ACCENT_BLUE),
                    text(&f.filename)
                        .size(13)
                        .color(theme::TEXT_PRIMARY)
                        .width(Length::FillPortion(4)),
                    container(
                        text(format!("{} MB", f.size_mb))
                            .size(11)
                            .color(theme::TEXT_MUTED)
                    )
                    .padding([2, 8])
                    .style(|_| theme::badge_style(false)),
                    space::horizontal(),
                    button(text("▶ Play").size(12).color(theme::TEXT_PRIMARY))
                        .on_press(Message::PlayEpisode(f.path.clone()))
                        .padding([4, 14])
                        .style(|_, s| theme::button_primary_style(s)),
                ]
                .spacing(12)
                .align_y(Alignment::Center);

                local_downloads_section = local_downloads_section.push(f_item);
            }
        }

        let main_content = container(
            column![
                header,
                status_bar,
                scrollable(cards_col).height(Fill),
                downloads_section,
                local_downloads_section,
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
                let downloaded_path = self.is_file_downloaded(&ep.title);
                let is_downloading = self
                    .active_downloads
                    .iter()
                    .any(|t| t.file_name == ep.title && t.status == DownloadStatus::Downloading);

                let action_element = if let Some(path) = downloaded_path {
                    row![
                        container(
                            text("✓ In ~/Downloads")
                                .size(11)
                                .color(theme::ACCENT_SUCCESS)
                        )
                        .padding([3, 8])
                        .style(|_| theme::badge_style(false)),
                        button(text("▶ Play").size(12).color(theme::TEXT_PRIMARY))
                            .on_press(Message::PlayEpisode(path))
                            .padding([4, 14])
                            .style(|_, s| theme::button_primary_style(s)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                } else if is_downloading {
                    let pct = self
                        .active_downloads
                        .iter()
                        .find(|t| t.file_name == ep.title)
                        .map_or(0, DownloadTask::progress_percent);
                    row![button(
                        text(format!("Downloading {pct}%"))
                            .size(12)
                            .color(theme::ACCENT_BLUE)
                    )
                    .padding([4, 10])
                    .style(|_, s| theme::button_secondary_style(s)),]
                    .align_y(Alignment::Center)
                } else {
                    row![button(text("Download").size(12).color(theme::TEXT_PRIMARY))
                        .on_press(Message::TriggerDownload(ep.clone()))
                        .padding([4, 14])
                        .style(|_, s| theme::button_primary_style(s)),]
                    .align_y(Alignment::Center)
                };

                let ep_row = row![
                    container(
                        text(format!("EP {:02}", ep.episode_number))
                            .size(11)
                            .color(theme::ACCENT_BLUE)
                    )
                    .padding([3, 8])
                    .style(|_| theme::badge_style(true)),
                    text(&ep.title)
                        .size(13)
                        .color(theme::TEXT_PRIMARY)
                        .width(Length::FillPortion(3)),
                    container(text(ep.quality.as_str()).size(11).color(theme::TEXT_MUTED))
                        .padding([3, 8])
                        .style(|_| theme::badge_style(false)),
                    container(text(ep.formatted_size()).size(11).color(theme::TEXT_MUTED))
                        .padding([3, 8])
                        .style(|_| theme::badge_style(false)),
                    action_element,
                ]
                .spacing(12)
                .align_y(Alignment::Center);

                ep_list = ep_list.push(ep_row);
            }

            let modal_box = container(
                column![
                    row![
                        text(&show.title).size(18).color(theme::TEXT_PRIMARY),
                        space::horizontal(),
                        button(text("✕ Close").size(12).color(theme::TEXT_PRIMARY))
                            .on_press(Message::DismissModal)
                            .padding([4, 12])
                            .style(|_, s| theme::button_secondary_style(s)),
                    ]
                    .align_y(Alignment::Center),
                    scrollable(container(ep_list).padding(iced::Padding {
                        top: 0.0,
                        right: 24.0,
                        bottom: 0.0,
                        left: 0.0,
                    }),)
                    .height(420),
                ]
                .spacing(16),
            )
            .width(860)
            .padding(24)
            .style(|_| container::Style {
                background: Some(Background::Color(theme::SURFACE)),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: theme::BORDER_HIGHLIGHT,
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
            let file_title = player
                .file_path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Playing Video");

            let video_viewport: Element<Message> = if let Some(frame) = player.current_frame() {
                container(
                    image(iced::widget::image::Handle::from_bytes(frame.to_vec()))
                        .width(Fill)
                        .height(Fill),
                )
                .width(Fill)
                .height(400)
                .center_x(Fill)
                .center_y(Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::BLACK)),
                    border: iced::Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: theme::BORDER,
                    },
                    ..Default::default()
                })
                .into()
            } else {
                container(center(
                    column![
                        text("▶ Loading In-App Frame Stream...")
                            .size(14)
                            .color(theme::TEXT_MUTED),
                        text("Native zero-dependency video decoder")
                            .size(11)
                            .color(theme::TEXT_MUTED),
                    ]
                    .spacing(8)
                    .align_x(Alignment::Center),
                ))
                .width(Fill)
                .height(400)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::BLACK)),
                    border: iced::Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: theme::BORDER,
                    },
                    ..Default::default()
                })
                .into()
            };

            let player_box = container(
                column![
                    row![
                        text(format!("▶ Now Playing: {file_title}"))
                            .size(15)
                            .color(theme::TEXT_PRIMARY)
                            .width(Length::FillPortion(1)),
                        space::horizontal(),
                        button(text("✕ Close").size(12).color(theme::TEXT_PRIMARY))
                            .on_press(Message::ClosePlayer)
                            .padding([6, 16])
                            .style(|_, s| theme::button_secondary_style(s)),
                    ]
                    .align_y(Alignment::Center),
                    video_viewport,
                    row![
                        button(
                            text(if player.is_playing() {
                                "❚❚ Pause"
                            } else {
                                "▶ Play"
                            })
                            .size(12)
                            .color(theme::TEXT_PRIMARY)
                        )
                        .on_press(Message::TogglePlayback)
                        .padding([6, 16])
                        .style(|_, s| theme::button_primary_style(s)),
                        button(text("⏪ -10s").size(12).color(theme::TEXT_PRIMARY))
                            .on_press(Message::SeekRelative(-10.0))
                            .padding([6, 12])
                            .style(|_, s| theme::button_secondary_style(s)),
                        button(text("⏩ +10s").size(12).color(theme::TEXT_PRIMARY))
                            .on_press(Message::SeekRelative(10.0))
                            .padding([6, 12])
                            .style(|_, s| theme::button_secondary_style(s)),
                        text(player.formatted_time())
                            .size(13)
                            .color(theme::TEXT_PRIMARY),
                        space::horizontal(),
                        button(text("⛶ Popout Player").size(12).color(theme::ACCENT_BLUE))
                            .on_press(Message::LaunchNativePopout)
                            .padding([6, 14])
                            .style(|_, s| theme::button_secondary_style(s)),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                ]
                .spacing(16),
            )
            .width(860)
            .padding(24)
            .style(|_| container::Style {
                background: Some(Background::Color(theme::SURFACE)),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: theme::BORDER_HIGHLIGHT,
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
