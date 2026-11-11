
use gtk::prelude::*;

use super::{Control, Form};
use crate::config::Config;
use crate::state::{resolve_pi_bin, ModelItem};
use crate::theme::{self, FontChoice, ThemeMode};


pub fn choice_value(key: &str, index: usize) -> &'static str {
    match key {
        "target" => ["local", "remote"].get(index).copied().unwrap_or("local"),
        "thinking" => ["", "off", "minimal", "low", "medium", "high", "xhigh", "max"]
            .get(index)
            .copied()
            .unwrap_or(""),
        "approve" => ["ask", "yes", "no"].get(index).copied().unwrap_or("ask"),
        "theme" => ["system", "light", "dark"]
            .get(index)
            .copied()
            .unwrap_or("system"),
        _ => "",
    }
}


pub fn blank() -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, 0)
}

pub fn connection_target(config: &Config, form: &mut Form) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let values: &'static [&'static str] = &["local", "remote"];
    let target = dropdown(
        values,
        &["this computer (standard)", "server via ssh (alternative)"],
        config.target.index(),
    );
    push_choice(form, "target", target.clone());
    rows.append(&row(
        "where Pi runs",
        "default: this computer. Server is an explicit alternative - UI stays here and the agent runs there",
        &target,
    ));

    let bin = text_entry(&config.bin, 300);
    push_text(form, "bin", bin.clone());
    let detected = resolve_pi_bin(&config.bin)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "not found".to_string());
    rows.append(&row(
        "Pi binary",
        &format!("binary used in local mode — resolved: {detected}"),
        &bin,
    ));

    let ssh = text_entry(&config.remote_ssh, 300);
    push_text(form, "remote_ssh", ssh.clone());
    let cmd = text_entry(&config.remote_cmd, 300);
    push_text(form, "remote_cmd", cmd.clone());

    let remote = gtk::Box::new(gtk::Orientation::Vertical, 0);
    remote.append(&row("ssh server", "user", &ssh));
    remote.append(&row(
        "Remote control",
        "empty = mounts command with workspace and session",
        &cmd,
    ));
    remote.set_visible(config.target == crate::config::Target::Remote);
    rows.append(&remote);

    let remote_c = remote.clone();
    target.connect_selected_notify(move |dropdown| {
        remote_c.set_visible(dropdown.selected() == 1);
    });
    page
}

pub fn connection_session(config: &Config, form: &mut Form) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let workspace = text_entry(&config.workspace, 300);
    push_text(form, "workspace", workspace.clone());
    rows.append(&row(
        "workspace",
        "working directory of the pi process",
        &workspace,
    ));

    let id = text_entry(&config.session_id, 300);
    push_text(form, "session_id", id.clone());
    rows.append(&row("session id", "lowercase letters, numbers and hyphen", &id));

    let start_last = toggle(config.start_last);
    push_switch(form, "start_last", start_last.clone());
    rows.append(&row(
        "start with last conversation",
        "Off (standard): each opening starts a new session; linked: continues the latest",
        &start_last,
    ));

    page
}

pub fn connection_session_advanced(config: &Config, form: &mut Form) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let dir = text_entry(&config.session_dir, 300);
    push_text(form, "session_dir", dir.clone());
    rows.append(&row("Directory of Sessions", "empty = the pi pattern", &dir));

    let name = text_entry(&config.name, 300);
    push_text(form, "name", name.clone());
    rows.append(&row("initial name", "Open session display name", &name));

    let no_session = toggle(config.no_session);
    push_switch(form, "no_session", no_session.clone());
    rows.append(&row(
        "Non-session",
        "session in memory, not recorded on disk",
        &no_session,
    ));
    page
}

pub fn connection_trust(config: &Config, form: &mut Form) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let values: &'static [&'static str] = &["ask", "yes", "no"];
    let approve = dropdown(
        values,
        &["ask (ask)", "allow (yes)", "deny (no)"],
        config.approve.index(),
    );
    push_choice(form, "approve", approve.clone());
    rows.append(&row(
        "trust",
        "local project files: ask, allow or deny",
        &approve,
    ));

    let offline = toggle(config.offline);
    push_switch(form, "offline", offline.clone());
    rows.append(&row(
        "offline",
        "Turn off automatic network activity at the beginning",
        &offline,
    ));
    page
}

pub fn main_model(config: &Config, form: &mut Form, aux_models: &[ModelItem]) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let model = text_entry(&config.model, 300);
    push_text(form, "model", model.clone());
    rows.append(&row(
        "model",
        "accepts provider/id:thinking; empty = Pi decides",
        &model,
    ));

    let provider = text_entry(&config.provider, 300);
    push_text(form, "provider", provider.clone());
    rows.append(&row(
        "provider",
        "provider restriction for --model",
        &provider,
    ));

    let values: &'static [&'static str] = &[
        "", "off", "minimal", "low", "medium", "high", "xhigh", "max",
    ];
    let labels: &'static [&'static str] = &[
        "Pi default",
        "off",
        "minimal",
        "low",
        "medium",
        "high",
        "xhigh",
        "max",
    ];
    let thinking = dropdown(values, labels, config.thinking.index());
    push_choice(form, "thinking", thinking.clone());
    rows.append(&row(
        "reasoning",
        "reasoning level; default = Pi decides",
        &thinking,
    ));

    let auxiliary = aux_model_picker(config, form, aux_models);
    rows.append(&row(
        "auxiliary model",
        "used for titles and verifications; empty = session model",
        &auxiliary,
    ));

    page
}

fn aux_model_picker(config: &Config, form: &mut Form, models: &[ModelItem]) -> gtk::Box {
    let mut values: Vec<String> = vec![String::new()];
    let mut labels: Vec<String> = vec!["same as session model".to_string()];
    for model in models {
        let value = format!("{}/{}", model.provider, model.id);
        if values.iter().any(|item| item == &value) {
            continue;
        }
        let label = if model.name.trim().is_empty() {
            value.clone()
        } else {
            format!("{} · {}", model.name, value)
        };
        labels.push(label);
        values.push(value);
    }
    let configured = config.aux_model.trim();
    if !configured.is_empty() && !values.iter().any(|item| item == configured) {
        labels.push(format!("custom · {configured}"));
        values.push(configured.to_string());
    }
    let selected = values
        .iter()
        .position(|item| item == configured)
        .unwrap_or(0);

    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let list = gtk::StringList::new(&label_refs);
    let dropdown = gtk::DropDown::new(Some(list), gtk::Expression::NONE);
    dropdown.add_css_class("settings-dropdown");
    dropdown.set_enable_search(true);
    dropdown.set_width_request(330);
    dropdown.set_selected(selected as u32);

    let value = text_entry(configured, 300);
    value.set_visible(false);
    push_text(form, "aux_model", value.clone());
    let entry = value.clone();
    dropdown.connect_selected_notify(move |dropdown| {
        let index = dropdown.selected() as usize;
        if let Some(model) = values.get(index) {
            entry.set_text(model);
        }
    });

    let test = gtk::Button::with_label("test");
    test.add_css_class("settings-mini");
    test.set_tooltip_text(Some("makes an invisible call and shows if the model answers"));
    let status = gtk::Label::new(None);
    status.add_css_class("settings-row-desc");
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.set_visible(false);

    let config_c = config.clone();
    let value_c = value.clone();
    let status_c = status.clone();
    let test_c = test.clone();
    test.connect_clicked(move |_| {
        test_c.set_sensitive(false);
        status_c.set_text("testing the model...");
        status_c.set_visible(true);
        let mut test_config = config_c.clone();
        test_config.aux_model = value_c.text().trim().to_string();
        let selected = test_config.aux_model.clone();
        let (tx, rx) = async_channel::bounded::<Result<String, String>>(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(crate::title::probe_model(&test_config, &selected));
        });
        let status_c = status_c.clone();
        let test_c = test_c.clone();
        glib::spawn_future_local(async move {
            match rx.recv().await {
                Ok(Ok(reply)) => {
                    status_c.set_text(&format!("ok · answer: {reply}"));
                    status_c.remove_css_class("error");
                }
                Ok(Err(error)) => {
                    status_c.set_text(&format!("failed: {error}"));
                    status_c.add_css_class("error");
                }
                Err(_) => {
                    status_c.set_text("failed: no process responds");
                    status_c.add_css_class("error");
                }
            }
            test_c.set_sensitive(true);
        });
    });

    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    line.append(&dropdown);
    line.append(&test);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    body.append(&line);
    body.append(&status);
    body
}

pub fn model_cycle(config: &Config, form: &mut Form, agent_dir: &std::path::Path) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let models = text_entry(&config.models, 300);
    push_text(form, "models", models.clone());
    rows.append(&row(
        "scope of models",
        "paragraph-separated globes (Ctrl+P): anthropic/*, *sonnet*",
        &models,
    ));

    let known = crate::providers::known_models(agent_dir);
    if !known.is_empty() {
        let label = gtk::Label::new(Some("KNOWN MODELS"));
        label.add_css_class("settings-block-label");
        label.set_xalign(0.0);
        rows.append(&label);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("settings-known");
        rows.append(&list);

        for model in known.iter().take(80) {
            let token = format!("{}/{}", model.provider, model.id);
            let check = gtk::CheckButton::with_label(&token);
            check.add_css_class("settings-check");
            check.set_active(
                models
                    .text()
                    .split(',')
                    .any(|item| item.trim() == token),
            );
            if !model.name.trim().is_empty() {
                check.set_tooltip_text(Some(&model.name));
            }
            list.append(&check);

            let entry = models.clone();
            let token = token.clone();
            check.connect_toggled(move |check| {
                let mut tokens: Vec<String> = entry
                    .text()
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect();
                if check.is_active() {
                    if !tokens.iter().any(|item| item == &token) {
                        tokens.push(token.clone());
                    }
                } else {
                    tokens.retain(|item| item != &token);
                }
                entry.set_text(&tokens.join(", "));
            });
        }
    }
    page
}

pub fn model_tools(config: &Config, form: &mut Form) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let tools = text_entry(&config.tools, 300);
    push_text(form, "tools", tools.clone());
    rows.append(&row(
        "tools",
        "allowlist; accept +name/-name",
        &tools,
    ));

    let exclude = text_entry(&config.exclude_tools, 300);
    push_text(form, "exclude_tools", exclude.clone());
    rows.append(&row(
        "delete tools",
        "names or globes separated by paragraph",
        &exclude,
    ));

    for (key, label, desc, value) in [
        ("no_tools", "No tools", "turn off all tools", config.no_tools),
        (
            "no_builtin_tools",
            "without native tools",
            "keeps only those of extensions",
            config.no_builtin_tools,
        ),
        ("no_mcp", "Without mcp", "no MCP server connections", config.no_mcp),
    ] {
        let switch = toggle(value);
        push_switch(form, key, switch.clone());
        rows.append(&row(label, desc, &switch));
    }
    page
}

pub fn model_context(config: &Config, form: &mut Form) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let skills = text_entry(&config.skills, 300);
    push_text(form, "skills", skills.clone());
    rows.append(&row("skills", "paths separated by:", &skills));

    let extensions = text_entry(&config.extensions, 300);
    push_text(form, "extensions", extensions.clone());
    rows.append(&row("extensions", "paths separated by:", &extensions));

    let templates = text_entry(&config.prompt_templates, 300);
    push_text(form, "prompt_templates", templates.clone());
    rows.append(&row(
        "quick templates",
        "paths separated by:",
        &templates,
    ));

    for (key, label, desc, value) in [
        ("no_skills", "No skills", "ignores skills discovered", config.no_skills),
        (
            "no_extensions",
            "No extensions",
            "ignore discovered extensions",
            config.no_extensions,
        ),
        (
            "no_prompt_templates",
            "without prompt templates",
            "ignore discovered templates",
            config.no_prompt_templates,
        ),
        (
            "no_context_files",
            "in context files",
            "two not read AGENTS.md or CLAUD.md",
            config.no_context_files,
        ),
    ] {
        let switch = toggle(value);
        push_switch(form, key, switch.clone());
        rows.append(&row(label, desc, &switch));
    }
    page
}

pub fn personalization(config: &Config, form: &mut Form) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    let current = theme::load_mode().unwrap_or(ThemeMode::System);
    let theme_labels: Vec<String> = ThemeMode::all()
        .iter()
        .map(|mode| mode.menu_label())
        .collect();
    let theme = dropdown_owned(&theme_labels, current.index());
    push_choice(form, "theme", theme.clone());
    rows.append(&row(
        "theme",
        "system, light, dark or one of the 50 food themes",
        &theme,
    ));

    let font_labels: Vec<String> = (0..3)
        .map(|index| FontChoice::from_index(index).label().to_string())
        .collect();
    let font = dropdown_owned(&font_labels, config.font.index());
    push_choice(form, "font", font.clone());
    rows.append(&row(
        "font",
        "typeface for prose and code; the size stays adjustable below",
        &font,
    ));

    let scale = text_entry(&config.font_scale.to_string(), 120);
    push_text(form, "font_scale", scale.clone());
    rows.append(&row("font size", "90 to 130 (%) applied to CSS", &scale));

    page
}

pub fn interface(config: &Config, form: &mut Form) -> gtk::Box {
    let page = blank();
    let rows = rows(&page);

    for (key, label, desc, value) in [
        ("timestamps", "time in messages", "HH:MM shows above each message", config.timestamps),
        ("auto_scroll", "Auto-scroll", "follows the end during streaming", config.auto_scroll),
        ("show_costs", "cost in status bar", "shows the token/cost block", config.show_costs),
    ] {
        let switch = toggle(value);
        push_switch(form, key, switch.clone());
        rows.append(&row(label, desc, &switch));
    }

    let width = text_entry(&config.chat_width.to_string(), 120);
    push_text(form, "chat_width", width.clone());
    rows.append(&row("prose width", "60 to 120 characters", &width));

    page
}


fn rows(page: &gtk::Box) -> gtk::Box {
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    rows.add_css_class("settings-rows");
    rows.set_valign(gtk::Align::Start);
    page.append(&rows);
    rows
}

pub fn row(label: &str, description: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    row.add_css_class("settings-row");

    let left = gtk::Box::new(gtk::Orientation::Vertical, 2);
    left.set_hexpand(true);

    let title = gtk::Label::new(Some(label));
    title.add_css_class("settings-row-label");
    title.set_xalign(0.0);
    title.set_wrap(true);

    left.append(&title);
    if !description.is_empty() {
        let desc = gtk::Label::new(Some(description));
        desc.add_css_class("settings-row-desc");
        desc.set_xalign(0.0);
        desc.set_wrap(true);
        desc.set_max_width_chars(58);
        left.append(&desc);
    }

    row.append(&left);
    control.as_ref().set_valign(gtk::Align::Center);
    row.append(control);
    row
}

pub fn text_entry(value: &str, width: i32) -> gtk::Entry {
    let entry = gtk::Entry::new();
    entry.add_css_class("settings-entry");
    entry.set_text(value);
    entry.set_width_chars((width / 10).max(10));
    entry.set_max_width_chars(60);
    entry
}

pub fn text_view_sized(value: &str, min_height: i32, max_height: i32) -> gtk::ScrolledWindow {
    let view = gtk::TextView::new();
    view.add_css_class("settings-editor");
    view.set_wrap_mode(gtk::WrapMode::Word);
    view.buffer().set_text(value);
    view.set_left_margin(8);
    view.set_right_margin(8);
    view.set_top_margin(8);
    view.set_bottom_margin(8);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(min_height)
        .max_content_height(max_height)
        .width_request(380)
        .child(&view)
        .build();
    scroll.add_css_class("settings-view");
    scroll
}

pub fn view_text(scroll: &gtk::ScrolledWindow) -> String {
    let Some(view) = scroll.child().and_then(|child| child.downcast::<gtk::TextView>().ok()) else {
        return String::new();
    };
    let buffer = view.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}

pub fn toggle(value: bool) -> gtk::Switch {
    let switch = gtk::Switch::new();
    switch.add_css_class("settings-switch");
    switch.set_active(value);
    switch
}

pub fn dropdown_owned(labels: &[String], selected: usize) -> gtk::DropDown {
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let list = gtk::StringList::new(&refs);
    let dropdown = gtk::DropDown::new(Some(list), gtk::Expression::NONE);
    dropdown.add_css_class("settings-dropdown");
    dropdown.set_enable_search(true);
    dropdown.set_width_request(330);
    let selected = if selected < labels.len() { selected } else { 0 };
    dropdown.set_selected(selected as u32);
    dropdown
}

pub fn dropdown(
    values: &'static [&'static str],
    labels: &'static [&'static str],
    selected: usize,
) -> gtk::DropDown {
    let _ = values;
    let list = gtk::StringList::new(labels);
    let dropdown = gtk::DropDown::new(Some(list), gtk::Expression::NONE);
    dropdown.add_css_class("settings-dropdown");
    let selected = if selected < labels.len() { selected } else { 0 };
    dropdown.set_selected(selected as u32);
    dropdown
}

pub fn push_choice(form: &mut Form, key: &'static str, dropdown: gtk::DropDown) {
    form.controls.insert(key, Control::Choice(dropdown));
}

pub fn push_text(form: &mut Form, key: &'static str, entry: gtk::Entry) {
    form.controls.insert(key, Control::Entry(entry));
}

pub fn push_editor(form: &mut Form, key: &'static str, view: gtk::ScrolledWindow) {
    form.controls.insert(key, Control::View(view));
}

pub fn push_switch(form: &mut Form, key: &'static str, switch: gtk::Switch) {
    form.controls.insert(key, Control::Switch(switch));
}

pub fn block(rows: &gtk::Box, title: &str) -> gtk::Box {
    let label = gtk::Label::new(Some(title));
    label.add_css_class("settings-block-label");
    label.set_xalign(0.0);
    rows.append(&label);

    let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    block.add_css_class("settings-block");
    rows.append(&block);
    block
}
