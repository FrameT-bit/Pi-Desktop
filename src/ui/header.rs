
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::prelude::*;

use crate::icons::{self, Tone};
use crate::logos;
use crate::state::{ModelItem, SessionActivity};
use crate::theme::{Palette, Theme};

use super::popup::Popup;

pub type ModelFn = Box<dyn Fn(String, String, String)>;

const MARK_ON_DARK: &[u8] = include_bytes!("../../resources/brand/mark-on-dark-32.png");
const MARK_ON_LIGHT: &[u8] = include_bytes!("../../resources/brand/mark-on-light-32.png");

fn mark_paintable(palette: Palette) -> Option<gtk::gdk::Texture> {
    let bytes = match palette {
        Palette::Dark => MARK_ON_DARK,
        Palette::Light => MARK_ON_LIGHT,
    };
    gtk::gdk::Texture::from_bytes(&glib::Bytes::from_static(bytes)).ok()
}

fn theme_icon(palette: Palette) -> &'static str {
    match palette {
        Palette::Light => "sun",
        Palette::Dark => "moon",
    }
}

pub fn elide_middle(text: &str) -> String {
    const MAX: usize = 40;
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX {
        return text.to_string();
    }
    let head: String = chars[..19].iter().collect();
    let tail: String = chars[chars.len() - 20..].iter().collect();
    format!("{head}…{tail}")
}

struct MenuState {
    models: Vec<ModelItem>,
    provider: String,
    id: String,
    query: String,
}

pub struct HeaderUi {
    pub root: gtk::CenterBox,
    tabs: gtk::Box,
    center: gtk::Stack,
    chat_title: gtk::Label,
    pub mode_chat: gtk::ToggleButton,
    pub mode_code: gtk::ToggleButton,
    pub sidebar_toggle: gtk::Button,
    theme_button: gtk::Button,
    busy: gtk::Box,
    busy_label: gtk::Label,
    pub model: gtk::Label,
    logo_slot: gtk::Box,
    provider: Rc<RefCell<String>>,
    theme: Theme,
    pub settings: gtk::Button,
    pub help: gtk::Button,
    palette: Rc<Popup>,
    on_model: Rc<RefCell<Option<ModelFn>>>,
    on_session: Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
    on_new: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_close: Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
    preferred: Rc<Cell<usize>>,
    menu: Rc<RefCell<MenuState>>,
}

pub fn build(model: &str, theme: &Theme) -> HeaderUi {
    let root = gtk::CenterBox::new();
    root.add_css_class("header-panel");
    root.set_hexpand(true);


    let mark = gtk::Image::new();
    mark.add_css_class("wordmark-mark");
    mark.set_pixel_size(24);
    mark.set_valign(gtk::Align::Center);
    if let Some(texture) = mark_paintable(theme.palette()) {
        mark.set_paintable(Some(&texture));
    }

    let word = gtk::Label::new(Some("Pi Desktop"));
    word.add_css_class("wordmark");

    let wordmark = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    wordmark.add_css_class("wordmark-box");
    wordmark.append(&mark);
    wordmark.append(&word);
    wordmark.set_valign(gtk::Align::Center);


    let sidebar_toggle = gtk::Button::new();
    sidebar_toggle.add_css_class("theme-button");
    sidebar_toggle.add_css_class("sidebar-toggle");
    sidebar_toggle.set_valign(gtk::Align::Center);
    sidebar_toggle.set_child(Some(&icons::icon("compact", Tone::Ink)));
    sidebar_toggle.set_tooltip_text(Some("show/hide chats"));

    let start = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    start.add_css_class("header-start");
    start.set_valign(gtk::Align::Center);
    start.append(&wordmark);
    start.append(&sidebar_toggle);
    root.set_start_widget(Some(&start));

    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    tabs.add_css_class("session-tabs");
    tabs.set_valign(gtk::Align::Center);

    let chat_title = gtk::Label::new(None);
    chat_title.add_css_class("header-chat-title");
    chat_title.set_xalign(0.5);
    chat_title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    chat_title.set_max_width_chars(56);

    let center = gtk::Stack::new();
    center.set_transition_type(gtk::StackTransitionType::None);
    center.add_named(&tabs, Some("tabs"));
    center.add_named(&chat_title, Some("title"));
    center.set_visible_child_name("tabs");
    root.set_center_widget(Some(&center));

    let busy_dot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    busy_dot.add_css_class("busy-dot");
    busy_dot.set_valign(gtk::Align::Center);
    let busy_label = gtk::Label::new(None);
    busy_label.add_css_class("busy-label");
    let busy = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    busy.add_css_class("busy-chip");
    busy.set_valign(gtk::Align::Center);
    busy.append(&busy_dot);
    busy.append(&busy_label);
    busy.set_visible(false);


    let model_label = gtk::Label::new(Some(model));
    model_label.add_css_class("model-chip-label");

    let caret = icons::icon("chevron-down", Tone::Muted);
    caret.add_css_class("model-chip-chevron");
    caret.set_valign(gtk::Align::Center);

    let logo_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    logo_slot.add_css_class("model-chip-logo");
    logo_slot.set_valign(gtk::Align::Center);

    let chip_body = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    chip_body.append(&logo_slot);
    chip_body.append(&model_label);
    chip_body.append(&caret);

    let model_button = gtk::Button::new();
    model_button.add_css_class("model-chip");
    model_button.set_child(Some(&chip_body));
    model_button.set_tooltip_text(Some("Change model"));
    model_button.set_valign(gtk::Align::Center);

    let help_button = gtk::Button::with_label("?");
    help_button.add_css_class("theme-button");
    help_button.add_css_class("help-button");
    help_button.set_tooltip_text(Some("keyboard shortcuts (Ctrl+/)"));
    help_button.set_valign(gtk::Align::Center);

    let settings_image = icons::icon("settings", Tone::Ink);
    let settings_button = gtk::Button::new();
    settings_button.add_css_class("theme-button");
    settings_button.add_css_class("settings-button");
    settings_button.set_child(Some(&settings_image));
    settings_button.set_tooltip_text(Some("settings (Ctrl+,)"));
    settings_button.set_valign(gtk::Align::Center);

    let theme_image = icons::icon(theme_icon(theme.palette()), Tone::Ink);

    let theme_button = gtk::Button::new();
    theme_button.add_css_class("theme-button");
    theme_button.set_child(Some(&theme_image));
    theme_button.set_tooltip_text(Some(&theme.tooltip()));
    theme_button.set_valign(gtk::Align::Center);

    let controls = gtk::WindowControls::new(gtk::PackType::End);
    controls.add_css_class("window-controls");
    controls.set_valign(gtk::Align::Center);

    let mode_chat = gtk::ToggleButton::with_label("CHAT");
    mode_chat.add_css_class("mode-option");
    mode_chat.set_tooltip_text(Some("Chat mode"));
    let mode_code = gtk::ToggleButton::with_label("code");
    mode_code.add_css_class("mode-option");
    mode_code.set_group(Some(&mode_chat));
    mode_code.set_tooltip_text(Some("Code mode"));
    let mode_switch = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    mode_switch.add_css_class("mode-switch");
    mode_switch.append(&mode_chat);
    mode_switch.append(&mode_code);

    let end = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    end.add_css_class("header-end");
    end.set_valign(gtk::Align::Center);
    end.append(&busy);
    end.append(&mode_switch);
    end.append(&model_button);
    end.append(&help_button);
    end.append(&settings_button);
    end.append(&theme_button);
    end.append(&controls);
    root.set_end_widget(Some(&end));


    let theme_c = theme.clone();
    let image_c = theme_image.clone();
    let button_c = theme_button.clone();
    let mark_c = mark.clone();
    let sync = move || {
        if let Some(texture) = mark_paintable(theme_c.palette()) {
            mark_c.set_paintable(Some(&texture));
        }
        icons::set_icon(&image_c, theme_icon(theme_c.palette()), Tone::Ink);
        button_c.set_tooltip_text(Some(&theme_c.tooltip()));
    };
    sync();
    theme.connect_changed(sync);

    let theme_c = theme.clone();
    theme_button.connect_clicked(move |_| theme_c.cycle());


    let palette = Popup::new(gtk::PositionType::Bottom, 380);
    palette.attach(&model_button);
    palette.enable_search();

    let menu = Rc::new(RefCell::new(MenuState {
        models: Vec::new(),
        provider: String::new(),
        id: String::new(),
        query: String::new(),
    }));

    let on_model: Rc<RefCell<Option<ModelFn>>> = Rc::new(RefCell::new(None));
    let on_session: Rc<RefCell<Option<Box<dyn Fn(usize)>>>> = Rc::new(RefCell::new(None));
    let on_new: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let on_close: Rc<RefCell<Option<Box<dyn Fn(usize)>>>> = Rc::new(RefCell::new(None));

    let palette_c = palette.clone();
    let preferred = Rc::new(Cell::new(0));
    let preferred_c = preferred.clone();
    model_button.connect_clicked(move |_| {
        if palette_c.is_open() {
            palette_c.close();
        } else {
            palette_c.open(380);
            palette_c.select_index(preferred_c.get());
        }
    });

    {
        let menu = menu.clone();
        let palette = palette.clone();
        let theme = theme.clone();
        let on_model = on_model.clone();
        let preferred = preferred.clone();
        let search = palette.search.clone();
        search.connect_changed(move |entry| {
            menu.borrow_mut().query = entry.text().to_string();
            rebuild_menu(&menu.borrow(), &palette, &theme, &on_model, &preferred);
            palette.select_index(preferred.get());
        });
    }

    {
        let palette_key = palette.clone();
        let search = palette.search.clone();
        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        key.connect_key_pressed(move |_, key, _, _| match key {
            gtk::gdk::Key::Up => {
                palette_key.move_selection(-1);
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Down => {
                palette_key.move_selection(1);
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                palette_key.activate_selected();
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Escape => {
                palette_key.close();
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        search.add_controller(key);
    }

    let provider: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    {
        let theme_c = theme.clone();
        let slot = logo_slot.clone();
        let provider = provider.clone();
        theme.connect_changed(move || {
            paint_logo(&slot, &provider.borrow(), theme_c.palette());
        });
    }
    paint_logo(&logo_slot, "", theme.palette());

    HeaderUi {
        root,
        tabs,
        center,
        chat_title,
        mode_chat,
        mode_code,
        sidebar_toggle,
        theme_button,
        busy,
        busy_label,
        model: model_label,
        logo_slot,
        provider,
        theme: theme.clone(),
        settings: settings_button,
        help: help_button,
        palette,
        on_model,
        on_session,
        on_new,
        on_close,
        preferred,
        menu,
    }
}

fn paint_logo(slot: &gtk::Box, provider: &str, palette: Palette) {
    while let Some(child) = slot.first_child() {
        slot.remove(&child);
    }
    if provider.trim().is_empty() {
        return;
    }
    slot.append(&logos::widget(provider, palette));
}

impl HeaderUi {
    pub fn set_on_session(&self, callback: impl Fn(usize) + 'static) {
        *self.on_session.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_new(&self, callback: impl Fn() + 'static) {
        *self.on_new.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_busy(&self, running: bool, text: &str) {
        self.busy.set_visible(running);
        if running {
            self.busy_label.set_text(text);
        }
    }

    pub fn set_sessions(
        &self,
        sessions: &[crate::state::RemoteSession],
        active: Option<usize>,
        closed: &std::collections::HashSet<String>,
        activity: &HashMap<String, SessionActivity>,
    ) {
        while let Some(child) = self.tabs.first_child() {
            self.tabs.remove(&child);
        }
        let visible: Vec<usize> = sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| !closed.contains(&session.path))
            .map(|(index, _)| index)
            .collect();
        if visible.is_empty() {
            let plus = self.new_tab_button();
            self.tabs.append(&plus);
            return;
        }
        let mut window: Vec<usize> = visible.iter().copied().take(5).collect();
        if let Some(active) = active {
            if visible.contains(&active) && !window.contains(&active) {
                window.pop();
                window.push(active);
                window.sort_unstable();
            }
        }
        for index in window {
            let session = &sessions[index];
            let running = activity
                .get(&session.session_id)
                .or_else(|| activity.get(&session.path))
                .is_some_and(|activity| activity.running);
            let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            group.add_css_class("session-tab-group");
            if Some(index) == active {
                group.add_css_class("active");
            }
            if running {
                group.add_css_class("running");
                let dot = gtk::Box::new(gtk::Orientation::Vertical, 0);
                dot.add_css_class("session-tab-dot");
                dot.set_valign(gtk::Align::Center);
                group.append(&dot);
            }
            let label = gtk::Label::new(Some(&elide_middle(&session.title())));
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_single_line_mode(true);
            let button = gtk::Button::new();
            button.add_css_class("session-tab");
            button.set_tooltip_text(Some(&session.title()));
            button.set_child(Some(&label));
            button.set_valign(gtk::Align::Center);
            let on_session = self.on_session.clone();
            button.connect_clicked(move |_| {
                if let Some(callback) = on_session.borrow().as_ref() {
                    callback(index);
                }
            });
            group.append(&button);

            let close = gtk::Button::with_label("×");
            close.add_css_class("session-tab-close");
            close.set_tooltip_text(Some("close tab (session stays on disk)"));
            close.set_valign(gtk::Align::Center);
            let on_close = self.on_close.clone();
            close.connect_clicked(move |_| {
                if let Some(callback) = on_close.borrow().as_ref() {
                    callback(index);
                }
            });
            group.append(&close);
            self.tabs.append(&group);
        }
        let plus = self.new_tab_button();
        self.tabs.append(&plus);
    }

    fn new_tab_button(&self) -> gtk::Button {
        let plus = gtk::Button::with_label("+");
        plus.add_css_class("session-tab");
        plus.add_css_class("add");
        plus.set_tooltip_text(Some("New session (Ctrl+N)"));
        plus.set_valign(gtk::Align::Center);
        let on_new = self.on_new.clone();
        plus.connect_clicked(move |_| {
            if let Some(callback) = on_new.borrow().as_ref() {
                callback();
            }
        });
        plus
    }

    pub fn set_view(&self, chat: bool) {
        self.center
            .set_visible_child_name(if chat { "title" } else { "tabs" });
        self.root.remove_css_class("chat-mode");
        if chat {
            self.root.add_css_class("chat-mode");
        }
        self.help.set_visible(!chat);
        self.settings.set_visible(!chat);
        self.theme_button.set_visible(!chat);
        let (on, off) = if chat {
            (&self.mode_chat, &self.mode_code)
        } else {
            (&self.mode_code, &self.mode_chat)
        };
        on.add_css_class("active");
        off.remove_css_class("active");
    }

    pub fn set_chat_title(&self, title: &str) {
        self.chat_title.set_text(title);
        self.chat_title.set_tooltip_text(Some(title));
    }

    pub fn set_on_close(&self, callback: impl Fn(usize) + 'static) {
        *self.on_close.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_model_provider(&self, provider: &str) {
        if self.provider.borrow().as_str() == provider {
            return;
        }
        *self.provider.borrow_mut() = provider.to_string();
        paint_logo(&self.logo_slot, provider, self.theme.palette());
    }

    pub fn set_on_model(&self, callback: impl Fn(String, String, String) + 'static) {
        *self.on_model.borrow_mut() = Some(Box::new(callback));
    }

    pub fn open_models(&self) {
        if self.palette.is_open() {
            self.palette.close();
        } else {
            self.palette.open(380);
            self.palette.select_index(self.preferred.get());
        }
    }

    pub fn close_models(&self) {
        self.palette.close();
    }

    pub fn models_open(&self) -> bool {
        self.palette.is_open()
    }

    pub fn filter_models(&self, query: &str) {
        self.palette.search.set_text(query);
    }

    pub fn set_menu(&self, models: &[ModelItem], current_provider: &str, current_id: &str) {
        {
            let mut state = self.menu.borrow_mut();
            state.models = models.to_vec();
            state.provider = current_provider.to_string();
            state.id = current_id.to_string();
        }
        rebuild_menu(
            &self.menu.borrow(),
            &self.palette,
            &self.theme,
            &self.on_model,
            &self.preferred,
        );
        if self.palette.is_open() {
            self.palette.select_index(self.preferred.get());
        }
    }
}

fn rebuild_menu(
    state: &MenuState,
    palette: &Rc<Popup>,
    theme: &Theme,
    on_model: &Rc<RefCell<Option<ModelFn>>>,
    preferred: &Cell<usize>,
) {
    palette.clear();
    preferred.set(0);
    if state.models.is_empty() {
        palette.note(if palette.is_open() {
            "Loading..."
        } else {
            "no model — waiting for the Pi"
        });
        return;
    }
    let query = state.query.trim().to_lowercase();
    let filtered: Vec<&ModelItem> = state
        .models
        .iter()
        .filter(|model| {
            query.is_empty()
                || model.name.to_lowercase().contains(&query)
                || model.id.to_lowercase().contains(&query)
                || model.provider.to_lowercase().contains(&query)
        })
        .collect();
    if filtered.is_empty() {
        palette.note("no model with this filter");
        return;
    }

    let mut provider = String::new();
    for model in filtered {
        if model.provider != provider {
            provider = model.provider.clone();
            palette.heading(&provider);
        }
        let current = model.provider == state.provider && model.id == state.id;
        let name = gtk::Label::new(Some(&model.name));
        name.add_css_class("palette-name");
        name.set_xalign(0.0);

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);

        let id = gtk::Label::new(Some(&model.id));
        id.add_css_class("palette-model-id");
        id.set_xalign(1.0);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_hexpand(true);
        row.append(&logos::widget(&model.provider, theme.palette()));
        row.append(&name);
        row.append(&spacer);
        row.append(&id);

        if current {
            let mark = gtk::Label::new(Some("current"));
            mark.add_css_class("palette-current");
            mark.set_valign(gtk::Align::Center);
            row.append(&mark);
        }
        if current {
            preferred.set(palette.len());
        }

        let on_model = on_model.clone();
        let palette_c = palette.clone();
        let (provider, id, name) =
            (model.provider.clone(), model.id.clone(), model.name.clone());
        palette.add(&row, move || {
            palette_c.close();
            if let Some(callback) = on_model.borrow().as_ref() {
                callback(provider.clone(), id.clone(), name.clone());
            }
        });
    }
}
