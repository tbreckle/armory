//! Starting programs: executables directly, scripts through their interpreter, other files
//! through the file association of the OS.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

/// What `run` refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A file, resolved against the folder of the YAML file.
    Path(PathBuf),
    /// A bare command name, looked up in `PATH` by the OS.
    Command(String),
}

impl Target {
    /// Absolute paths and paths with a folder part are files; a bare name is a file only if
    /// it exists next to the YAML file, otherwise a command from `PATH`.
    pub fn resolve(run: &str, base: &Path) -> Self {
        let path = Path::new(run);
        if path.is_absolute() {
            return Target::Path(path.to_path_buf());
        }
        let has_folder = path.components().count() > 1 || run.starts_with('.');
        let candidate = base.join(path);
        if has_folder || candidate.exists() {
            Target::Path(candidate)
        } else {
            Target::Command(run.to_string())
        }
    }
}

/// A resolved command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    /// The started process is the program itself (or waits for it), so its exit means the
    /// program has ended. False for openers that return right away (`start`, `xdg-open`).
    pub tracked: bool,
    /// The OS opener can't pass `args` on, so they are dropped.
    pub args_ignored: bool,
    /// Start without a console window (Windows `cmd` used as an opener).
    pub hide_console: bool,
}

/// Program settings of a button.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub target: &'a Target,
    pub args: &'a [String],
    pub with: Option<&'a str>,
    pub cwd: Option<&'a Path>,
}

impl Plan {
    /// Builds the command line for `os`. `executable` tells whether the target is a file with
    /// the executable bit (Unix only).
    pub fn new(os: Os, request: Request<'_>, executable: bool) -> Self {
        let args: Vec<OsString> = request.args.iter().map(OsString::from).collect();
        let cwd = request
            .cwd
            .map(Path::to_path_buf)
            .or_else(|| match request.target {
                Target::Path(p) => p.parent().map(Path::to_path_buf),
                Target::Command(_) => None,
            });
        let direct = |program: OsString| Plan {
            program,
            args: args.clone(),
            cwd: cwd.clone(),
            tracked: true,
            args_ignored: false,
            hide_console: false,
        };
        let via = |program: &str, mut before: Vec<OsString>| {
            before.extend(args.iter().cloned());
            Plan {
                program: program.into(),
                args: before,
                cwd: cwd.clone(),
                tracked: true,
                args_ignored: false,
                hide_console: false,
            }
        };

        let path = match request.target {
            Target::Command(name) => {
                return match request.with {
                    Some(with) => via(with, vec![name.into()]),
                    None => direct(name.into()),
                };
            }
            Target::Path(path) => path,
        };
        if let Some(with) = request.with {
            return via(with, vec![path.into()]);
        }

        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match os {
            Os::Windows => match ext.as_str() {
                // std runs .bat/.cmd through cmd.exe with safe argument quoting.
                "exe" | "com" | "bat" | "cmd" => direct(path.into()),
                "ps1" => via(
                    "powershell",
                    vec![
                        "-NoProfile".into(),
                        "-ExecutionPolicy".into(),
                        "Bypass".into(),
                        "-File".into(),
                        path.into(),
                    ],
                ),
                _ => {
                    // `start` hands the file to its association and returns right away.
                    let mut plan = via(
                        "cmd",
                        vec!["/C".into(), "start".into(), "".into(), path.into()],
                    );
                    plan.tracked = false;
                    plan.hide_console = true;
                    plan
                }
            },
            Os::Mac => match ext.as_str() {
                // `open -W` waits until the app quits, so the program stays tracked.
                "app" => via("open", vec!["-W".into(), path.into(), "--args".into()]),
                _ if executable => direct(path.into()),
                "sh" | "command" => via("sh", vec![path.into()]),
                "bash" => via("bash", vec![path.into()]),
                _ => Plan {
                    program: "open".into(),
                    args: vec!["-W".into(), path.into()],
                    cwd,
                    tracked: true,
                    args_ignored: !args.is_empty(),
                    hide_console: false,
                },
            },
            Os::Linux => match ext.as_str() {
                _ if executable => direct(path.into()),
                "sh" => via("sh", vec![path.into()]),
                "bash" => via("bash", vec![path.into()]),
                _ => Plan {
                    program: "xdg-open".into(),
                    args: vec![path.into()],
                    cwd,
                    tracked: false,
                    args_ignored: !args.is_empty(),
                    hide_console: false,
                },
            },
        }
    }

    /// Plans `request` for this OS, checking the executable bit of the target.
    pub fn for_current_os(request: Request<'_>) -> Self {
        let executable = match request.target {
            Target::Path(path) => is_executable_file(path),
            Target::Command(_) => false,
        };
        Plan::new(Os::current(), request, executable)
    }

    pub fn spawn(&self) -> std::io::Result<Child> {
        let mut command = Command::new(&self.program);
        command.args(&self.args).stdin(Stdio::null());
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        #[cfg(windows)]
        if self.hide_console {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn()
    }
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(os: Os, run: &str, args: &[&str], with: Option<&str>, executable: bool) -> Plan {
        let target = Target::Path(PathBuf::from(run));
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        Plan::new(
            os,
            Request {
                target: &target,
                args: &args,
                with,
                cwd: None,
            },
            executable,
        )
    }

    fn line(plan: &Plan) -> Vec<String> {
        std::iter::once(&plan.program)
            .chain(&plan.args)
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn resolves_targets() {
        let absolute = std::env::temp_dir().join("x");
        assert_eq!(
            Target::resolve(&absolute.to_string_lossy(), Path::new("/cfg")),
            Target::Path(absolute)
        );
        let base = Path::new("/cfg");
        assert_eq!(
            Target::resolve("scripts/a.sh", base),
            Target::Path("/cfg/scripts/a.sh".into())
        );
        assert_eq!(
            Target::resolve("./a", base),
            Target::Path("/cfg/./a".into())
        );
        assert_eq!(
            Target::resolve("notepad", base),
            Target::Command("notepad".into())
        );
    }

    #[test]
    fn windows_plans() {
        let exe = plan(Os::Windows, "C:/g/game.EXE", &["-f"], None, false);
        assert_eq!(line(&exe), ["C:/g/game.EXE", "-f"]);
        assert!(exe.tracked);
        assert_eq!(exe.cwd, Some(PathBuf::from("C:/g")));

        let ps = plan(Os::Windows, "C:/s/x.ps1", &["a"], None, false);
        assert_eq!(
            line(&ps),
            [
                "powershell",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                "C:/s/x.ps1",
                "a"
            ]
        );

        let ahk = plan(Os::Windows, "C:/s/recoil.ahk", &["a"], None, false);
        assert_eq!(
            line(&ahk),
            ["cmd", "/C", "start", "", "C:/s/recoil.ahk", "a"]
        );
        assert!(!ahk.tracked);
        assert!(ahk.hide_console);
    }

    #[test]
    fn linux_plans() {
        let bin = plan(Os::Linux, "/g/game", &["-f"], None, true);
        assert_eq!(line(&bin), ["/g/game", "-f"]);
        let sh = plan(Os::Linux, "/s/a.sh", &["x"], None, false);
        assert_eq!(line(&sh), ["sh", "/s/a.sh", "x"]);
        let doc = plan(Os::Linux, "/s/manual.pdf", &["x"], None, false);
        assert_eq!(line(&doc), ["xdg-open", "/s/manual.pdf"]);
        assert!(!doc.tracked);
        assert!(doc.args_ignored);
    }

    #[test]
    fn macos_plans() {
        let app = plan(Os::Mac, "/Applications/Game.app", &["-f"], None, true);
        assert_eq!(
            line(&app),
            ["open", "-W", "/Applications/Game.app", "--args", "-f"]
        );
        assert!(app.tracked);
        let doc = plan(Os::Mac, "/s/readme.txt", &[], None, false);
        assert_eq!(line(&doc), ["open", "-W", "/s/readme.txt"]);
        assert!(!doc.args_ignored);
    }

    #[test]
    fn with_overrides_detection() {
        for os in [Os::Windows, Os::Mac, Os::Linux] {
            let p = plan(os, "/s/recoil.ahk", &["a"], Some("AutoHotkey.exe"), false);
            assert_eq!(line(&p), ["AutoHotkey.exe", "/s/recoil.ahk", "a"]);
            assert!(p.tracked);
        }
    }

    #[test]
    fn commands_run_from_path() {
        let target = Target::Command("notepad".into());
        let p = Plan::new(
            Os::Windows,
            Request {
                target: &target,
                args: &["x.txt".to_string()],
                with: None,
                cwd: None,
            },
            false,
        );
        assert_eq!(line(&p), ["notepad", "x.txt"]);
        assert_eq!(p.cwd, None);
    }
}
