//! launchbox-images: downloads clear logos of games from the LaunchBox Games Database as button
//! images for A.R.M.O.R.Y.
//!
//! LaunchBox publishes its whole database as one archive (Metadata.zip, about 100 MB). It is
//! downloaded once and turned into a small index in the user's cache directory, searches run
//! offline against that index, and only the chosen logo itself is downloaded.

mod index;
mod logo;
mod net;
mod search;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use index::{Game, Index};

/// Version injected by build.rs (`ARMORY_VERSION`, see scripts/version.sh).
const VERSION: &str = env!("ARMORY_VERSION");

const USAGE: &str = "\
Usage: launchbox-images [--cache-dir <dir>] <command>

Commands:
  search <title>...   List games whose title contains all these words, with their IDs.
    -p, --platform <text>  Only platforms containing this text, e.g. arcade.
    -a, --all              Also games without a clear logo.
    -n, --limit <n>        Show at most n games. Default: 50.
  get <id>            Save the clear logo of a game as <name>.png, the name in lowercase and
                      other characters than letters and digits replaced by '-'.
    -o, --out <dir>        Target folder. Default: images in the current folder.
    -r, --region <region>  Preferred region, e.g. Japan. Default: no region, World, North
                           America, United States, Europe, United Kingdom, others.
        --name <name>      File name instead of the game's name.
    -f, --force            Replace an existing file.
  update              Download the database again and rebuild the index.

Options:
      --cache-dir <dir>  Index location. Default: armory/launchbox in the user's cache folder.
  -h, --help             Show this help.
  -V, --version          Show the version.

The first search or get downloads the LaunchBox database (about 100 MB).";

#[derive(Debug, PartialEq)]
enum Command {
    Search {
        title: String,
        platform: String,
        all: bool,
        limit: usize,
    },
    Get {
        id: u64,
        out: PathBuf,
        region: String,
        name: String,
        force: bool,
    },
    Update,
    Help,
    Version,
}

#[derive(Debug, PartialEq)]
struct Args {
    cache_dir: Option<PathBuf>,
    command: Command,
}

fn main() -> ExitCode {
    let args = match parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("launchbox-images: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("launchbox-images: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> anyhow::Result<ExitCode> {
    let cache = args.cache_dir.unwrap_or_else(index::default_cache_dir);
    match args.command {
        Command::Help => println!("launchbox-images {VERSION}\n\n{USAGE}"),
        Command::Version => println!("launchbox-images {VERSION}"),
        Command::Update => {
            index::update(&cache)?;
        }
        Command::Search {
            title,
            platform,
            all,
            limit,
        } => {
            let index = index::load(&cache)?;
            let hits = search::search(&index, &title, &platform, !all)?;
            if hits.is_empty() {
                eprintln!("No games found.");
                return Ok(ExitCode::FAILURE);
            }
            print!("{}", table(&index, &hits[..hits.len().min(limit)], all));
            if hits.len() > limit {
                println!(
                    "... {} more, narrow the search or use --limit.",
                    hits.len() - limit
                );
            }
        }
        Command::Get {
            id,
            out,
            region,
            name,
            force,
        } => {
            let index = index::load(&cache)?;
            let request = logo::Request {
                id,
                out_dir: &out,
                region: &region,
                name: &name,
                force,
            };
            let target = logo::get(&index, &request, net::fetch)?;
            println!("Saved {}", target.display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, String> {
    let mut args = args.into_iter();
    let mut cache_dir = None;
    let mut command = None;
    let mut words: Vec<String> = Vec::new();
    let (mut platform, mut region, mut name) = (String::new(), String::new(), String::new());
    let (mut all, mut force, mut limit, mut out) = (false, false, 50, PathBuf::from("images"));

    while let Some(arg) = args.next() {
        let Some(arg) = arg.to_str().map(str::to_owned) else {
            return Err(format!("invalid argument {}", arg.to_string_lossy()));
        };
        let mut value = |option: &str| match args.next() {
            Some(v) => v
                .into_string()
                .map_err(|v| format!("invalid {option} {v:?}")),
            None => Err(format!("{option} needs a value")),
        };
        match (command.as_deref(), arg.as_str()) {
            (_, "-h" | "--help") => {
                return Ok(Args {
                    cache_dir,
                    command: Command::Help,
                });
            }
            (_, "-V" | "--version") => {
                return Ok(Args {
                    cache_dir,
                    command: Command::Version,
                });
            }
            (_, "--cache-dir") => cache_dir = Some(PathBuf::from(value("--cache-dir")?)),
            (None, "search" | "get" | "update") => command = Some(arg),
            (Some("search"), "-p" | "--platform") => platform = value("--platform")?,
            (Some("search"), "-a" | "--all") => all = true,
            (Some("search"), "-n" | "--limit") => {
                limit = value("--limit")?
                    .parse()
                    .map_err(|_| "--limit needs a number".to_string())?;
            }
            (Some("get"), "-o" | "--out") => out = PathBuf::from(value("--out")?),
            (Some("get"), "-r" | "--region") => region = value("--region")?,
            (Some("get"), "--name") => name = value("--name")?,
            (Some("get"), "-f" | "--force") => force = true,
            (_, option) if option.starts_with('-') && option.len() > 1 => {
                return Err(format!("unknown option {option}"));
            }
            (Some("search" | "get"), _) => words.push(arg),
            (None, _) => return Err(format!("unknown command {arg}")),
            (Some(_), _) => return Err(format!("unexpected argument {arg}")),
        }
    }

    let command = match command.as_deref() {
        None => return Err("a command is needed".into()),
        Some("search") if words.is_empty() => return Err("search needs a title".into()),
        Some("search") => Command::Search {
            title: words.join(" "),
            platform,
            all,
            limit,
        },
        Some("get") => match words.as_slice() {
            [id] => Command::Get {
                id: id.parse().map_err(|_| format!("'{id}' is not a game ID"))?,
                out,
                region,
                name,
                force,
            },
            _ => return Err("get needs exactly one game ID".into()),
        },
        _ => Command::Update,
    };
    Ok(Args { cache_dir, command })
}

/// Aligned columns ID, year, platform (cut at 32 characters), [logo,] title.
fn table(index: &Index, games: &[&Game], show_logo: bool) -> String {
    let mut rows = vec![["ID", "Year", "Platform", "Logo", "Title"].map(String::from)];
    for game in games {
        let logo = if index.logos.contains_key(&game.id) {
            "yes"
        } else {
            "no"
        };
        rows.push([
            game.id.to_string(),
            game.year.clone(),
            game.platform.chars().take(32).collect(),
            logo.to_string(),
            game.name.clone(),
        ]);
    }
    let width = |col: usize| {
        rows.iter()
            .map(|r| r[col].chars().count())
            .max()
            .unwrap_or(0)
    };
    let (w_id, w_year, w_platform, w_logo) = (width(0), width(1), width(2), width(3));
    let mut out = String::new();
    for [id, year, platform, logo, name] in &rows {
        out += &format!("{id:>w_id$}  {year:w_year$}  {platform:w_platform$}  ");
        if show_logo {
            out += &format!("{logo:w_logo$}  ");
        }
        out += name;
        out += "\n";
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse(list.iter().map(OsString::from))
    }

    fn command(list: &[&str]) -> Command {
        args(list).unwrap().command
    }

    #[test]
    fn search_options() {
        assert_eq!(
            command(&[
                "search", "time", "-p", "arcade", "crisis", "--all", "-n", "5"
            ]),
            Command::Search {
                title: "time crisis".into(),
                platform: "arcade".into(),
                all: true,
                limit: 5,
            }
        );
    }

    #[test]
    fn get_options_and_defaults() {
        assert_eq!(
            command(&["get", "42"]),
            Command::Get {
                id: 42,
                out: PathBuf::from("images"),
                region: String::new(),
                name: String::new(),
                force: false,
            }
        );
        assert_eq!(
            command(&["get", "-f", "--name", "TC", "42", "-r", "Japan", "-o", "x"]),
            Command::Get {
                id: 42,
                out: PathBuf::from("x"),
                region: "Japan".into(),
                name: "TC".into(),
                force: true,
            }
        );
    }

    #[test]
    fn cache_dir_anywhere() {
        let parsed = args(&["update", "--cache-dir", "/tmp/c"]).unwrap();
        assert_eq!(parsed.cache_dir, Some(PathBuf::from("/tmp/c")));
        assert_eq!(parsed.command, Command::Update);
    }

    #[test]
    fn help_and_version() {
        assert_eq!(command(&["--help"]), Command::Help);
        assert_eq!(command(&["get", "-V"]), Command::Version);
    }

    #[test]
    fn errors() {
        for list in [
            &[][..],
            &["find"],
            &["search"],
            &["get"],
            &["get", "1", "2"],
            &["get", "abc"],
            &["get", "1", "--all"],
            &["search", "x", "-n"],
            &["search", "x", "-n", "many"],
            &["update", "extra"],
        ] {
            assert!(args(list).is_err(), "{list:?} should fail");
        }
    }

    #[test]
    fn table_columns() {
        let index = index::tests::fixture();
        let games: Vec<&Game> = index.games.iter().take(4).collect();
        assert_eq!(
            table(&index, &games, true),
            "\
ID  Year  Platform          Logo  Title
 1  1995  Arcade            yes   Time Crisis
 2  2001  Sony Playstation  yes   Time Crisis: Project Titan
 3        Arcade            yes   Point Blank
 4        Arcade            no    Crisis Zone
"
        );
        assert!(table(&index, &games, false).starts_with("ID  Year  Platform          Title\n"));
    }
}
