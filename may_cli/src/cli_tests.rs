use super::{Cli, Commands};
use clap::{CommandFactory, Parser};

#[test]
fn verify_cli_structure() {
    // clap's built-in assertion to ensure our CLI configuration is valid
    // (no overlapping flags, correct types, etc.)
    Cli::command().debug_assert();
}

#[test]
fn test_parse_validate_command() {
    let args = vec!["may", "validate", "--path", "./demos/metric_demo"];
    let cli = Cli::try_parse_from(args).unwrap();

    match cli.command {
        Some(Commands::Validate { path }) => {
            assert_eq!(path, Some("./demos/metric_demo".to_string()));
        }
        other => panic!("Expected Validate command, got: {other:?}"),
    }
}

#[test]
fn test_parse_compile_command_no_path() {
    let args = vec!["may", "compile"];
    let cli = Cli::try_parse_from(args).unwrap();

    match cli.command {
        Some(Commands::Compile { path }) => {
            assert_eq!(path, None);
        }
        other => panic!("Expected Compile command, got: {other:?}"),
    }
}

#[test]
fn test_parse_run_command() {
    let args = vec!["may", "run", "--query", "Revenue by Region"];
    let cli = Cli::try_parse_from(args).unwrap();

    match cli.command {
        Some(Commands::Run { query }) => {
            assert_eq!(query, "Revenue by Region");
        }
        other => panic!("Expected Run command, got: {other:?}"),
    }
}
