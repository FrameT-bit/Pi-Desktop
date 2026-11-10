
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;

use crate::theme_catalog;
use crate::tokens;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
    Named(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Palette {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn next(self) -> Self {
        match self {
            ThemeMode::System => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
            ThemeMode::Named(_) => ThemeMode::System,
        }
    }

    pub fn resolve(self, system_prefers_dark: bool) -> Palette {
        match self {
            ThemeMode::System if system_prefers_dark => Palette::Dark,
            ThemeMode::System => Palette::Light,
            ThemeMode::Light => Palette::Light,
            ThemeMode::Dark => Palette::Dark,
            ThemeMode::Named(index) => {
                if theme_catalog::at(index as usize).is_some_and(|theme| theme.category.is_dark()) {
                    Palette::Dark
                } else {
                    Palette::Light
                }
            }
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let value = value
            .split(['#', ';'])
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        match value.as_str() {
            "system" => Some(ThemeMode::System),
            "light" | "clear" => Some(ThemeMode::Light),
            "dark" | "Dark" => Some(ThemeMode::Dark),
            _ => theme_catalog::find(&value).map(|index| ThemeMode::Named(index as u8)),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
            ThemeMode::Named(index) => theme_catalog::at(index as usize)
                .map(|theme| theme.id)
                .unwrap_or("system"),
        }
    }

    pub fn menu_label(self) -> String {
        match self {
            ThemeMode::Named(index) => theme_catalog::at(index as usize)
                .map(|theme| format!("{} · {}", theme.category.group(), theme.name))
                .unwrap_or_else(|| self.label().to_string()),
            _ => self.label().to_string(),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Light => "Sure.",
            ThemeMode::Dark => "Dark",
            ThemeMode::Named(index) => theme_catalog::at(index as usize)
                .map(|theme| theme.name)
                .unwrap_or("System"),
        }
    }

    pub fn index(self) -> usize {
        match self {
            ThemeMode::System => 0,
            ThemeMode::Light => 1,
            ThemeMode::Dark => 2,
            ThemeMode::Named(index) => 3 + index as usize,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => ThemeMode::Light,
            2 => ThemeMode::Dark,
            index if index >= 3 && index - 3 < theme_catalog::THEMES.len() => {
                ThemeMode::Named((index - 3) as u8)
            }
            _ => ThemeMode::System,
        }
    }

    pub fn all() -> Vec<Self> {
        let mut out = vec![ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];
        out.extend((0..theme_catalog::THEMES.len()).map(|index| ThemeMode::Named(index as u8)));
        out
    }

    fn named_index(self) -> Option<usize> {
        match self {
            ThemeMode::Named(index) => Some(index as usize),
            _ => None,
        }
    }
}

impl Palette {
    pub fn section(self) -> &'static str {
        match self {
            Palette::Light => "light",
            Palette::Dark => "dark",
        }
    }

    fn css(self) -> &'static str {
        match self {
            Palette::Dark => include_str!(concat!(env!("OUT_DIR"), "/style-dark.css")),
            Palette::Light => include_str!(concat!(env!("OUT_DIR"), "/style-light.css")),
        }
    }
}

type Listener = Rc<dyn Fn()>;

struct Inner {
    mode: Cell<ThemeMode>,
    palette: Cell<Palette>,
    system_dark: Cell<bool>,
    font_scale: Cell<u32>,
    persist: Cell<bool>,
    provider: RefCell<Option<gtk::CssProvider>>,
    listeners: RefCell<Vec<Listener>>,
}

#[derive(Clone)]
pub struct Theme {
    inner: Rc<Inner>,
}

pub fn init() -> Theme {
    let from_env = std::env::var("PI_DESKTOP_THEME")
        .ok()
        .and_then(|value| ThemeMode::parse(&value));
    let forced = from_env.is_some();
    let mode = from_env.or_else(load_mode).unwrap_or(ThemeMode::System);
    let system_dark = system_prefers_dark();

    let theme = Theme {
        inner: Rc::new(Inner {
            mode: Cell::new(mode),
            palette: Cell::new(mode.resolve(system_dark)),
            system_dark: Cell::new(system_dark),
            font_scale: Cell::new(100),
            persist: Cell::new(!forced),
            provider: RefCell::new(None),
            listeners: RefCell::new(Vec::new()),
        }),
    };

    theme.apply();
    theme.watch_system();
    if !forced {
        save_mode(mode);
    }
    theme
}

impl Theme {
    #[allow(dead_code)]
    pub fn mode(&self) -> ThemeMode {
        self.inner.mode.get()
    }

    #[allow(dead_code)]
    pub fn palette(&self) -> Palette {
        self.inner.palette.get()
    }

    pub fn cycle(&self) {
        self.set_mode(self.inner.mode.get().next());
    }

    pub fn set_mode(&self, mode: ThemeMode) {
        self.inner.mode.set(mode);
        if self.inner.persist.get() {
            save_mode(mode);
        }
        self.apply();
    }

    pub fn tooltip(&self) -> String {
        format!("Subject: {}", self.inner.mode.get().label())
    }

    pub fn set_font_scale(&self, percent: u32) {
        let percent = percent.clamp(90, 130);
        if self.inner.font_scale.get() == percent {
            return;
        }
        self.inner.font_scale.set(percent);
        self.apply();
    }

    #[allow(dead_code)]
    pub fn font_scale(&self) -> u32 {
        self.inner.font_scale.get()
    }

    pub fn page_bg_rgb(&self) -> (f64, f64, f64) {
        self.rgb("pageBg", (0.0, 0.0, 0.0))
    }

    pub fn border_rgb(&self) -> (f64, f64, f64) {
        self.rgb("border", (1.0, 1.0, 1.0))
    }

    fn rgb(&self, key: &str, fallback: (f64, f64, f64)) -> (f64, f64, f64) {
        tokens::lookup(self.inner.palette.get().section(), key)
            .map(|_| tokens::rgb(&tokens::active(key)))
            .unwrap_or(fallback)
    }

    pub fn connect_changed(&self, callback: impl Fn() + 'static) {
        self.inner
            .listeners
            .borrow_mut()
            .push(Rc::new(callback));
    }

    fn apply(&self) {
        let palette = self
            .inner
            .mode
            .get()
            .resolve(self.inner.system_dark.get());
        self.inner.palette.set(palette);

        let named_tokens = self
            .inner
            .mode
            .get()
            .named_index()
            .map(theme_catalog::tokens)
            .unwrap_or_default();
        if named_tokens.is_empty() {
            crate::tokens::set_active(palette.section());
        } else {
            crate::tokens::set_active_named(palette.section(), &named_tokens);
        }

        let provider = gtk::CssProvider::new();
        let scale = self.inner.font_scale.get();
        let css = apply_named_tokens(palette.css(), &named_tokens);
        provider.load_from_string(&scale_css(&css, scale));

        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
            if let Some(old) = self.inner.provider.replace(Some(provider)) {
                gtk::style_context_remove_provider_for_display(&display, &old);
            }
        } else {
            self.inner.provider.replace(Some(provider));
        }

        adw::StyleManager::default().set_color_scheme(match self.inner.mode.get() {
            ThemeMode::System => adw::ColorScheme::Default,
            ThemeMode::Light => adw::ColorScheme::ForceLight,
            ThemeMode::Dark => adw::ColorScheme::ForceDark,
            ThemeMode::Named(index) => {
                if theme_catalog::at(index as usize).is_some_and(|theme| theme.category.is_dark()) {
                    adw::ColorScheme::ForceDark
                } else {
                    adw::ColorScheme::ForceLight
                }
            }
        });

        let listeners: Vec<Listener> = self.inner.listeners.borrow().clone();
        for listener in listeners {
            listener();
        }
    }

    fn watch_system(&self) {
        let Some(settings) = gtk::Settings::default() else {
            return;
        };
        self.inner
            .system_dark
            .set(settings.is_gtk_application_prefer_dark_theme());

        let weak = Rc::downgrade(&self.inner);
        settings.connect_notify_local(
            Some("gtk-application-prefer-dark-theme"),
            move |settings, _| {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                inner
                    .system_dark
                    .set(settings.is_gtk_application_prefer_dark_theme());
                if inner.mode.get() == ThemeMode::System {
                    Theme { inner }.apply();
                }
            },
        );
    }
}

fn system_prefers_dark() -> bool {
    gtk::Settings::default()
        .map(|settings| settings.is_gtk_application_prefer_dark_theme())
        .unwrap_or(false)
}

fn camel_to_snake(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 4);
    for (index, ch) in value.char_indices() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn apply_named_tokens(base: &str, overrides: &[(String, String)]) -> String {
    if overrides.is_empty() {
        return base.to_string();
    }
    let mut out = base.to_string();
    for (key, value) in overrides {
        let marker = format!("@define-color {} ", camel_to_snake(key));
        if let Some(start) = out.find(&marker) {
            let value_start = start + marker.len();
            if let Some(end) = out[value_start..].find(';') {
                out.replace_range(value_start..value_start + end, value);
            }
        }
    }
    out
}

fn scale_css(css: &str, percent: u32) -> String {
    if percent == 100 {
        return css.to_string();
    }
    let factor = percent as f64 / 100.0;
    let mut out = String::with_capacity(css.len() + css.len() / 20);
    for line in css.lines() {
        let Some(pos) = line.find("font-size:") else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let rest = &line[pos + "font-size:".len()..];
        let trimmed = rest.trim_start();
        let digits: String = trimmed
            .chars()
            .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
            .collect();
        let Ok(size) = digits.parse::<f64>() else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let suffix = &trimmed[digits.len()..];
        let scaled = size * factor;
        let rendered = if (scaled - scaled.round()).abs() < 0.05 {
            format!("{:.0}", scaled)
        } else {
            format!("{scaled:.1}")
        };
        out.push_str(&line[..pos]);
        out.push_str("font-size:");
        out.push_str(&rest[..rest.len() - trimmed.len()]);
        out.push_str(&rendered);
        out.push_str(suffix);
        out.push('\n');
    }
    out
}

pub fn save_preference(mode: ThemeMode) {
    save_mode(mode);
}

pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PI_DESKTOP_CONFIG") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("pi-desktop").join("config.ini"))
}

pub fn load_mode() -> Option<ThemeMode> {
    let text = fs::read_to_string(config_path()?).ok()?;
    parse_config(&text)
}

pub fn parse_config(text: &str) -> Option<ThemeMode> {
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[') {
            section = name
                .split(']')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !(section.is_empty() || section == "ui") {
            continue;
        }
        if key.trim().eq_ignore_ascii_case("theme") {
            return ThemeMode::parse(value);
        }
    }
    None
}



fn save_mode(mode: ThemeMode) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if parse_config(&existing) == Some(mode) {
        return;
    }
    let _ = fs::write(path, upsert_ui_theme(&existing, mode));
}

fn upsert_ui_theme(text: &str, mode: ThemeMode) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut section = String::new();
    let mut ui_start: Option<usize> = None;
    let mut theme_line: Option<usize> = None;

    for (index, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if let Some(name) = line.strip_prefix('[') {
            section = name
                .split(']')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if section == "ui" && ui_start.is_none() {
                ui_start = Some(index);
            }
            continue;
        }
        if section == "ui" && !line.starts_with('#') && !line.starts_with(';') {
            if let Some((key, _)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("theme") {
                    theme_line = Some(index);
                }
            }
        }
    }

    let setting = format!("theme={}", mode.as_str());
    match (theme_line, ui_start) {
        (Some(index), _) => lines[index] = setting,
        (None, Some(index)) => lines.insert(index + 1, setting),
        (None, None) => {
            if !lines.is_empty() && lines.last().is_some_and(|line| !line.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[ui]".to_string());
            lines.push(setting);
        }
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}


