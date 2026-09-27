//! Background check of monitored processes, once per second.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub const INTERVAL: Duration = Duration::from_secs(1);

/// Number of running processes per watched name; `None` if the process list couldn't be read.
pub type Counts = HashMap<String, Option<usize>>;

/// Normalizes a process name for comparison: without directory and `.exe`, and lowercase on
/// Windows and macOS, whose file systems are usually case-insensitive. Both `/` and `\\` count
/// as separators, so Windows paths of Wine processes match on Linux too.
pub fn normalize(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let name = if cfg!(any(windows, target_os = "macos")) {
        name.to_lowercase()
    } else {
        name.to_string()
    };
    match name.len().checked_sub(4) {
        Some(i) if name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(".exe") => {
            name[..i].to_string()
        }
        _ => name,
    }
}

/// Counts processes per watched (normalized) name. Each process is given by its known names
/// (process name, executable, first command line argument); it counts once if any matches.
pub fn count<'a, P, N>(watched: &[String], processes: P) -> HashMap<String, usize>
where
    P: IntoIterator<Item = N>,
    N: IntoIterator<Item = &'a str>,
{
    let mut counts: HashMap<String, usize> = watched.iter().map(|w| (w.clone(), 0)).collect();
    for names in processes {
        let names: Vec<String> = names.into_iter().map(normalize).collect();
        for watched in watched {
            if names.iter().any(|n| n == watched) {
                *counts.entry(watched.clone()).or_default() += 1;
            }
        }
    }
    counts
}

/// Starts the monitor thread. It checks the names in `watched` (normalized) every second and
/// sends the result through `send`. The thread ends when the receiver is gone.
pub fn spawn<E: Send + 'static>(
    watched: Arc<Mutex<Vec<String>>>,
    sender: Sender<E>,
    wrap: fn(Counts) -> E,
) {
    std::thread::spawn(move || {
        let mut system = System::new();
        let refresh = ProcessRefreshKind::nothing()
            .without_tasks()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet);
        loop {
            let names = watched.lock().map(|w| w.clone()).unwrap_or_default();
            let counts = if names.is_empty() {
                Counts::new()
            } else {
                system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);
                check(&system, &names)
            };
            if sender.send(wrap(counts)).is_err() {
                return;
            }
            std::thread::sleep(INTERVAL);
        }
    });
}

fn check(system: &System, names: &[String]) -> Counts {
    let processes = system.processes();
    // An empty list means the processes couldn't be read (at least ARMORY itself runs).
    if !sysinfo::IS_SUPPORTED_SYSTEM || processes.is_empty() {
        return names.iter().map(|n| (n.clone(), None)).collect();
    }
    let known = processes
        .values()
        .filter(|p| p.thread_kind().is_none())
        .map(|p| {
            let exe = p.exe().and_then(Path::file_name);
            let arg0 = p.cmd().first().map(OsStr::new);
            std::iter::once(p.name())
                .chain(exe)
                .chain(arg0)
                .filter_map(OsStr::to_str)
                .collect::<Vec<_>>()
        });
    count(names, known)
        .into_iter()
        .map(|(name, n)| (name, Some(n)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_names() {
        assert_eq!(normalize("tool.exe"), "tool");
        assert_eq!(normalize("/usr/bin/tool"), "tool");
        assert_eq!(normalize("tool.exe.bak"), "tool.exe.bak");
        assert_eq!(normalize("é"), "é");
        assert_eq!(normalize("C:\\Tools\\Sinden.EXE").to_lowercase(), "sinden");
    }

    #[test]
    fn counts_each_process_once() {
        let watched = vec![normalize("tool.exe"), normalize("other")];
        let counts = count(
            &watched,
            [
                // Name and executable match: still one process.
                vec!["tool", "/opt/tool", "tool"],
                // Only the command line matches (e.g. a Wine process).
                vec![
                    "wine-preloader",
                    "/usr/bin/wine-preloader",
                    "C:\\x\\tool.exe",
                ],
                vec!["unrelated"],
            ],
        );
        assert_eq!(counts[&watched[0]], 2);
        assert_eq!(counts[&watched[1]], 0);
    }
}
