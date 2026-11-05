use clap::Parser;
use kuro::cli::{Cli, Commands};

#[test]
fn test_cli_parsing() {
    let args = vec!["kuro", "search", "Solo Leveling", "--json"];
    let cli = Cli::try_parse_from(args).expect("Failed to parse search CLI");
    assert!(
        matches!(cli.command, Some(Commands::Search { ref query, json: true }) if query == "Solo Leveling")
    );

    let ep_args = vec!["kuro", "episodes", "Solo Leveling", "--quality", "1080p"];
    let ep_cli = Cli::try_parse_from(ep_args).expect("Failed to parse episodes CLI");
    assert!(
        matches!(ep_cli.command, Some(Commands::Episodes { ref show, ref quality, json: false }) if show == "Solo Leveling" && quality == "1080p")
    );

    let default_gui = vec!["kuro"];
    let gui_cli = Cli::try_parse_from(default_gui).expect("Failed to parse default CLI");
    assert!(gui_cli.command.is_none());
}
