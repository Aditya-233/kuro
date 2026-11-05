//! Main entrypoint for Kuro (黒) anime application and agent CLI.

#![forbid(unsafe_code)]
#![deny(warnings)]

use clap::Parser;
use kuro::agent::{
    execute_download, execute_episodes, execute_inspect, execute_search, run_agent_diagnostics,
};
use kuro::cli::{Cli, Commands};
use kuro::gui::App;

const FONT_BYTES: &[u8] = include_bytes!("../fonts/LiberationSans-Regular.ttf");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Search { query, json }) => {
            let rt = tokio::runtime::Runtime::new()?;
            if let Err(e) = rt.block_on(execute_search(&query, json)) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Episodes {
            show,
            quality,
            json,
        }) => {
            let rt = tokio::runtime::Runtime::new()?;
            if let Err(e) = rt.block_on(execute_episodes(&show, &quality, json)) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Download {
            show,
            ep,
            quality,
            output_dir,
        }) => {
            let rt = tokio::runtime::Runtime::new()?;
            if let Err(e) = rt.block_on(execute_download(&show, ep, &quality, output_dir)) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Inspect { path, json }) => {
            if let Err(e) = execute_inspect(&path, json) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::AgentTest) => {
            println!("Running Kuro Agent Diagnostics Suite...\n");
            let rt = tokio::runtime::Runtime::new()?;
            let report = rt.block_on(run_agent_diagnostics());

            for check in &report.checks {
                let symbol = if check.passed { "✓ PASS" } else { "✗ FAIL" };
                println!("{:<8} {:<35} : {}", symbol, check.name, check.detail);
            }

            println!(
                "\nOverall Status: {}",
                if report.all_passed() {
                    "ALL SYSTEMS OPERATIONAL"
                } else {
                    "FAILURES DETECTED"
                }
            );

            if !report.all_passed() {
                std::process::exit(1);
            }
        }
        Some(Commands::Gui) | None => {
            let default_font = iced::Font {
                family: iced::font::Family::Name("Liberation Sans"),
                ..iced::Font::DEFAULT
            };

            iced::application(App::new, App::update, App::view)
                .font(FONT_BYTES)
                .default_font(default_font)
                .decorations(false)
                .scale_factor(App::scale_factor)
                .title("Kuro — Anime")
                .theme(App::theme)
                .run()?;
        }
    }

    Ok(())
}
