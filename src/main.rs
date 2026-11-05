//! Main entrypoint for Kuro (黒) anime application and agent CLI.

#![forbid(unsafe_code)]
#![deny(warnings)]

use clap::Parser;
use kuro::agent::{
    execute_download, execute_episodes, execute_inspect, execute_search, run_agent_diagnostics,
};
use kuro::cli::{Cli, Commands};
use kuro::gui::App;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Search { query, json }) => {
            if let Err(e) = execute_search(&query, json).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Episodes {
            show,
            quality,
            json,
        }) => {
            if let Err(e) = execute_episodes(&show, &quality, json).await {
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
            if let Err(e) = execute_download(&show, ep, &quality, output_dir).await {
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
            let report = run_agent_diagnostics().await;

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
            iced::application(App::new, App::update, App::view)
                .title("Kuro / 黒")
                .theme(App::theme)
                .run()?;
        }
    }

    Ok(())
}
