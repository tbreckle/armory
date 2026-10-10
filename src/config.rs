//! The YAML configuration (`armory.yaml`): schema, validation and lookup.
//!
//! Parsing happens in two steps: serde reads the `Raw*` types, then [`Config::parse`] validates
//! them and resolves paths into the public types, with the line number for every error.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde_saphyr::Spanned;

use crate::launch::{Plan, Request, Target};
use crate::monitor;
use crate::status::{StatusTextOverrides, StatusTexts, Template};

pub const FILE_NAME: &str = "armory.yaml";
const FORMAT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------------------------
// Validated configuration

#[derive(Debug, Clone)]
pub struct Config {
    pub style: Style,
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Clone)]
pub struct Tab {
    pub name: String,
    pub image: Option<PathBuf>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Divider(Option<String>),
    Label(Label),
    Button(Button),
}

#[derive(Debug, Clone)]
pub struct Label {
    pub template: Template,
    /// Normalized process name (see [`monitor::normalize`]).
    pub monitor: Option<String>,
    pub status_texts: StatusTexts,
    pub tile: bool,
}

#[derive(Debug, Clone)]
pub struct Button {
    pub label: Option<String>,
    pub image: Option<PathBuf>,
    pub kind: ButtonKind,
}

#[derive(Debug, Clone)]
pub enum ButtonKind {
    Action(Action),
    Run(Program),
}

#[derive(Debug, Clone)]
pub struct Program {
    pub target: Target,
    pub args: Vec<String>,
    pub with: Option<String>,
    pub cwd: Option<PathBuf>,
    pub on_launch: OnLaunch,
    /// Normalized process name (see [`monitor::normalize`]).
    pub wait_for: Option<String>,
}

impl Program {
    pub fn plan(&self) -> Plan {
        Plan::for_current_os(Request {
            target: &self.target,
            args: &self.args,
            with: self.with.as_deref(),
            cwd: self.cwd.as_deref(),
        })
    }
}

/// Built-in functions of ARMORY that a button can trigger instead of a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Quit,
    Reload,
    Minimize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnLaunch {
    #[default]
    Stay,
    Minimize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// Look and sizes. All sizes are logical pixels.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    pub theme: Theme,
    pub font_size: f32,
    pub tab_width: f32,
    pub tab_height: f32,
    pub button_width: f32,
    pub button_height: f32,
    pub spacing: f32,
    pub accent_color: Color,
    pub status_colors: StatusColors,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            font_size: 28.0,
            tab_width: 340.0,
            tab_height: 110.0,
            button_width: 300.0,
            button_height: 200.0,
            spacing: 20.0,
            accent_color: Color(0xffb400),
            status_colors: StatusColors::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StatusColors {
    pub on: Color,
    pub off: Color,
    pub fault: Color,
}

impl Default for StatusColors {
    fn default() -> Self {
        Self {
            on: Color(0x2ecc71),
            off: Color(0x8a8f98),
            fault: Color(0xe74c3c),
        }
    }
}

/// RGB color, written as `"#rrggbb"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Color(pub u32);

impl TryFrom<String> for Color {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .map(Color)
            .ok_or_else(|| format!("invalid color {value:?}, expected \"#rrggbb\""))
    }
}

// ---------------------------------------------------------------------------------------------
// Errors

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("No configuration file found. Create {FILE_NAME} in one of these places:\n{}", list(.0))]
    NotFound(Vec<PathBuf>),
    #[error("Cannot read {}: {source}", .path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}:\n{message}", .path.display())]
    Syntax { path: PathBuf, message: String },
    #[error("{}, line {line}:\n{message}", .path.display())]
    Invalid {
        path: PathBuf,
        line: u64,
        message: String,
    },
}

fn list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| format!("  {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------------------------
// Lookup

/// Finds the configuration file: `explicit` (from `--config`), else `armory.yaml` next to the
/// executable, else in the user's config directory (`~/.config/armory`,
/// `%APPDATA%\armory`, `~/Library/Application Support/armory`).
pub fn locate(explicit: Option<PathBuf>) -> Result<PathBuf, ConfigError> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let candidates: Vec<PathBuf> = [
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(FILE_NAME))),
        dirs::config_dir().map(|dir| dir.join("armory").join(FILE_NAME)),
    ]
    .into_iter()
    .flatten()
    .collect();
    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .ok_or(ConfigError::NotFound(candidates))
}

// ---------------------------------------------------------------------------------------------
// Parsing and validation

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text, path)
    }

    /// Parses `text`; relative paths resolve against the folder of `path`.
    pub fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let raw: RawConfig = serde_saphyr::from_str(text).map_err(|e| ConfigError::Syntax {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        let base = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let invalid = |line: u64, message: String| ConfigError::Invalid {
            path: path.to_path_buf(),
            line,
            message,
        };

        if raw.version != FORMAT_VERSION {
            return Err(invalid(
                1,
                format!(
                    "unsupported version {}, this ARMORY reads version {FORMAT_VERSION}",
                    raw.version
                ),
            ));
        }
        let style = raw.style;
        let sizes = [
            ("font_size", style.font_size),
            ("tab_width", style.tab_width),
            ("tab_height", style.tab_height),
            ("button_width", style.button_width),
            ("button_height", style.button_height),
        ];
        if let Some((name, _)) = sizes.iter().find(|(_, v)| !(v.is_finite() && *v > 0.0)) {
            return Err(invalid(1, format!("style.{name} must be greater than 0")));
        }
        if !(style.spacing.is_finite() && style.spacing >= 0.0) {
            return Err(invalid(1, "style.spacing must not be negative".into()));
        }
        if raw.tabs.is_empty() {
            return Err(invalid(1, "at least one tab is required".into()));
        }

        let status_texts = StatusTexts::default().merged(&raw.status_texts);
        let mut tabs = Vec::with_capacity(raw.tabs.len());
        for tab in raw.tabs {
            let line = tab.referenced.line();
            let tab = tab.value;
            let name = tab.name.trim().to_string();
            if name.is_empty() {
                return Err(invalid(line, "tab needs a name".into()));
            }
            let image = tab
                .image
                .map(|image| existing_file(base, &image))
                .transpose()
                .map_err(|m| invalid(line, m))?;
            let mut items = Vec::with_capacity(tab.items.len());
            for item in tab.items {
                let line = item.referenced.line();
                let item = match item.value {
                    RawItem::Divider(text) => Item::Divider(non_empty(text)),
                    RawItem::Label(label) => Item::Label(
                        label
                            .validate(&status_texts)
                            .map_err(|m| invalid(line, m))?,
                    ),
                    RawItem::Button(button) => {
                        Item::Button(button.validate(base).map_err(|m| invalid(line, m))?)
                    }
                };
                items.push(item);
            }
            tabs.push(Tab { name, image, items });
        }
        Ok(Config { style, tabs })
    }

    /// Normalized names of all processes the labels monitor.
    pub fn monitored(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .tabs
            .iter()
            .flat_map(|t| &t.items)
            .filter_map(|item| match item {
                Item::Label(label) => label.monitor.clone(),
                _ => None,
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

fn existing_file(base: &Path, path: &Path) -> Result<PathBuf, String> {
    let resolved = base.join(path);
    if resolved.is_file() {
        Ok(resolved)
    } else {
        Err(format!("image not found: {}", resolved.display()))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default)]
    style: Style,
    #[serde(default)]
    status_texts: StatusTextOverrides,
    #[serde(default)]
    tabs: Vec<Spanned<RawTab>>,
}

fn default_version() -> u32 {
    FORMAT_VERSION
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTab {
    name: String,
    image: Option<PathBuf>,
    #[serde(default)]
    items: Vec<Spanned<RawItem>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawItem {
    Divider(Option<String>),
    Label(RawLabel),
    Button(RawButton),
}

/// A label is either just its text or a map with `text` and the other settings.
#[derive(Default)]
struct RawLabel {
    text: String,
    monitor: Option<String>,
    status_texts: Option<StatusTextOverrides>,
    tile: bool,
}

impl RawLabel {
    fn validate(self, global_texts: &StatusTexts) -> Result<Label, String> {
        let template = Template::parse(&self.text).map_err(|e| format!("label text: {e}"))?;
        let monitor = match self.monitor.as_deref().map(str::trim) {
            Some("") => return Err("monitor needs a process name".into()),
            Some(name) => Some(monitor::normalize(name)),
            None => None,
        };
        if monitor.is_none() {
            if template.has_status() {
                return Err("{STATUS} needs a monitored process (monitor: <process name>)".into());
            }
            if self.status_texts.is_some() {
                return Err(
                    "status_texts needs a monitored process (monitor: <process name>)".into(),
                );
            }
        }
        Ok(Label {
            template,
            monitor,
            status_texts: global_texts.merged(&self.status_texts.unwrap_or_default()),
            tile: self.tile,
        })
    }
}

impl<'de> Deserialize<'de> for RawLabel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            text: String,
            monitor: Option<String>,
            status_texts: Option<StatusTextOverrides>,
            #[serde(default)]
            tile: bool,
        }

        struct LabelVisitor;

        impl<'de> Visitor<'de> for LabelVisitor {
            type Value = RawLabel;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a label text, or a map with `text` and optional `monitor`, `status_texts`, `tile`")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<RawLabel, E> {
                Ok(RawLabel {
                    text: v.to_string(),
                    ..Default::default()
                })
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<RawLabel, E> {
                self.visit_str(&v.to_string())
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<RawLabel, E> {
                self.visit_str(&v.to_string())
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<RawLabel, E> {
                self.visit_str(&v.to_string())
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<RawLabel, E> {
                self.visit_str(&v.to_string())
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<RawLabel, A::Error> {
                let f = Fields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(RawLabel {
                    text: f.text,
                    monitor: f.monitor,
                    status_texts: f.status_texts,
                    tile: f.tile,
                })
            }
        }

        deserializer.deserialize_any(LabelVisitor)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawButton {
    label: Option<String>,
    image: Option<PathBuf>,
    run: Option<String>,
    action: Option<Action>,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<PathBuf>,
    with: Option<String>,
    on_launch: Option<OnLaunch>,
    wait_for: Option<String>,
}

impl RawButton {
    fn validate(self, base: &Path) -> Result<Button, String> {
        let label = non_empty(self.label);
        let image = self
            .image
            .map(|image| existing_file(base, &image))
            .transpose()?;
        if label.is_none() && image.is_none() {
            return Err("button needs a label, an image, or both".into());
        }

        let kind = match (self.run, self.action) {
            (Some(_), Some(_)) => return Err("button has both run and action, choose one".into()),
            (None, None) => return Err("button needs run (a program) or action".into()),
            (None, Some(action)) => {
                let program_settings = [
                    ("args", !self.args.is_empty()),
                    ("cwd", self.cwd.is_some()),
                    ("with", self.with.is_some()),
                    ("on_launch", self.on_launch.is_some()),
                    ("wait_for", self.wait_for.is_some()),
                ];
                if let Some((name, _)) = program_settings.iter().find(|(_, set)| *set) {
                    return Err(format!("{name} only works with run, not with action"));
                }
                ButtonKind::Action(action)
            }
            (Some(run), None) => {
                let run = run.trim();
                if run.is_empty() {
                    return Err("run needs a program".into());
                }
                let on_launch = self.on_launch.unwrap_or_default();
                let wait_for = match self.wait_for.as_deref().map(str::trim) {
                    Some("") => return Err("wait_for needs a process name".into()),
                    Some(_) if on_launch != OnLaunch::Minimize => {
                        return Err("wait_for only works with on_launch: minimize".into());
                    }
                    Some(name) => Some(monitor::normalize(name)),
                    None => None,
                };
                let program = Program {
                    target: Target::resolve(run, base),
                    args: self.args,
                    with: non_empty(self.with),
                    cwd: self.cwd.map(|cwd| base.join(cwd)),
                    on_launch,
                    wait_for,
                };
                let opened_by_os = matches!(&program.target, Target::Path(p) if p.exists())
                    && !program.plan().tracked;
                if on_launch == OnLaunch::Minimize && program.wait_for.is_none() && opened_by_os {
                    return Err(format!(
                        "{run} is opened through its file association, so ARMORY can't tell when \
                         it ends. Add wait_for: <process name> to restore ARMORY afterwards."
                    ));
                }
                ButtonKind::Run(program)
            }
        };
        Ok(Button { label, image, kind })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Status;

    fn parse(yaml: &str) -> Result<Config, ConfigError> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        Config::parse(yaml, &dir.join(FILE_NAME))
    }

    fn error(yaml: &str) -> String {
        parse(yaml).unwrap_err().to_string()
    }

    #[test]
    fn example_config_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(FILE_NAME);
        let config = Config::load(&path).unwrap_or_else(|e| panic!("{e}"));
        assert!(!config.tabs.is_empty());
    }

    #[test]
    fn minimal_config_uses_defaults() {
        let config = parse("tabs:\n  - name: Games\n").unwrap();
        assert_eq!(config.style, Style::default());
        assert!(config.tabs[0].items.is_empty());
    }

    #[test]
    fn items_in_all_forms() {
        let config = parse(
            r##"
status_texts: { fault: Error }
style:
  theme: light
  status_colors: { on: "#00ff00" }
tabs:
  - name: Games
    items:
      - divider:
      - divider: Section
      - label: Plain
      - label: 42
      - label:
          text: "Driver: {STATUS}"
          monitor: Driver.exe
          status_texts: { on: Yes }
          tile: true
      - button: { label: Quit, action: quit }
      - button: { label: Game, run: game.exe, args: [-f], on_launch: minimize, wait_for: game.exe }
"##,
        )
        .unwrap();
        assert_eq!(config.style.theme, Theme::Light);
        assert_eq!(config.style.status_colors.on, Color(0x00ff00));
        assert_eq!(config.style.status_colors.off, StatusColors::default().off);

        let items = &config.tabs[0].items;
        assert!(matches!(items[0], Item::Divider(None)));
        assert!(matches!(&items[1], Item::Divider(Some(t)) if t == "Section"));
        assert!(matches!(&items[3], Item::Label(l) if l.template.render(None) == "42"));
        let Item::Label(status) = &items[4] else {
            panic!("expected a label")
        };
        assert_eq!(
            status.monitor.as_deref(),
            Some(monitor::normalize("Driver").as_str())
        );
        assert!(status.tile);
        assert_eq!(status.status_texts.get(Status::On), "Yes");
        assert_eq!(status.status_texts.get(Status::Off), "Off");
        assert_eq!(status.status_texts.get(Status::Fault), "Error");
        assert!(matches!(
            &items[5],
            Item::Button(Button {
                kind: ButtonKind::Action(Action::Quit),
                ..
            })
        ));
        let Item::Button(Button {
            kind: ButtonKind::Run(program),
            ..
        }) = &items[6]
        else {
            panic!("expected a program button")
        };
        assert_eq!(program.on_launch, OnLaunch::Minimize);
        assert_eq!(program.args, ["-f"]);
    }

    #[test]
    fn errors_name_the_line() {
        let e = error("tabs:\n  - name: A\n    items:\n      - button: { run: x }\n");
        assert!(e.contains("line 4"), "{e}");
        assert!(e.contains("needs a label, an image"), "{e}");
    }

    #[test]
    fn rejects_invalid_buttons() {
        let button = |b: &str| {
            error(&format!(
                "tabs:\n  - name: A\n    items:\n      - button: {b}\n"
            ))
        };
        assert!(button("{ label: x }").contains("needs run"));
        assert!(button("{ label: x, run: a, action: quit }").contains("both run and action"));
        assert!(
            button("{ label: x, action: quit, args: [a] }").contains("args only works with run")
        );
        assert!(button("{ label: x, run: a, wait_for: b }").contains("on_launch: minimize"));
        assert!(button("{ label: x, image: missing.png, run: a }").contains("image not found"));
        assert!(button("{ label: x, run: a, color: red }").contains("color"));
    }

    #[test]
    fn rejects_invalid_labels() {
        let label = |l: &str| {
            error(&format!(
                "tabs:\n  - name: A\n    items:\n      - label: {l}\n"
            ))
        };
        assert!(label("\"{STATUS}\"").contains("needs a monitored process"));
        assert!(
            label("{ text: x, status_texts: { on: y } }").contains("needs a monitored process")
        );
        assert!(label("\"{STATE}\"").contains("unknown placeholder"));
    }

    #[test]
    fn rejects_invalid_top_level() {
        assert!(error("tabs: []").contains("at least one tab"));
        assert!(error("version: 2\ntabs:\n  - name: A").contains("unsupported version"));
        assert!(error("style: { font_size: 0 }\ntabs:\n  - name: A").contains("font_size"));
        assert!(error("style: { accent_color: red }\ntabs:\n  - name: A").contains("#rrggbb"));
        assert!(error("tabs:\n  - name: ' '").contains("tab needs a name"));
    }
}
