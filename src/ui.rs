//! Window and controller: loads the configuration, pushes tabs and items to the Slint UI,
//! handles clicks, and applies monitor results.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::config::{
    self, Action, ButtonKind, Config, Item, OnLaunch, Program, Theme as ThemeKind,
};
use crate::launch::Target;
use crate::layout::{Layout, Shape};
use crate::monitor::{self, Counts};
use crate::status::Status;

slint::include_modules!();

/// How often the UI thread picks up monitor results and exited programs.
const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Mouse wheel distance that scrolls one row: one wheel notch in Slint's winit backend.
/// Touchpads report pixels, which add up until they reach it.
const WHEEL_STEP: f32 = 60.0;
/// How long a `wait_for` process may take to appear before ARMORY restores anyway.
const WAIT_FOR_APPEAR_TIMEOUT: Duration = Duration::from_secs(30);

/// Font for the error screen, whose snippets point at columns.
const MONO_FONT: &str = if cfg!(windows) {
    "Consolas"
} else if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "DejaVu Sans Mono"
};

enum Event {
    Counts(Counts),
    ChildExited(u64),
}

pub fn run(config_arg: Option<PathBuf>) -> anyhow::Result<()> {
    let app = AppWindow::new()?;
    app.set_version(crate::VERSION.into());
    app.global::<Theme>().set_mono_font(MONO_FONT.into());

    let (sender, receiver) = mpsc::channel();
    let watched = Arc::new(Mutex::new(Vec::new()));
    monitor::spawn(watched.clone(), sender.clone(), Event::Counts);

    let controller = Rc::new(RefCell::new(Controller::new(
        &app, config_arg, watched, sender,
    )));
    controller.borrow_mut().load();
    connect(&app, &controller);
    let _poll = poll_events(controller.clone(), receiver);

    // Full display size, but a normal window (not fullscreen).
    maximize_when_ready(app.as_weak());
    app.run()?;
    Ok(())
}

/// Slint's set_maximized() before the window exists is overwritten while winit creates the
/// window, so this maximizes the winit window directly, retrying until it exists.
fn maximize_when_ready(app: slint::Weak<AppWindow>) {
    slint::Timer::single_shot(Duration::from_millis(20), move || {
        use slint::winit_030::WinitWindowAccessor;
        let Some(window) = app.upgrade() else { return };
        if window
            .window()
            .with_winit_window(|w| w.set_maximized(true))
            .is_none()
        {
            maximize_when_ready(app);
        }
    });
}

fn connect(app: &AppWindow, controller: &Rc<RefCell<Controller>>) {
    let c = controller.clone();
    app.on_tab_clicked(move |i| c.borrow_mut().select_tab(i as usize));
    let c = controller.clone();
    app.on_item_clicked(move |i| c.borrow_mut().click_item(i as usize));
    let c = controller.clone();
    app.on_page(move |direction| c.borrow_mut().page(direction));
    let c = controller.clone();
    app.on_scroll(move |delta| c.borrow_mut().scroll(delta));
    let c = controller.clone();
    app.on_content_resized(move |w, h| c.borrow_mut().resize(w, h));
    let c = controller.clone();
    app.on_reload(move || c.borrow_mut().load());
    let c = controller.clone();
    app.on_dismiss_message(move || c.borrow().show_message("", ""));
    app.on_quit(quit);
}

fn poll_events(controller: Rc<RefCell<Controller>>, receiver: Receiver<Event>) -> slint::Timer {
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, POLL_INTERVAL, move || {
        while let Ok(event) = receiver.try_recv() {
            controller.borrow_mut().handle(event);
        }
    });
    timer
}

fn quit() {
    // Only fails if the event loop isn't running, and then there is nothing to quit.
    let _ = slint::quit_event_loop();
}

/// ARMORY is minimized while a program runs and comes back when it ends.
struct Restore {
    launch: u64,
    wait_for: Option<WaitFor>,
}

struct WaitFor {
    name: String,
    seen: bool,
    since: Instant,
}

struct Controller {
    app: slint::Weak<AppWindow>,
    items: Rc<VecModel<ItemData>>,
    config_arg: Option<PathBuf>,
    config: Option<Config>,
    images: HashMap<PathBuf, slint::Image>,
    active_tab: usize,
    view: (f32, f32),
    layout: Layout,
    offset: f32,
    /// Mouse wheel distance not yet turned into row steps; positive scrolls up.
    wheel: f32,
    counts: Counts,
    restore: Option<Restore>,
    launches: u64,
    watched: Arc<Mutex<Vec<String>>>,
    events: Sender<Event>,
}

impl Controller {
    fn new(
        app: &AppWindow,
        config_arg: Option<PathBuf>,
        watched: Arc<Mutex<Vec<String>>>,
        events: Sender<Event>,
    ) -> Self {
        let items = Rc::new(VecModel::default());
        app.set_items(ModelRc::from(items.clone()));
        Self {
            app: app.as_weak(),
            items,
            config_arg,
            config: None,
            images: HashMap::new(),
            active_tab: 0,
            view: (0.0, 0.0),
            layout: Layout::default(),
            offset: 0.0,
            wheel: 0.0,
            counts: Counts::new(),
            restore: None,
            launches: 0,
            watched,
            events,
        }
    }

    fn with_app(&self, f: impl FnOnce(&AppWindow)) {
        if let Some(app) = self.app.upgrade() {
            f(&app);
        }
    }

    /// (Re)loads the configuration file; on errors, the error screen replaces the content.
    fn load(&mut self) {
        let loaded = config::locate(self.config_arg.clone())
            .map_err(|e| e.to_string())
            .and_then(|path| Config::load(&path).map_err(|e| e.to_string()))
            .and_then(|config| load_images(&config).map(|images| (config, images)));
        match loaded {
            Ok((config, images)) => {
                self.images = images;
                self.active_tab = self.active_tab.min(config.tabs.len() - 1);
                self.apply_style(&config);
                self.config = Some(config);
                self.with_app(|app| app.set_config_error("".into()));
            }
            Err(message) => {
                self.config = None;
                self.images.clear();
                self.active_tab = 0;
                self.with_app(|app| app.set_config_error(message.into()));
            }
        }
        self.offset = 0.0;
        self.update_watched();
        self.push_tabs();
        self.refresh();
    }

    fn apply_style(&self, config: &Config) {
        let style = &config.style;
        self.with_app(|app| {
            let theme = app.global::<Theme>();
            theme.set_dark(style.theme == ThemeKind::Dark);
            theme.set_font_size(style.font_size);
            theme.set_accent(color(style.accent_color));
            theme.set_status_on(color(style.status_colors.on));
            theme.set_status_off(color(style.status_colors.off));
            theme.set_status_fault(color(style.status_colors.fault));
            app.set_spacing(style.spacing);
            app.set_tab_width(style.tab_width);
            app.set_tab_height(style.tab_height);
            app.set_scroll_width(scroll_width(style.font_size));
        });
    }

    fn push_tabs(&self) {
        let tabs: Vec<TabData> = self
            .config
            .iter()
            .flat_map(|c| &c.tabs)
            .map(|tab| {
                let image = self.image(tab.image.as_deref());
                TabData {
                    name: tab.name.clone().into(),
                    has_image: image.is_some(),
                    image: image.unwrap_or_default(),
                }
            })
            .collect();
        self.with_app(|app| {
            app.set_tabs(ModelRc::new(VecModel::from(tabs)));
            app.set_active_tab(self.active_tab as i32);
        });
    }

    fn image(&self, path: Option<&Path>) -> Option<slint::Image> {
        path.and_then(|p| self.images.get(p)).cloned()
    }

    /// Recomputes the layout of the active tab and pushes items and paging state.
    fn refresh(&mut self) {
        let (width, height) = self.view;
        let mut data = Vec::new();
        let mut show_scroll = false;
        self.layout = Layout::default();

        if let Some(config) = &self.config {
            let style = &config.style;
            let tab = &config.tabs[self.active_tab];
            let shapes: Vec<Shape> = tab
                .items
                .iter()
                .map(|item| shape(item, &config.style))
                .collect();
            self.layout = Layout::new(&shapes, width, style.spacing);
            if self.layout.height > height {
                show_scroll = true;
                let narrower = width - scroll_width(style.font_size) - style.spacing;
                self.layout = Layout::new(&shapes, narrower, style.spacing);
            }
            data = tab
                .items
                .iter()
                .zip(&self.layout.rects)
                .map(|(item, rect)| {
                    let mut d = self.item_data(item);
                    d.x = rect.x;
                    d.y = rect.y;
                    d.width = rect.width;
                    d.height = rect.height;
                    d
                })
                .collect();
        }

        self.offset = self.layout.clamp_offset(self.offset, height);
        sync_model(&self.items, data);
        let (offset, max) = (self.offset, self.layout.max_offset(height));
        self.with_app(|app| {
            app.set_show_scroll(show_scroll);
            app.set_scroll_offset(offset);
            app.set_can_page_up(offset > 0.5);
            app.set_can_page_down(offset < max - 0.5);
        });
    }

    fn item_data(&self, item: &Item) -> ItemData {
        match item {
            Item::Divider(text) => ItemData {
                kind: ItemKind::Divider,
                text: text.clone().unwrap_or_default().into(),
                ..Default::default()
            },
            Item::Label(label) => {
                let (status, text) = match &label.monitor {
                    Some(name) => {
                        let status = self.status(name);
                        let text = label.template.render(Some(label.status_texts.get(status)));
                        (status_kind(status), text)
                    }
                    None => (StatusKind::Unmonitored, label.template.render(None)),
                };
                ItemData {
                    kind: ItemKind::Label,
                    text: text.into(),
                    status,
                    tile: label.tile,
                    ..Default::default()
                }
            }
            Item::Button(button) => {
                let image = self.image(button.image.as_deref());
                ItemData {
                    kind: ItemKind::Button,
                    text: button.label.clone().unwrap_or_default().into(),
                    has_image: image.is_some(),
                    image: image.unwrap_or_default(),
                    tile: true,
                    ..Default::default()
                }
            }
        }
    }

    /// Status of a monitored process; Off until the first check has run.
    fn status(&self, name: &str) -> Status {
        match self.counts.get(name) {
            Some(count) => Status::from_count(*count),
            None => Status::Off,
        }
    }

    fn resize(&mut self, width: f32, height: f32) {
        // A minimized window reports a zero size; keep the last layout.
        if width > 0.0 && height > 0.0 && (width, height) != self.view {
            self.view = (width, height);
            self.refresh();
        }
    }

    fn select_tab(&mut self, index: usize) {
        let count = self.config.as_ref().map_or(0, |c| c.tabs.len());
        if index < count && index != self.active_tab {
            self.active_tab = index;
            self.offset = 0.0;
            self.with_app(|app| app.set_active_tab(index as i32));
            self.refresh();
        }
    }

    fn page(&mut self, direction: i32) {
        let height = self.view.1;
        self.offset = if direction < 0 {
            self.layout.page_up(self.offset, height)
        } else {
            self.layout.page_down(self.offset, height)
        };
        self.refresh();
    }

    /// Mouse wheel: scrolls row by row, `delta` > 0 is up.
    fn scroll(&mut self, delta: f32) {
        if self.wheel * delta < 0.0 {
            self.wheel = 0.0;
        }
        self.wheel += delta;
        let height = self.view.1;
        let start = self.offset;
        while self.wheel.abs() >= WHEEL_STEP {
            let step = self.wheel.signum() * WHEEL_STEP;
            self.offset = if step > 0.0 {
                self.layout.row_up(self.offset, height)
            } else {
                self.layout.row_down(self.offset, height)
            };
            self.wheel -= step;
        }
        if self.offset != start {
            self.refresh();
        }
    }

    fn click_item(&mut self, index: usize) {
        let Some(item) = self
            .config
            .as_ref()
            .and_then(|c| c.tabs[self.active_tab].items.get(index))
        else {
            return;
        };
        let Item::Button(button) = item else {
            return;
        };
        match &button.kind {
            ButtonKind::Action(Action::Quit) => quit(),
            ButtonKind::Action(Action::Reload) => self.load(),
            ButtonKind::Action(Action::Minimize) => {
                self.with_app(|app| app.window().set_minimized(true))
            }
            ButtonKind::Run(program) => {
                let program = program.clone();
                self.launch(&program);
            }
        }
    }

    fn launch(&mut self, program: &Program) {
        if let Target::Path(path) = &program.target
            && !path.exists()
        {
            self.show_message("Program not found", &path.display().to_string());
            return;
        }
        let plan = program.plan();
        if let Some(cwd) = plan.cwd.as_ref().filter(|cwd| !cwd.is_dir()) {
            self.show_message("Working directory not found", &cwd.display().to_string());
            return;
        }
        if plan.args_ignored {
            eprintln!(
                "armory: {:?} is opened through its file association, args are ignored",
                plan.args.last().unwrap_or(&plan.program)
            );
        }
        let mut child = match plan.spawn() {
            Ok(child) => child,
            Err(e) => {
                let program = std::iter::once(&plan.program)
                    .chain(&plan.args)
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                self.show_message("Cannot start program", &format!("{program}\n\n{e}"));
                return;
            }
        };

        self.launches += 1;
        let launch = self.launches;
        let events = self.events.clone();
        // Waiting also reaps the child, so no zombie processes stay behind on Unix.
        std::thread::spawn(move || {
            let _ = child.wait();
            let _ = events.send(Event::ChildExited(launch));
        });

        if program.on_launch == OnLaunch::Minimize {
            let wait_for = program.wait_for.clone().map(|name| WaitFor {
                name,
                seen: false,
                since: Instant::now(),
            });
            if wait_for.is_none() && !plan.tracked {
                // Rejected when the config is loaded; the file only became an OS-opened one
                // afterwards. Without a way back, staying visible is the safe choice.
                return;
            }
            self.restore = Some(Restore { launch, wait_for });
            self.update_watched();
            self.with_app(|app| app.window().set_minimized(true));
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Counts(counts) => {
                self.counts = counts;
                if let Some(Restore {
                    wait_for: Some(wait_for),
                    ..
                }) = &mut self.restore
                {
                    // A failed check (None) neither counts as running nor as ended.
                    match self.counts.get(&wait_for.name).copied().flatten() {
                        Some(n) if n > 0 => wait_for.seen = true,
                        Some(_) if wait_for.seen => self.restore_window(),
                        _ if !wait_for.seen
                            && wait_for.since.elapsed() > WAIT_FOR_APPEAR_TIMEOUT =>
                        {
                            self.restore_window()
                        }
                        _ => {}
                    }
                }
                self.refresh();
            }
            Event::ChildExited(launch) => {
                if matches!(&self.restore, Some(r) if r.launch == launch && r.wait_for.is_none()) {
                    self.restore_window();
                }
            }
        }
    }

    fn restore_window(&mut self) {
        self.restore = None;
        self.update_watched();
        self.with_app(|app| {
            let window = app.window();
            window.set_minimized(false);
            window.set_maximized(true);
            use slint::winit_030::WinitWindowAccessor;
            window.with_winit_window(|w| w.focus_window());
        });
    }

    /// Tells the monitor thread which processes to check.
    fn update_watched(&self) {
        let mut names = self
            .config
            .as_ref()
            .map(Config::monitored)
            .unwrap_or_default();
        if let Some(Restore {
            wait_for: Some(wait_for),
            ..
        }) = &self.restore
        {
            names.push(wait_for.name.clone());
        }
        if let Ok(mut watched) = self.watched.lock() {
            *watched = names;
        }
    }

    fn show_message(&self, title: &str, message: &str) {
        self.with_app(|app| {
            app.set_message_title(SharedString::from(title));
            app.set_message(SharedString::from(message));
        });
    }
}

fn load_images(config: &Config) -> Result<HashMap<PathBuf, slint::Image>, String> {
    let paths = config.tabs.iter().flat_map(|tab| {
        tab.image
            .iter()
            .chain(tab.items.iter().filter_map(|item| match item {
                Item::Button(button) => button.image.as_ref(),
                _ => None,
            }))
    });
    let mut images = HashMap::new();
    for path in paths {
        if !images.contains_key(path) {
            let image = slint::Image::load_from_path(path)
                .map_err(|_| format!("Cannot load image {}", path.display()))?;
            images.insert(path.clone(), image);
        }
    }
    Ok(images)
}

fn shape(item: &Item, style: &config::Style) -> Shape {
    let tile = Shape::Tile {
        width: style.button_width,
        height: style.button_height,
    };
    match item {
        Item::Divider(_) => Shape::Full {
            height: style.font_size * 2.0,
        },
        Item::Label(label) if label.tile => tile,
        Item::Label(label) => {
            let lines = label.template.render(None).lines().count().max(1) as f32;
            Shape::Full {
                height: lines * style.font_size * 1.4 + style.font_size * 0.8,
            }
        }
        Item::Button(_) => tile,
    }
}

/// Width of the page up/down column.
fn scroll_width(font_size: f32) -> f32 {
    (font_size * 3.5).max(100.0)
}

fn color(c: config::Color) -> slint::Color {
    let [_, r, g, b] = c.0.to_be_bytes();
    slint::Color::from_rgb_u8(r, g, b)
}

fn status_kind(status: Status) -> StatusKind {
    match status {
        Status::On => StatusKind::On,
        Status::Off => StatusKind::Off,
        Status::Fault => StatusKind::Fault,
    }
}

/// Replaces only changed rows, so a button that is being pressed keeps its state while the
/// status labels update every second.
fn sync_model(model: &VecModel<ItemData>, data: Vec<ItemData>) {
    if model.row_count() != data.len() {
        model.set_vec(data);
        return;
    }
    for (row, item) in data.into_iter().enumerate() {
        if model.row_data(row).as_ref() != Some(&item) {
            model.set_row_data(row, item);
        }
    }
}
