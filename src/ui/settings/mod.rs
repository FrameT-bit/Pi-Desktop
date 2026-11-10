
mod connections;
mod credits;
mod mcp;
mod memory;
mod pages;
mod providers;
mod soul;

use std::cell::Cell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use gtk::prelude::*;

use crate::config::{self, Approve, Config, Target, Thinking};
use crate::state::ModelItem;
use crate::theme::{self, ThemeMode};

use self::pages::block;

pub type SaveFn = Rc<dyn Fn(Config, bool)>;

const MIN_CHAT_WIDTH: i32 = 60;
const MAX_CHAT_WIDTH: i32 = 120;
const MIN_FONT_SCALE: i32 = 90;
const MAX_FONT_SCALE: i32 = 130;

const TABS: &[(&str, &str)] = &[
    ("CONNECTION", "CONNECTION"),
    ("MODEL", "MODEL"),
    ("PROVIDERS", "PROVIDERS"),
    ("MCP", "MCP"),
    ("MEMORY", "MEMORY"),
    ("CONNECTIONS", "SEARCH"),
    ("SOUL", "PERSONALITY"),
    ("CREDITS", "CREDITS"),
];

const NAV: &[(&str, &[&str])] = &[
    ("GENERAL", &["CONNECTION", "MODEL"]),
    ("TOOLS", &["PROVIDERS", "MCP", "MEMORY", "SOUL"]),
    ("ADVANCED", &["CONNECTIONS"]),
    ("ABOUT", &["CREDITS"]),
];

fn tab_label(name: &str) -> &'static str {
    TABS.iter()
        .find(|(id, _)| *id == name)
        .map(|(_, label)| *label)
        .unwrap_or("?")
}

pub struct Form {
    pub controls: HashMap<&'static str, Control>,
}

pub enum Control {
    Entry(gtk::Entry),
    View(gtk::ScrolledWindow),
    Switch(gtk::Switch),
    Choice(gtk::DropDown),
}

impl Form {
    pub fn text(&self, key: &str) -> String {
        match self.controls.get(key) {
            Some(Control::Entry(entry)) => entry.text().to_string(),
            Some(Control::View(scroll)) => pages::view_text(scroll),
            _ => String::new(),
        }
    }

    pub fn boolean(&self, key: &str) -> bool {
        match self.controls.get(key) {
            Some(Control::Switch(switch)) => switch.state(),
            _ => false,
        }
    }

    pub fn index(&self, key: &str) -> usize {
        match self.controls.get(key) {
            Some(Control::Choice(dropdown)) => dropdown.selected() as usize,
            _ => 0,
        }
    }

    pub fn choice(&self, key: &str) -> &'static str {
        match self.controls.get(key) {
            Some(Control::Choice(dropdown)) => {
                let index = dropdown.selected() as usize;
                pages::choice_value(key, index)
            }
            _ => "",
        }
    }

    pub fn int(&self, key: &str) -> Option<i32> {
        self.text(key).trim().parse::<i32>().ok()
    }
}

pub fn show(
    parent: &impl IsA<gtk::Window>,
    config: Config,
    aux_models: Vec<ModelItem>,
    on_save: impl Fn(Config, bool) + 'static,
    on_provider_change: impl Fn() + 'static,
    initial_tab: Option<String>,
) {
    let window = gtk::Window::new();
    window.set_transient_for(Some(parent));
    window.set_modal(true);
    window.set_destroy_with_parent(true);
    window.set_decorated(false);
    window.set_resizable(true);
    window.add_css_class("pi-settings");
    let height = std::env::var("PI_DESKTOP_SETTINGS_HEIGHT")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value >= 400)
        .unwrap_or(720);
    crate::ui::fit::fit(&window, 1000, height);
    if let Some(app) = parent.as_ref().application() {
        window.set_application(Some(&app));
    }
    window.set_title(Some("settings"));

    let config_before = config.clone();

    if let Ok(name) = std::env::var("PI_DESKTOP_SETTINGS_PROVIDER_ADD") {
        seed_provider(name.trim());
    }


    let title = gtk::Label::new(Some("SETTINGS"));
    title.add_css_class("settings-title");
    title.set_xalign(0.0);
    title.set_hexpand(true);

    let controls = gtk::WindowControls::new(gtk::PackType::End);
    controls.add_css_class("window-controls");
    controls.set_valign(gtk::Align::Center);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("settings-header");
    header.append(&title);
    header.append(&controls);


    let mut form = Form {
        controls: HashMap::new(),
    };


    let stack = gtk::Stack::new();
    stack.add_css_class("settings-stack");
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::None);

    let (connections_page, save_connections) = connections::page();

    for (name, _) in TABS {
        let content = match *name {
            "CONNECTION" => connection_page(&config, &mut form),
            "MODEL" => model_page(&config, &mut form, &aux_models),
            "PROVIDERS" => providers::page(),
            "MCP" => mcp::page(&window, &config.bin),
            "MEMORY" => memory::page(),
            "CONNECTIONS" => connections_page.clone(),
            "SOUL" => soul::page(&config, &mut form),
            "CREDITS" => credits::page(),
            _ => pages::blank(),
        };
        let page = tab_shell(*name, &content);
        stack.add_named(&page, Some(*name));
    }

    let nav = gtk::Box::new(gtk::Orientation::Vertical, 0);
    nav.add_css_class("settings-nav");
    nav.set_size_request(210, -1);
    nav.set_vexpand(true);

    let mut toggles: Vec<(String, gtk::ToggleButton)> = Vec::new();
    let mut group: Option<gtk::ToggleButton> = None;
    for (group_label, names) in NAV {
        let heading = gtk::Label::new(Some(group_label));
        heading.add_css_class("settings-nav-group");
        heading.set_xalign(0.0);
        nav.append(&heading);
        for name in *names {
            let toggle = gtk::ToggleButton::new();
            toggle.add_css_class("settings-nav-item");
            toggle.set_hexpand(true);
            toggle.set_halign(gtk::Align::Fill);
            toggle.set_focus_on_click(false);
            let label = gtk::Label::new(Some(tab_label(name)));
            label.add_css_class("settings-nav-label");
            label.set_xalign(0.0);
            label.set_hexpand(true);
            toggle.set_child(Some(&label));
            match &group {
                Some(first) => toggle.set_group(Some(first)),
                None => group = Some(toggle.clone()),
            }
            nav.append(&toggle);
            toggles.push(((*name).to_string(), toggle));
        }
    }

    let nav_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    nav_spacer.set_vexpand(true);
    nav.append(&nav_spacer);
    let open = gtk::Button::with_label("open config.ini");
    open.add_css_class("settings-nav-link");
    open.set_halign(gtk::Align::Fill);
    nav.append(&open);

    for (name, toggle) in &toggles {
        let stack = stack.clone();
        let toggles = toggles.clone();
        let name = name.clone();
        toggle.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            stack.set_visible_child_name(&name);
            for (other, widget) in &toggles {
                if *other == name {
                    widget.add_css_class("active");
                } else {
                    widget.remove_css_class("active");
                }
            }
        });
    }

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.add_css_class("settings-body-shell");
    body.set_hexpand(true);
    body.set_vexpand(true);
    body.append(&nav);
    body.append(&stack);


    let error = gtk::Label::new(None);
    error.add_css_class("settings-error");
    error.set_xalign(0.0);
    error.set_visible(false);

    let cancel = gtk::Button::with_label("cancel");
    cancel.add_css_class("dialog-cancel");
    cancel.set_valign(gtk::Align::Center);

    let save = gtk::Button::with_label("save");
    save.add_css_class("settings-save");
    save.set_valign(gtk::Align::Center);

    save.set_receives_default(true);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.add_css_class("settings-footer");
    footer.append(&cancel);
    footer.append(&error);
    footer.append(&spacer);
    footer.append(&save);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("pi-settings");
    root.append(&header);
    root.append(&body);
    root.append(&footer);
    window.set_child(Some(&root));


    let form = Rc::new(form);
    let on_save: SaveFn = Rc::new(on_save);
    let provider_before = Rc::new(Cell::new(provider_fingerprint()));
    let saved_reconnect = Rc::new(Cell::new(false));

    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }

    {
        let error = error.clone();
        open.connect_clicked(move |_| {
            let Some(path) = config::config_path() else {
                error.set_text("I couldn't solve the path of config.ini");
                error.set_visible(true);
                return;
            };
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if !path.exists() {
                if let Err(err) = config::write_atomic(&path, "") {
                    error.set_text(&format!("failure to create config.ini: {err}"));
                    error.set_visible(true);
                    return;
                }
            }
            match std::process::Command::new("xdg-open").arg(&path).spawn() {
                Ok(_) => {
                    error.set_visible(false);
                }
                Err(err) => {
                    error.set_text(&format!("failure to open file: {err}"));
                    error.set_visible(true);
                }
            }
        });
    }

    let config_before = Rc::new(config_before);
    let commit: Rc<dyn Fn()> = {
        let form = form.clone();
        let error = error.clone();
        let window = window.clone();
        let on_save = on_save.clone();
        let saved_reconnect = saved_reconnect.clone();
        let config_before = config_before.clone();
        let save_connections = save_connections.clone();
        Rc::new(move || {
            if let Err(message) = save_connections() {
                error.set_text(&message);
                error.set_visible(true);
                return;
            }
            let config = match collect(&form) {
                Ok(config) => config,
                Err(message) => {
                    error.set_text(&message);
                    error.set_visible(true);
                    return;
                }
            };
            if let Err(err) = config.save() {
                error.set_text(&format!("failure to record config.ini: {err}"));
                error.set_visible(true);
                return;
            }
            let mode = ThemeMode::from_index(form.index("theme"));
            theme::save_preference(mode);
            error.set_visible(false);
            if config_before.needs_reconnect(&config) {
                saved_reconnect.set(true);
            }
            on_save(config, false);
            window.close();
        })
    };

    {
        let commit = commit.clone();
        save.connect_clicked(move |_| commit());
    }

    {
        let on_provider_change: Rc<dyn Fn()> = Rc::new(on_provider_change);
        let provider_before = provider_before.clone();
        let saved_reconnect = saved_reconnect.clone();
        window.connect_close_request(move |_| {
            if !saved_reconnect.get() && provider_fingerprint() != provider_before.get() {
                on_provider_change();
            }
            glib::Propagation::Proceed
        });
    }

    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    let window_c = window.clone();
    key.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            window_c.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key);

    let initial = initial_tab
        .as_deref()
        .and_then(tab_name)
        .or_else(|| {
            std::env::var("PI_DESKTOP_SETTINGS_TAB")
                .ok()
                .and_then(|value| tab_name(&value))
        })
        .unwrap_or("CONNECTION");
    if let Some((_, toggle)) = toggles.iter().find(|(name, _)| name == initial) {
        toggle.set_active(true);
    }

    window.present();

    if std::env::var("PI_DESKTOP_SETTINGS_SAVE").is_ok() {
        let commit = commit.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
            commit();
        });
    }
}

fn provider_fingerprint() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    for name in ["auth.json", "models.json", "mcp.json", "mcp-auth.json"] {
        let path = config::pi_agent_dir().join(name);
        if let Ok(bytes) = std::fs::read(path) {
            name.hash(&mut hasher);
            bytes.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn tab_name(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "connection" => Some("CONNECTION"),
        "model" => Some("MODEL"),
        "providers" | "provider" => Some("PROVIDERS"),
        "mcp" => Some("MCP"),
        "connections" | "connection_search" => Some("CONNECTIONS"),
        "memoria" | "memory" => Some("MEMORY"),
        "soul" => Some("SOUL"),
        "credits" | "about" => Some("CREDITS"),
        _ => None,
    }
}

fn tab_shell(name: &str, content: &gtk::Box) -> gtk::ScrolledWindow {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-tab-page");

    let title = gtk::Label::new(Some(tab_title(name)));
    title.add_css_class("page-title");
    title.set_xalign(0.0);
    title.set_wrap(true);

    let desc = gtk::Label::new(Some(tab_desc(name)));
    desc.add_css_class("page-desc");
    desc.set_xalign(0.0);
    desc.set_wrap(true);

    page.append(&title);
    page.append(&desc);
    page.append(content);

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .child(&page)
        .build()
}

fn tab_title(name: &str) -> &'static str {
    match name {
        "CONNECTION" => "Connection and interface",
        "MODEL" => "Model",
        "PROVIDERS" => "Providers and models",
        "MCP" => "MCP servers",
        "CONNECTIONS" => "Search connections",
        "MEMORY" => "Memory between sessions",
        "SOUL" => "Personality",
        "CREDITS" => "Credits",
        _ => "Settings",
    }
}

fn tab_desc(name: &str) -> &'static str {
    match name {
        "CONNECTION" => "Where Pi runs and which session to open. Advanced options are below.",
        "MODEL" => "Model, provider and reasoning; scope and tools are under Advanced.",
        "PROVIDERS" => "Connect a provider, inspect the models and choose the default without editing JSON.",
        "MCP" => "Global MCP servers; the list comes from Pi itself.",
        "CONNECTIONS" => "Search providers used by the web access extension.",
        "MEMORY" => "Facts and preferences that pi-memory injects into the prompt.",
        "SOUL" => "Pi has no SOUL: the persona goes into the system prompt.",
        "CREDITS" => "Version, updates, links and licenses.",
        _ => "",
    }
}


fn advanced(parent: &gtk::Box, title: &str) -> gtk::Box {
    let expander = gtk::Expander::new(Some(title));
    expander.add_css_class("settings-advanced");
    expander.set_expanded(false);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    expander.set_child(Some(&body));
    parent.append(&expander);
    body
}

fn connection_page(config: &Config, form: &mut Form) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");

    let target = block(&page, "TARGET");
    let rows = pages::connection_target(config, form);
    target.append(&rows);

    let session = block(&page, "SESSION");
    session.append(&pages::connection_session(config, form));

    let theme = block(&page, "THEME");
    theme.append(&pages::interface_theme(form));

    let adv = advanced(&page, "advanced · session, trust, network and interface");
    let sessao_extra = block(&adv, "SESSION");
    sessao_extra.append(&pages::connection_session_advanced(config, form));
    let trust = block(&adv, "TRUST AND NETWORK");
    trust.append(&pages::connection_trust(config, form));
    let interface = block(&adv, "INTERFACE");
    interface.append(&pages::interface(config, form));

    page
}

fn model_page(config: &Config, form: &mut Form, aux_models: &[ModelItem]) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");

    let default_model = block(&page, "DEFAULT MODEL");
    default_model.append(&pages::main_model(config, form, aux_models));

    let adv = advanced(&page, "advanced · scope of Ctrl+P, tools and context");
    let cycle = block(&adv, "CTRL+P CYCLE");
    cycle.append(&pages::model_cycle(config, form, &config::pi_agent_dir()));
    let tools = block(&adv, "TOOLS");
    tools.append(&pages::model_tools(config, form));
    let context = block(&adv, "EXTENSIONS AND CONTEXT");
    context.append(&pages::model_context(config, form));

    page
}

fn seed_provider(name: &str) {
    use crate::providers::{self, Model, Provider};
    if name.is_empty() {
        return;
    }
    let agent = config::pi_agent_dir();
    let mut store = providers::Providers::load(&agent);
    let provider = Provider {
        name: name.to_string(),
        base_url: "http://127.0.0.1:8091/v1".to_string(),
        api: "openai-completions".to_string(),
        api_key: "sk-demo-seed-9876".to_string(),
        compat_developer: false,
        compat_reasoning: false,
        models: vec![Model {
            id: format!("{name}-model"),
            name: format!("{name} demo"),
            reasoning: false,
            context_window: 65536,
            max_tokens: 4096,
        }],
    };
    if let Err(message) = providers::validate(&provider) {
        eprintln!("seed provider: {message}");
        return;
    }
    store.upsert(&provider);
    match store.save_atomic(&agent) {
        Ok(()) => eprintln!("seed provider: {name} recorded in {}", agent.display()),
        Err(err) => eprintln!("seed provider: recording failure: {err}"),
    }
}


fn collect(form: &Form) -> Result<Config, String> {
    let workspace = form.text("workspace").trim().to_string();
    if workspace.is_empty() {
        return Err("empty workspace".to_string());
    }
    if !config::expand_home(&workspace).is_dir() {
        return Err(format!("workspace does not exist: {workspace}"));
    }

    let session_id = form.text("session_id").trim().to_string();
    if !session_id.is_empty()
        && !session_id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err("session id: use only [a-z0-9-]".to_string());
    }

    let chat_width = form
        .int("chat_width")
        .ok_or_else(|| "prose width: number between 60 and 120".to_string())?;
    if !(MIN_CHAT_WIDTH..=MAX_CHAT_WIDTH).contains(&chat_width) {
        return Err(format!(
            "prose width: {chat_width} outside range 60..120"
        ));
    }

    let font_scale = form
        .int("font_scale")
        .ok_or_else(|| "source scales: number between 90 and 130".to_string())?;
    if !(MIN_FONT_SCALE..=MAX_FONT_SCALE).contains(&font_scale) {
        return Err(format!(
            "source scale: {font_scale} outside range 90..130"
        ));
    }

    if form.choice("target") == "remote" && form.text("remote_ssh").trim().is_empty() {
        return Err("remote target requires ssh (user@host)".to_string());
    }

    let mut config = Config::default();
    config.target = if form.choice("target") == "remote" {
        Target::Remote
    } else {
        Target::Local
    };
    config.bin = form.text("bin");
    config.remote_ssh = form.text("remote_ssh");
    config.remote_cmd = form.text("remote_cmd");
    config.workspace = workspace;
    config.session_id = session_id;
    config.session_dir = form.text("session_dir");
    config.no_session = form.boolean("no_session");
    config.start_last = form.boolean("start_last");
    config.name = form.text("name");
    config.model = form.text("model");
    config.aux_model = form.text("aux_model");
    config.models = form.text("models");
    config.provider = form.text("provider");
    config.thinking = Thinking::from_index(form.index("thinking"));
    config.tools = form.text("tools");
    config.exclude_tools = form.text("exclude_tools");
    config.no_tools = form.boolean("no_tools");
    config.no_builtin_tools = form.boolean("no_builtin_tools");
    config.no_mcp = form.boolean("no_mcp");
    config.approve = Approve::from_index(form.index("approve"));
    config.offline = form.boolean("offline");
    config.skills = form.text("skills");
    config.no_skills = form.boolean("no_skills");
    config.extensions = form.text("extensions");
    config.no_extensions = form.boolean("no_extensions");
    config.prompt_templates = form.text("prompt_templates");
    config.no_prompt_templates = form.boolean("no_prompt_templates");
    config.no_context_files = form.boolean("no_context_files");
    config.system_prompt = if form.boolean("soul_replace") {
        form.text("system_prompt")
    } else {
        String::new()
    };
    config.append_system_prompt = form.text("append_system_prompt");
    config.timestamps = form.boolean("timestamps");
    config.auto_scroll = form.boolean("auto_scroll");
    config.chat_width = chat_width;
    config.font_scale = font_scale;
    config.show_costs = form.boolean("show_costs");
    Ok(config)
}
