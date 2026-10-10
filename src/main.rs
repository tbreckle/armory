//! A.R.M.O.R.Y. (Arcade Runner Menu On Rails, Yeah!): a light gun friendly launcher for arcade
//! games and tools.

// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod launch;
mod layout;
mod monitor;
mod status;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

/// Version injected by build.rs (`ARMORY_VERSION`, see scripts/version.sh).
const VERSION: &str = env!("ARMORY_VERSION");

const USAGE: &str = "\
Usage: armory [--config <file>]

Options:
  -c, --config <file>  Configuration file. Default: armory.yaml next to the executable,
                       else armory/armory.yaml in the user's config directory.
  -h, --help           Show this help.
  -V, --version        Show the version.";

fn main() -> ExitCode {
    let mut config = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-c" | "--config") => match args.next() {
                Some(path) => config = Some(PathBuf::from(path)),
                None => return usage_error("--config needs a file"),
            },
            Some("-h" | "--help") => {
                println!("A.R.M.O.R.Y. {VERSION}\n\n{USAGE}");
                return ExitCode::SUCCESS;
            }
            Some("-V" | "--version") => {
                println!("armory {VERSION}");
                return ExitCode::SUCCESS;
            }
            _ => return usage_error(&format!("unknown argument {}", arg.to_string_lossy())),
        }
    }

    match ui::run(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("armory: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("armory: {message}\n\n{USAGE}");
    ExitCode::from(2)
}
