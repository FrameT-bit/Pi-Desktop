
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontChoice {
    Pixel,
    Adwaita,
    System,
}

impl FontChoice {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pixel" | "pixelify" => Some(FontChoice::Pixel),
            "adwaita" => Some(FontChoice::Adwaita),
            "system" => Some(FontChoice::System),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FontChoice::Pixel => "pixel",
            FontChoice::Adwaita => "adwaita",
            FontChoice::System => "system",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FontChoice::Pixel => "Pixelify Sans + JetBrains Mono",
            FontChoice::Adwaita => "Adwaita Sans + Adwaita Mono",
            FontChoice::System => "System Sans + System Mono",
        }
    }

    pub fn families(self) -> (&'static str, &'static str) {
        match self {
            FontChoice::Pixel => ("Pixelify Sans", "JetBrains Mono"),
            FontChoice::Adwaita => ("Adwaita Sans", "Adwaita Mono"),
            FontChoice::System => ("Sans", "Monospace"),
        }
    }

    pub fn index(self) -> usize {
        match self {
            FontChoice::Pixel => 0,
            FontChoice::Adwaita => 1,
            FontChoice::System => 2,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => FontChoice::Adwaita,
            2 => FontChoice::System,
            _ => FontChoice::Pixel,
        }
    }
}

thread_local! {
    static ACTIVE_FONT: Cell<FontChoice> = const { Cell::new(FontChoice::Pixel) };
}

pub fn mono_font() -> &'static str {
    ACTIVE_FONT.with(|font| font.get().families().1)
}

pub fn body_font() -> &'static str {
    ACTIVE_FONT.with(|font| font.get().families().0)
}

fn apply_fonts(css: &str, font: FontChoice) -> String {
    let body_decl = match font {
        FontChoice::Pixel => "\"Pixelify Sans\", serif".to_string(),
        FontChoice::Adwaita => "\"Adwaita Sans\", sans-serif".to_string(),
        FontChoice::System => "Sans, sans-serif".to_string(),
    };
    let mono_decl = match font {
        FontChoice::Pixel => "\"JetBrains Mono\", monospace".to_string(),
        FontChoice::Adwaita => "\"Adwaita Mono\", monospace".to_string(),
        FontChoice::System => "Monospace, monospace".to_string(),
    };
    css.replace("\"Pixelify Sans\", serif", &body_decl)
        .replace("\"JetBrains Mono\", monospace", &mono_decl)
}


impl ThemeMode {
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
            "light" => Some(ThemeMode::Light),
            "dark" => Some(ThemeMode::Dark),
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
            ThemeMode::Light => "Light",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemePreferences {
    pub mode: ThemeMode,
    pub light: ThemeMode,
    pub dark: ThemeMode,
}

impl Default for ThemePreferences {
    fn default() -> Self {
        Self { mode: ThemeMode::System, light: ThemeMode::Light, dark: ThemeMode::Dark }
    }
}

impl ThemePreferences {
    pub fn parse(text: &str) -> Self {
        let mut prefs = Self::default();
        prefs.select(parse_config(text).unwrap_or(ThemeMode::System));
        for (key, palette) in [("light_theme", Palette::Light), ("dark_theme", Palette::Dark)] {
            if let Some(mode) = ui_value(text, key).and_then(ThemeMode::parse) {
                if mode != ThemeMode::System && mode.resolve(false) == palette {
                    match palette {
                        Palette::Light => prefs.light = mode,
                        Palette::Dark => prefs.dark = mode,
                    }
                }
            }
        }
        prefs
    }

    pub fn select(&mut self, mode: ThemeMode) {
        self.mode = mode;
        if let ThemeMode::Named(_) = mode {
            match mode.resolve(false) {
                Palette::Light => self.light = mode,
                Palette::Dark => self.dark = mode,
            }
        }
    }

    pub fn effective(self, system_dark: bool) -> ThemeMode {
        match self.mode.resolve(system_dark) {
            Palette::Light => self.light,
            Palette::Dark => self.dark,
        }
    }

    pub fn appearance_index(self) -> usize {
        match self.mode {
            ThemeMode::System => 0,
            _ if self.mode.resolve(false) == Palette::Light => 1,
            _ => 2,
        }
    }

    pub fn render(self, text: &str) -> String {
        let text = upsert_ui_key(text, "theme", self.mode.as_str());
        let text = upsert_ui_key(&text, "light_theme", self.light.as_str());
        upsert_ui_key(&text, "dark_theme", self.dark.as_str())
    }
}

pub fn palette_modes(palette: Palette) -> Vec<ThemeMode> {
    ThemeMode::all().into_iter()
        .filter(|mode| *mode != ThemeMode::System && mode.resolve(false) == palette)
        .collect()
}

pub fn load_preferences() -> ThemePreferences {
    config_path().and_then(|path| fs::read_to_string(path).ok())
        .map(|text| ThemePreferences::parse(&text)).unwrap_or_default()
}

pub fn save_preferences(prefs: ThemePreferences) -> std::io::Result<()> {
    let Some(path) = config_path() else { return Ok(()) };
    let existing = fs::read_to_string(&path).unwrap_or_default();
    crate::config::write_atomic(&path, &prefs.render(&existing))
}

fn ui_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut in_ui = true;
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('[') {
            in_ui = line.trim_matches(['[', ']']).eq_ignore_ascii_case("ui");
            return None;
        }
        if !in_ui || line.starts_with(['#', ';']) { return None; }
        let (name, value) = line.split_once('=')?;
        name.trim().eq_ignore_ascii_case(key).then_some(value.trim())
    })
}

type Listener = Rc<dyn Fn()>;

struct Inner {
    mode: Cell<ThemeMode>,
    preferences: Cell<ThemePreferences>,
    palette: Cell<Palette>,
    system_dark: Cell<bool>,
    font_scale: Cell<u32>,
    font: Cell<FontChoice>,
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
    let mut preferences = load_preferences();
    if let Some(mode) = from_env { preferences.select(mode); }
    let mode = preferences.mode;
    let system_dark = system_prefers_dark();
    let font = load_font().unwrap_or(FontChoice::Pixel);

    let theme = Theme {
        inner: Rc::new(Inner {
            mode: Cell::new(mode),
            preferences: Cell::new(preferences),
            palette: Cell::new(mode.resolve(system_dark)),
            system_dark: Cell::new(system_dark),
            font_scale: Cell::new(100),
            font: Cell::new(font),
            persist: Cell::new(!forced),
            provider: RefCell::new(None),
            listeners: RefCell::new(Vec::new()),
        }),
    };

    theme.apply();
    theme.watch_system();
    if !forced {
        if let Err(error) = save_preferences(preferences) {
            eprintln!("pi-desktop: saving appearance: {error}");
        }
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
        self.set_mode(match self.palette() {
            Palette::Light => ThemeMode::Dark,
            Palette::Dark => ThemeMode::Light,
        });
    }

    pub fn set_mode(&self, mode: ThemeMode) {
        let mut preferences = self.inner.preferences.get();
        preferences.select(mode);
        self.set_preferences(preferences);
    }

    pub fn set_preferences(&self, preferences: ThemePreferences) {
        self.inner.mode.set(preferences.mode);
        self.inner.preferences.set(preferences);
        if self.inner.persist.get() {
            if let Err(error) = save_preferences(preferences) {
                eprintln!("pi-desktop: saving appearance: {error}");
            }
        }
        self.apply();
    }

    pub fn tooltip(&self) -> String {
        let prefs = self.inner.preferences.get();
        let next = if self.palette() == Palette::Dark { prefs.light } else { prefs.dark };
        format!("{} · switch to {}", prefs.effective(self.inner.system_dark.get()).label(), next.label())
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

    pub fn font(&self) -> FontChoice {
        self.inner.font.get()
    }

    pub fn set_font(&self, font: FontChoice) {
        if self.inner.font.get() == font {
            return;
        }
        self.inner.font.set(font);
        if self.inner.persist.get() {
            save_font(font);
        }
        self.apply();
    }

    pub fn page_bg_rgb(&self) -> (f64, f64, f64) {
        self.rgb("pageBg", (0.0, 0.0, 0.0))
    }

    pub fn border_rgb(&self) -> (f64, f64, f64) {
        self.rgb("border", (1.0, 1.0, 1.0))
    }

    pub fn accent_rgb(&self) -> (f64, f64, f64) {
        self.rgb("accent", (0.42, 0.62, 0.8))
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

        let named_tokens = self.inner.preferences.get().effective(self.inner.system_dark.get())
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
        let font = self.inner.font.get();
        ACTIVE_FONT.with(|active| active.set(font));
        let css = apply_fonts(&apply_named_tokens(palette.css(), &named_tokens), font);
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

pub fn save_font_preference(font: FontChoice) {
    save_font(font);
}

pub fn load_font() -> Option<FontChoice> {
    let text = fs::read_to_string(config_path()?).ok()?;
    parse_font_config(&text)
}

pub fn parse_font_config(text: &str) -> Option<FontChoice> {
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
        let Some((key, value)) = line.split_once('=') else { continue };
        if !(section.is_empty() || section == "ui") {
            continue;
        }
        if key.trim().eq_ignore_ascii_case("font") {
            return FontChoice::parse(value);
        }
    }
    None
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



fn save_font(font: FontChoice) {
    let Some(path) = config_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if parse_font_config(&existing) == Some(font) {
        return;
    }
    let _ = fs::write(path, upsert_ui_key(&existing, "font", font.as_str()));
}

fn upsert_ui_key(text: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut in_ui = false;
    let mut ui_end = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_ui {
                ui_end = Some(index);
                break;
            }
            in_ui = trimmed[1..trimmed.len() - 1].trim().eq_ignore_ascii_case("ui");
        }
        if in_ui && trimmed.split_once('=').is_some_and(|(name, _)| name.trim().eq_ignore_ascii_case(key)) {
            lines[index] = format!("{key}={value}");
            return lines.join("\n");
        }
    }
    let insert_at = ui_end.unwrap_or_else(|| {
        if !lines.iter().any(|line| line.trim().eq_ignore_ascii_case("[ui]")) {
            lines.push("[ui]".to_string());
        }
        lines.len()
    });
    lines.insert(insert_at, format!("{key}={value}"));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_named_theme_migrates_into_its_palette_slot() {
        for name in ["midnight-fudge", "oat-milk"] {
            let prefs = ThemePreferences::parse(&format!("[ui]\ntheme={name}\n"));
            assert_eq!(prefs.effective(false).as_str(), name);
            let mut toggled = prefs;
            toggled.select(if prefs.mode.resolve(false) == Palette::Dark { ThemeMode::Light } else { ThemeMode::Dark });
            toggled.select(prefs.mode);
            assert_eq!(toggled.effective(false).as_str(), name);
        }
    }

    #[test]
    fn appearance_switch_preserves_both_selected_themes() {
        let text = "[pi]\nmodel=openai/test\n[ui]\ntheme=system\nlight_theme=oat-milk\ndark_theme=midnight-fudge\nsession_tabs=false\n";
        let mut prefs = ThemePreferences::parse(text);
        assert_eq!(prefs.effective(false).as_str(), "oat-milk");
        assert_eq!(prefs.effective(true).as_str(), "midnight-fudge");
        for mode in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::Light] {
            prefs.select(mode);
            let saved = prefs.render(text);
            assert_eq!(ThemePreferences::parse(&saved), prefs);
            assert!(saved.contains("model=openai/test"));
            assert!(saved.contains("session_tabs=false"));
        }
    }

    #[test]
    fn theme_choices_reject_wrong_palette_and_system() {
        let prefs = ThemePreferences::parse("[ui]\nlight_theme=midnight-fudge\ndark_theme=system");
        assert_eq!(prefs.light, ThemeMode::Light);
        assert_eq!(prefs.dark, ThemeMode::Dark);
        for palette in [Palette::Light, Palette::Dark] {
            assert!(palette_modes(palette).iter().all(|mode| *mode != ThemeMode::System && mode.resolve(false) == palette));
        }
    }
}
