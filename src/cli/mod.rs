//! Command line interface and agent automation commands.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Kuro (黒) — Minimalist monochrome native anime desktop app and CLI engine.
#[derive(Parser, Debug)]
#[command(name = "kuro", author, version, about, long_about = None)]
pub struct Cli {
    /// Subcommand to execute. If omitted, launches the Iced monochrome GUI.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available CLI and agent commands.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Commands {
    /// Search for anime shows by title or alias.
    Search {
        /// Search keyword (e.g. "Solo Leveling", "The Quintessential Quintuplets", "Attack on Titan").
        query: String,
        /// Output results in JSON format for automated agent parsing.
        #[arg(long, short)]
        json: bool,
    },
    /// List available episodes and direct download sources for a show.
    Episodes {
        /// Show name or slug.
        show: String,
        /// Preferred quality (e.g. "1080p", "720p", "480p").
        #[arg(long, short, default_value = "1080p")]
        quality: String,
        /// Output episodes in JSON format for automated agent parsing.
        #[arg(long, short)]
        json: bool,
    },
    /// Directly download an episode to disk.
    Download {
        /// Show name or slug.
        show: String,
        /// Numeric episode number.
        #[arg(long, short)]
        ep: u32,
        /// Preferred quality.
        #[arg(long, short, default_value = "1080p")]
        quality: String,
        /// Destination directory (defaults to ~/Downloads).
        #[arg(long, short)]
        output_dir: Option<PathBuf>,
    },
    /// Inspect a downloaded media file on disk.
    Inspect {
        /// File path to inspect.
        path: PathBuf,
        /// Output file metadata in JSON format.
        #[arg(long, short)]
        json: bool,
    },
    /// Run automated agent diagnostics test suite.
    AgentTest,
    /// Explicitly launch the Iced monochrome GUI.
    Gui {
        /// Optional search query to trigger on launch.
        #[arg(long, short)]
        query: Option<String>,
        /// Automatically open the episodes modal for the first search result.
        #[arg(long)]
        select_first: bool,
        /// Direct file path to open in the native player immediately.
        #[arg(long)]
        play: Option<PathBuf>,
    },
}
