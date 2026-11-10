
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::config::{self, Config};
use crate::providers::{self, AuthStore, CliModel, Credential, Provider};
use crate::state::resolve_pi_bin;
use crate::ui::dialog;
use crate::ui::settings::pages::{block, dropdown, row, text_entry};

#[derive(Clone)]
struct Ui {
    window: gtk::Window,
    bin: String,
    connected: gtk::Box,
    status: gtk::Label,
    provider: gtk::DropDown,
    key: gtk::Entry,
    hint: gtk::Label,
    connect: gtk::Button,
    models_block: gtk::Box,
    models_title: gtk::Label,
    models_list: gtk::Box,
    custom_block: gtk::Box,
    custom_name: gtk::Entry,
    custom_url: gtk::Entry,
    custom_api: gtk::DropDown,
    custom_key: gtk::Entry,
    custom_models: gtk::TextView,
    custom_status: gtk::Label,
    custom_default: gtk::Entry,
    models_provider: RefCell<String>,
    connecting: Cell<bool>,
    discover: gtk::Button,
    discovering: Cell<bool>,
}

const CUSTOM_LABEL: &str = "Custom Endpoint...";

struct ConnectOutcome {
    check: Result<(), String>,
    models: Result<Vec<CliModel>, String>,
}

fn first_line(text: &str, max: usize) -> String {
    let joined = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join(" · ");
    joined.chars().take(max).collect()
}

fn selected_id(ui: &Ui) -> String {
    let index = ui.provider.selected() as usize;
    if index < providers::KNOWN.len() {
        providers::KNOWN[index].id.to_string()
    } else {
        String::new() // endpoint personalizado
    }
}

impl Ui {
    fn error(&self, message: &str) {
        self.status.remove_css_class("settings-ok");
        self.status.add_css_class("settings-error");
        self.status.set_text(message);
        self.status.set_visible(true);
    }

    fn connect_finished(&self) {
        self.connecting.set(false);
        self.connect.set_sensitive(true);
        self.connect.set_label("connect");
        self.provider.set_sensitive(true);
        self.key.set_sensitive(true);
        self.refresh_hint();
    }

    fn note(&self, message: &str) {
        self.status.remove_css_class("settings-error");
        self.status.add_css_class("settings-ok");
        self.status.set_text(message);
        self.status.set_visible(true);
    }

    fn clear(&self) {
        self.status.set_visible(false);
    }

    fn refresh_hint(&self) {
        if selected_id(self).is_empty() {
            self.hint.set_text(
                "own endpoint: inform URL and API; the key is optional on local servers",
            );
            self.key.set_sensitive(false);
            self.connect.set_visible(false);
            self.custom_block.set_visible(true);
            return;
        }
        let Some(known) = providers::known(&selected_id(self)) else {
            return;
        };
        self.hint
            .set_text(&format!("{} · variable {}", known.hint, known.env));
        self.key.set_sensitive(true);
        self.connect.set_visible(true);
        self.connect.set_label("connect");
        self.custom_block.set_visible(false);
    }

    fn refresh(self: &Rc<Self>) {
        let agent = config::pi_agent_dir();
        let keys = providers::ProviderKeys::load(&agent);
        let mut auth = AuthStore::load(&agent);
        let mut migrated = false;
        for name in keys.providers() {
            if !matches!(auth.credential(&name), Credential::None) {
                continue;
            }
            if let Some(active) = keys.active(&name) {
                auth.set_api_key(&name, &active);
                migrated = true;
            }
        }
        if migrated {
            let _ = auth.save_atomic(&agent);
        }
        match providers::repair_models_store(&agent) {
            Ok(Some(message)) => self.note(&message),
            Ok(None) => {}
            Err(message) => self.error(&message),
        }

        while let Some(child) = self.connected.first_child() {
            self.connected.remove(&child);
        }

        let store = providers::Providers::load(&agent);
        let mut names: Vec<String> = auth.providers();
        for name in store.names() {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if names.is_empty() {
            self.models_block.set_visible(false);
            let empty = gtk::Label::new(Some(
                "no connected provider — choose one below and paste the key",
            ));
            empty.add_css_class("settings-empty");
            empty.set_xalign(0.0);
            self.connected.append(&empty);
            return;
        }

        for name in names {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            line.add_css_class("settings-item");

            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("settings-dot");
            dot.add_css_class("unknown");
            check_auth(&self.bin, &name, &dot);

            let info = gtk::Box::new(gtk::Orientation::Vertical, 1);
            info.set_hexpand(true);
            let title = gtk::Label::new(Some(
                providers::known(&name).map(|known| known.label).unwrap_or(&name),
            ));
            title.add_css_class("settings-item-titles");
            title.set_xalign(0.0);

            let detail_text = match auth.credential(&name) {
                Credential::ApiKey(key) => format!("{name} · key {}", providers::mask_key(&key)),
                Credential::Other(kind) => {
                    format!("{name} · {kind} (remove in pi with /logout)")
                }
                Credential::None => {
                    let count = store.provider(&name).map(|p| p.models.len()).unwrap_or(0);
                    format!("{name} · custom endpoint · {count} model(s)")
                }
            };
            let detail = gtk::Label::new(Some(&detail_text));
            detail.add_css_class("settings-item-dec");
            detail.set_xalign(0.0);
            detail.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            info.append(&title);
            info.append(&detail);

            let models_button = gtk::Button::with_label("models");
            models_button.add_css_class("settings-mini");
            let remove_button = gtk::Button::with_label("remove");
            remove_button.add_css_class("settings-mini");
            remove_button.add_css_class("danger");
            if store.provider(&name).is_some() {
                remove_button.set_tooltip_text(Some("remove the endpoint from models.json"));
            } else if matches!(auth.credential(&name), Credential::Other(_)) {
                remove_button.set_sensitive(false);
                remove_button.set_tooltip_text(Some("OAuth credentials: remove in pi with /logout"));
            }

            line.append(&dot);
            line.append(&info);
            line.append(&models_button);
            line.append(&remove_button);
            self.connected.append(&line);

            {
                let ui = self.clone();
                let name_c = name.clone();
                models_button.connect_clicked(move |_| ui.show_models(&name_c));
            }
            {
                let ui = self.clone();
                let name_c = name.clone();
                let is_custom = store.provider(&name).is_some();
                remove_button.connect_clicked(move |_| {
                    let ui_c = ui.clone();
                    let name_c = name_c.clone();
                    let detail = if is_custom {
                        "the key in auth.json and the endpoint in models.json"
                    } else {
                        "the key in auth.json"
                    };
                    dialog::confirm(
                        &ui.window,
                        "Disconnect provider?",
                        &format!("{name_c}\n\nremover {detail}"),
                        "remove",
                        move || {
                            let agent = config::pi_agent_dir();
                            let mut auth = AuthStore::load(&agent);
                            let had_key = auth.remove_api_key(&name_c);
                            let mut store = providers::Providers::load(&agent);
                            let had_endpoint = store.remove(&name_c);
                            if !had_key && !had_endpoint {
                                ui_c.error("nothing to remove");
                                return;
                            }
                            let mut result = Ok(());
                            if had_key {
                                result = auth.save_atomic(&agent);
                            }
                            if result.is_ok() && had_endpoint {
                                result = store.save_atomic(&agent);
                            }
                            match result {
                                Ok(()) => {
                                    ui_c.clear();
                                    ui_c.models_block.set_visible(false);
                                    ui_c.refresh();
                                }
                                Err(err) => ui_c.error(&format!("failed to save: {err}")),
                            }
                        },
                    );
                });
            }
        }
    }

    fn show_models(self: &Rc<Self>, provider: &str) {
        *self.models_provider.borrow_mut() = provider.to_string();
        let label = providers::known(provider)
            .map(|known| known.label)
            .unwrap_or(provider);
        self.models_title
            .set_text(&format!("MODELS FOR {label}"));
        self.models_block.set_visible(true);
        while let Some(child) = self.models_list.first_child() {
            self.models_list.remove(&child);
        }
        let loading = gtk::Label::new(Some("Reading models..."));
        loading.add_css_class("settings-empty");
        loading.set_xalign(0.0);
        self.models_list.append(&loading);

        let agent = config::pi_agent_dir();
        let (default_provider, default_model) = providers::default_model(&agent);
        let custom = providers::Providers::load(&agent).provider(provider);

        let ui = self.clone();
        if let Some(custom) = custom {
            let models: Vec<CliModel> = custom
                .models
                .iter()
                .map(|model| CliModel {
                    provider: custom.name.clone(),
                    id: model.id.clone(),
                    context: format!("{}K", model.context_window / 1000),
                    max_out: format!("{}K", model.max_tokens / 1000),
                    thinking: model.reasoning,
                    images: false,
                })
                .collect();
            ui.render_models(models, default_provider, default_model);
            return;
        }

        let bin = self.bin.clone();
        let name = provider.to_string();
        let (tx, rx) = async_channel::bounded::<Result<Vec<CliModel>, String>>(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(providers::cli_models(&bin, &name));
        });
        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else {
                return;
            };
            match result {
                Ok(models) => ui.render_models(models, default_provider, default_model),
                Err(err) => ui.error(&err),
            }
        });
    }

    fn render_models(
        self: &Rc<Self>,
        models: Vec<CliModel>,
        default_provider: Option<String>,
        default_model: Option<String>,
    ) {
        while let Some(child) = self.models_list.first_child() {
            self.models_list.remove(&child);
        }
        if models.is_empty() {
            let empty = gtk::Label::new(Some("the provider returned no models"));
            empty.add_css_class("settings-empty");
            empty.set_xalign(0.0);
            self.models_list.append(&empty);
            return;
        }

        let provider = self.models_provider.borrow().clone();
        let count = gtk::Label::new(Some(&format!("{} model(s)", models.len())));
        count.add_css_class("settings-empty");
        count.set_xalign(0.0);
        self.models_list.append(&count);

        for model in models {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            line.add_css_class("settings-item");

            let info = gtk::Box::new(gtk::Orientation::Vertical, 1);
            info.set_hexpand(true);
            let title = gtk::Label::new(Some(&model.id));
            title.add_css_class("settings-item-titles");
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);

            let mut tags = vec![format!("ctx {}", model.context), format!("out {}", model.max_out)];
            if model.thinking {
                tags.push("thinking".to_string());
            }
            if model.images {
                tags.push("images".to_string());
            }
            let detail = gtk::Label::new(Some(&tags.join(" · ")));
            detail.add_css_class("settings-item-dec");
            detail.set_xalign(0.0);
            info.append(&title);
            info.append(&detail);

            let is_default = default_provider.as_deref() == Some(provider.as_str())
                && default_model.as_deref() == Some(model.id.as_str());
            let use_button = gtk::Button::with_label(if is_default { "standard" } else { "use" });
            use_button.add_css_class("settings-mini");
            if is_default {
                use_button.set_sensitive(false);
                use_button.set_tooltip_text(Some("already the default for new sessions"));
            } else {
                use_button.set_tooltip_text(Some("default for the next sessions"));
            }

            line.append(&info);
            line.append(&use_button);
            self.models_list.append(&line);

            if !is_default {
                let ui = self.clone();
                let provider_c = provider.clone();
                let model_c = model.id.clone();
                use_button.connect_clicked(move |_| {
                    let agent = config::pi_agent_dir();
                    match providers::set_default_model(&agent, &provider_c, &model_c) {
                        Ok(()) => {
                            ui.note(&format!(
                                "standard: {provider_c}/{model_c} (new sessions)"
                            ));
                            ui.show_models(&provider_c);
                        }
                        Err(err) => ui.error(&format!("failed to save settings.json: {err}")),
                    }
                });
            }
        }
    }

    fn connect_provider(self: &Rc<Self>) {
        if self.connecting.get() {
            return;
        }
        if selected_id(self).is_empty() {
            self.custom_block.set_visible(true);
            return;
        }
        let provider = selected_id(self);
        let key = self.key.text().trim().to_string();
        if key.is_empty() {
            self.error("paste the provider's API key (the variable environment also works, but is not read here)");
            return;
        }
        self.connecting.set(true);
        self.provider.set_sensitive(false);
        self.key.set_sensitive(false);
        self.connect.set_sensitive(false);
        self.connect.set_label("Connecting...");
        let agent = config::pi_agent_dir();
        let mut auth = AuthStore::load(&agent);
        auth.set_api_key(&provider, &key);
        if let Err(err) = auth.save_atomic(&agent) {
            self.connect_finished();
            self.error(&format!("failed to save auth.json: {err}"));
            return;
        }
        self.key.set_text("");
        self.note("checking credential with more...");

        let bin = self.bin.clone();
        let name = provider.clone();
        let (tx, rx) = async_channel::bounded::<ConnectOutcome>(1);
        std::thread::spawn(move || {
            let check = (|| {
                let path = resolve_pi_bin(&bin)?;
                let output = std::process::Command::new(path)
                    .args(["auth", "check", "--provider", &name, "--json", "--no-refresh"])
                    .env("PI_CODING_AGENT_DIR", config::pi_agent_dir())
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map_err(|err| format!("failed to run pi: {err}"))?;
                if output.status.success() {
                    Ok(())
                } else {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let detail = [stdout.trim(), stderr.trim()]
                        .into_iter()
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                    Err(detail)
                }
            })();
            let models = providers::cli_models(&bin, &name);
            let _ = tx.send_blocking(ConnectOutcome { check, models });
        });
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let Ok(outcome) = rx.recv().await else {
                ui.connect_finished();
                return;
            };
            ui.connect_finished();
            let label = providers::known(&provider)
                .map(|known| known.label.to_string())
                .unwrap_or_else(|| provider.clone());
            match (outcome.check, outcome.models) {
                (check, Ok(models)) if !models.is_empty() => {
                    let warning = match check {
                        Ok(()) => String::new(),
                        Err(detail) => format!(
                            "· more warning: {}",
                            first_line(&detail, 200)
                        ),
                    };
                    ui.note(&format!(
                        "{label} connected · {} model(s) · applies on close{warning}",
                        models.len()
                    ));
                    ui.refresh();
                    ui.show_models(&provider);
                }
                (Ok(()), Ok(_)) => {
                    ui.error(&format!(
                        "{label}: credential accepted, but the provider did not list models"
                    ));
                }
                (Ok(()), Err(err)) => {
                    ui.error(&format!(
                        "key saved in auth.json, but the list of models failed: {}",
                        first_line(&err, 400)
                    ));
                }
                (Err(detail), models) => {
                    let hint = providers::known(&provider)
                        .map(|known| known.hint)
                        .unwrap_or("the documentation of the provider");
                    let mut message = format!(
                        "the pi refused `{provider}`: {} — check the key in {hint}",
                        first_line(&detail, 400)
                    );
                    if let Err(err) = models {
                        message.push_str(&format!("· models: {}", first_line(&err, 200)));
                    }
                    ui.error(&message);
                }
            }
        });
    }

    fn discover_custom(self: &Rc<Self>) {
        if self.discovering.get() {
            return;
        }
        self.discovering.set(true);
        self.discover.set_sensitive(false);
        let url = self.custom_url.text().trim().to_string();
        let api = providers::APIS[self.custom_api.selected() as usize].to_string();
        let key = self.custom_key.text().trim().to_string();
        self.custom_status.set_text("querying /models...");
        self.custom_status.set_visible(true);

        let (tx, rx) = async_channel::bounded::<Result<Vec<String>, String>>(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(providers::discover_models(&url, &api, &key));
        });
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else {
                ui.discovering.set(false);
                ui.discover.set_sensitive(true);
                return;
            };
            ui.discovering.set(false);
            ui.discover.set_sensitive(true);
            match result {
                Ok(models) => {
                    ui.custom_status
                        .set_text(&format!("{} model(s) found", models.len()));
                    let buffer = ui.custom_models.buffer();
                    buffer.set_text(&models.join("\n"));
                    if ui.custom_default.text().trim().is_empty() {
                        if let Some(first) = models.first() {
                            ui.custom_default.set_text(first);
                        }
                    }
                }
                Err(err) => ui.custom_status.set_text(&format!("failure: {err}")),
            }
        });
    }

    fn save_custom(self: &Rc<Self>) {
        let name = self.custom_name.text().trim().to_string();
        let base_url = self.custom_url.text().trim().to_string();
        let api = providers::APIS[self.custom_api.selected() as usize].to_string();
        let key = self.custom_key.text().trim().to_string();
        let buffer = self.custom_models.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        let ids: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        if ids.is_empty() {
            self.error("discover the models or write at least one id (one per line)");
            return;
        }
        let default = self.custom_default.text().trim().to_string();
        if !default.is_empty() && !ids.contains(&default.as_str()) {
            self.error("the standard model needs to be in the list of models above");
            return;
        }
        let mut provider = Provider::new(&name);
        provider.base_url = base_url;
        provider.api = api;
        provider.api_key = key;
        provider.models = ids
            .iter()
            .map(|id| providers::Model {
                id: (*id).to_string(),
                name: (*id).to_string(),
                ..providers::Model::default()
            })
            .collect();
        if let Err(message) = providers::validate(&provider) {
            self.error(&message);
            return;
        }

        let agent = config::pi_agent_dir();
        let mut store = providers::Providers::load(&agent);
        store.upsert(&provider);
        if let Err(err) = store.save_atomic(&agent) {
            self.error(&format!("failed to save models.json: {err}"));
            return;
        }
        if !default.is_empty() {
            let _ = providers::set_default_model(&agent, &name, &default);
        }
        self.note(&format!(
            "endpoint {name} except in models.json · {} model(s) · apps when closing",
            provider.models.len()
        ));
        self.custom_block.set_visible(false);
        self.refresh();
        self.show_models(&name);
    }
}

pub fn page() -> gtk::Box {
    let config = Config::load();
    let bin = config.bin.clone();
    let window = settings_window().unwrap_or_else(gtk::Window::new);
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");

    let status = gtk::Label::new(None);
    status.add_css_class("settings-error");
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.set_visible(false);
    page.append(&status);

    let connected = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let connected_block = block(&page, "CONNECTED");
    connected_block.append(&connected);

    let labels: Vec<&'static str> = providers::KNOWN
        .iter()
        .map(|known| known.label)
        .chain(std::iter::once(CUSTOM_LABEL))
        .collect();
    let labels: &'static [&'static str] = Box::leak(labels.into_boxed_slice());
    let provider = dropdown(labels, labels, 0);
    provider.set_enable_search(true);

    let key = text_entry("", 360);
    key.set_visibility(false);
    key.set_placeholder_text(Some("paste API key"));

    let hint = gtk::Label::new(None);
    hint.add_css_class("settings-item-dec");
    hint.set_xalign(0.0);
    hint.set_wrap(true);

    let connect = gtk::Button::with_label("connect");
    connect.add_css_class("settings-mini");

    let connect_block = block(&page, "CONNECT PROVIDER");
    connect_block.append(&row("provider", "Pi catalog; key goes to auth.json", &provider));
    connect_block.append(&row("API key", "auth.json, 0600; Pi reads it at boot", &key));
    connect_block.append(&hint);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.add_css_class("settings-actions");
    actions.append(&connect);
    connect_block.append(&actions);

    let models_block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    models_block.set_visible(false);
    let models_title = gtk::Label::new(None);
    models_title.add_css_class("settings-block-label");
    models_title.set_xalign(0.0);
    let models_note = gtk::Label::new(Some(
        "the default is for the next sessions; new providers are applied when this window closes (the session reconnects)",
    ));
    models_note.add_css_class("settings-item-dec");
    models_note.set_xalign(0.0);
    models_note.set_wrap(true);
    let models_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let models_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .max_content_height(300)
        .propagate_natural_height(true)
        .child(&models_list)
        .build();
    models_scroll.add_css_class("settings-view");
    models_block.append(&models_title);
    models_block.append(&models_note);
    models_block.append(&models_scroll);
    page.append(&models_block);

    let custom_block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    custom_block.set_visible(false);
    let custom_title = gtk::Label::new(Some("CUSTOM ENDPOINT"));
    custom_title.add_css_class("settings-block-label");
    custom_title.set_xalign(0.0);
    custom_block.append(&custom_title);
    let custom_name = text_entry("", 200);
    custom_name.set_placeholder_text(Some("my-proxy"));
    let custom_url = text_entry("", 360);
    custom_url.set_placeholder_text(Some("http://127.0.0.1:8080/v1"));
    let custom_api = dropdown(providers::APIS, providers::APIS, 0);
    let custom_key = text_entry("", 360);
    custom_key.set_visibility(false);
    custom_key.set_placeholder_text(Some("optional on local server"));
    let custom_default = text_entry("", 240);
    custom_default.set_placeholder_text(Some("default model (optional)"));
    let custom_models = gtk::TextView::new();
    custom_models.add_css_class("settings-editor");
    custom_models.set_monospace(true);
    let custom_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(110)
        .child(&custom_models)
        .build();
    custom_scroll.add_css_class("settings-view");
    let custom_status = gtk::Label::new(None);
    custom_status.add_css_class("settings-item-dec");
    custom_status.set_xalign(0.0);
    custom_status.set_visible(false);

    let discover = gtk::Button::with_label("Discover models");
    discover.add_css_class("settings-mini");
    let save = gtk::Button::with_label("save endpoint");
    save.add_css_class("settings-mini");
    let custom_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    custom_actions.add_css_class("settings-actions");
    custom_actions.append(&discover);
    custom_actions.append(&save);

    custom_block.append(&row("name / id", "no spaces; is the provider id", &custom_name));
    custom_block.append(&row("url base", "e.g.: http://127.0.0.1:8080/v1", &custom_url));
    custom_block.append(&row("api", "protocol that the server spokes", &custom_api));
    custom_block.append(&row("API key", "goes in models.json; optional", &custom_key));
    custom_block.append(&row("models", "one id for line; use discover", &custom_scroll));
    custom_block.append(&row("standard model", "new sessions", &custom_default));
    custom_block.append(&custom_status);
    custom_block.append(&custom_actions);
    page.append(&custom_block);

    let ui = Rc::new(Ui {
        window: window.clone(),
        bin: bin.to_string(),
        connected,
        status,
        provider,
        key,
        hint,
        connect,
        models_block,
        models_title,
        models_list,
        custom_block,
        custom_name,
        custom_url,
        custom_api,
        custom_key,
        custom_models,
        custom_status,
        custom_default,
        models_provider: RefCell::new(String::new()),
        connecting: Cell::new(false),
        discover,
        discovering: Cell::new(false),
    });

    {
        let ui = ui.clone();
        let widget = ui.provider.clone();
        widget.connect_selected_notify(move |_| ui.refresh_hint());
    }
    {
        let ui = ui.clone();
        let widget = ui.connect.clone();
        widget.connect_clicked(move |_| ui.clone().connect_provider());
    }
    {
        let ui = ui.clone();
        let widget = ui.key.clone();
        widget.connect_activate(move |_| ui.clone().connect_provider());
    }
    {
        let ui = ui.clone();
        let widget = ui.custom_url.clone();
        widget.connect_activate(move |_| ui.clone().discover_custom());
    }
    {
        let ui = ui.clone();
        let widget = ui.discover.clone();
        widget.connect_clicked(move |_| ui.clone().discover_custom());
    }
    {
        let ui = ui.clone();
        save.connect_clicked(move |_| ui.clone().save_custom());
    }

    if std::env::var("PI_DESKTOP_SETTINGS_CUSTOM").is_ok() {
        ui.provider.set_selected(providers::KNOWN.len() as u32);
        ui.custom_name.set_text("my-proxy");
        ui.custom_url.set_text("http://127.0.0.1:8080/v1");
        ui.custom_models
            .buffer()
            .set_text("model-a\nmodel-b");
        ui.custom_default.set_text("model-a");
    }
    ui.refresh_hint();
    ui.refresh();
    if let Ok(provider) = std::env::var("PI_DESKTOP_SETTINGS_MODELS") {
        ui.show_models(provider.trim());
    }
    page
}


fn settings_window() -> Option<gtk::Window> {
    let toplevels = gtk::Window::toplevels();
    for index in 0..toplevels.n_items() {
        let item = toplevels.item(index)?;
        let Ok(window) = item.downcast::<gtk::Window>() else {
            continue;
        };
        if window.has_css_class("pi-settings") {
            return Some(window);
        }
    }
    None
}

fn check_auth(bin: &str, provider: &str, dot: &gtk::Box) {
    dot.set_tooltip_text(Some("Checking credentials..."));
    let bin = bin.to_string();
    let provider = provider.to_string();
    let dot = dot.clone();
    let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
    std::thread::spawn(move || {
        let result = resolve_pi_bin(&bin).and_then(|path| {
            std::process::Command::new(path)
                .args([
                    "auth",
                    "check",
                    "--provider",
                    &provider,
                    "--json",
                    "--no-refresh",
                ])
                .env("PI_CODING_AGENT_DIR", config::pi_agent_dir())
                .stdin(std::process::Stdio::null())
                .output()
                .map_err(|err| format!("failed to run pi: {err}"))
                .and_then(|output| {
                    if output.status.success() {
                        Ok(())
                    } else {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        Err([stdout.trim(), stderr.trim()]
                            .into_iter()
                            .filter(|part| !part.is_empty())
                            .collect::<Vec<_>>()
                            .join(" · "))
                    }
                })
        });
        let _ = tx.send_blocking(result);
    });
    glib::spawn_future_local(async move {
        let Ok(result) = rx.recv().await else {
            return;
        };
        match result {
            Ok(()) => {
                dot.remove_css_class("unknown");
                dot.add_css_class("ok");
                dot.set_tooltip_text(Some("credential ready"));
            }
            Err(reason) => {
                dot.remove_css_class("ok");
                dot.add_css_class("unknown");
                dot.set_tooltip_text(Some(if reason.trim().is_empty() {
                    "uncertified"
                } else {
                    &reason
                }));
            }
        }
    });
}
