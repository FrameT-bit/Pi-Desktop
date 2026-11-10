
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk::prelude::*;

use crate::state::PI_VERSION;
use crate::updater::{self, UpdateState};

use super::pages::{block, row};

const PLUGINS: &[(&str, &str)] = &[
    ("pi-memory", "durable memory and search across old sessions"),
    ("pi-personas", "agent personalities"),
    ("pi-plan", "read-only planning and task progress"),
    ("pi-guard", "permission modes, rules and protections"),
    ("pi-checkpoints", "file checkpoints and rollback"),
    ("pi-web-access", "web search, page extraction and research"),
    ("pi-subagents", "delegated subagents in the activity panel"),
];

pub fn page() -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");

    let about = block(&page, "ABOUT");
    let about_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    about_rows.add_css_class("settings-rows");
    about.append(&about_rows);
    for (label, value, description) in [
        ("version", env!("CARGO_PKG_VERSION").to_string(), "application version"),
        ("Pi server", PI_VERSION.to_string(), "target RPC protocol"),
        ("author", "FrameT".to_string(), "author of the project"),
        ("License", "GPL-3.0-or-later".to_string(), "free software; see LICENSE"),
    ] {
        let value_label = gtk::Label::new(Some(&value));
        value_label.add_css_class("settings-row-label");
        value_label.set_xalign(1.0);
        about_rows.append(&row(label, description, &value_label));
    }

    let links = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    links.add_css_class("settings-actions");
    for (label, url) in [
        ("repository", updater::REPO_URL),
        ("releases", updater::RELEASES_URL),
        ("issues", updater::ISSUES_URL),
    ] {
        let button = gtk::Button::with_label(label);
        button.add_css_class("settings-mini");
        let target = url.to_string();
        button.connect_clicked(move |_| updater::open(&target));
        links.append(&button);
    }
    about.append(&links);

    let updates = block(&page, "Updates");
    let update_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    update_rows.add_css_class("settings-rows");
    updates.append(&update_rows);

    let cached = updater::load();
    let status = gtk::Label::new(None);
    status.add_css_class("settings-row-desc");
    status.set_xalign(0.0);
    status.set_wrap(true);
    let status_row = row(
        "status",
        "GitHub releases, checked at most once a week when opening the app",
        &status,
    );
    update_rows.append(&status_row);
    render_update_state(&status, &cached);

    let check = gtk::Button::with_label("check now");
    check.add_css_class("settings-mini");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class("settings-actions");
    actions.append(&check);
    if cached.has_update() {
        let open = gtk::Button::with_label("open release");
        open.add_css_class("settings-mini");
        let url = cached.url.clone();
        open.connect_clicked(move |_| updater::open(&url));
        actions.append(&open);
    }
    updates.append(&actions);

    {
        let status = status.clone();
        let check_button = check.clone();
        check.connect_clicked(move |_| {
            check_button.set_sensitive(false);
            status.set_text("Checking GitHub...");
            status.set_visible(true);
            let (tx, rx) = async_channel::bounded::<UpdateState>(1);
            updater::spawn_check(tx);
            let status = status.clone();
            let check_button = check_button.clone();
            glib::spawn_future_local(async move {
                if let Ok(state) = rx.recv().await {
                    render_update_state(&status, &state);
                } else {
                    status.set_text("check failed: no response");
                }
                check_button.set_sensitive(true);
            });
        });
    }

    let plugins = block(&page, "PRE- INSTALLATED EXTENSIONS");
    let plugin_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    plugin_rows.add_css_class("settings-rows");
    plugins.append(&plugin_rows);
    for (name, description) in PLUGINS {
        let value = gtk::Label::new(Some("installed"));
        value.add_css_class("settings-row-desc");
        value.set_xalign(1.0);
        plugin_rows.append(&row(name, description, &value));
    }

    let licenses = block(&page, "LICENCES OF THIRD PARTIES");
    let license_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    license_rows.add_css_class("settings-rows");
    licenses.append(&license_rows);
    for (name, license) in [
        ("pixelarticons", "MIT"),
        ("GTK4 / libadwaite", "LGPL-2.1-or-later"),
        ("Pixelify Sans / JetBrains Mono", "SIL Open Font License 1.1"),
        ("pi-web-access and Pi packages", "licences of the respective projects"),
    ] {
        let value = gtk::Label::new(Some(license));
        value.add_css_class("settings-row-desc");
        value.set_xalign(1.0);
        license_rows.append(&row(name, "", &value));
    }

    page
}

fn render_update_state(label: &gtk::Label, state: &UpdateState) {
    if !state.error.is_empty() {
        label.set_text(&format!("check failed: {}", state.error));
        return;
    }
    if state.latest.is_empty() {
        label.set_text("not checked yet");
        return;
    }
    if state.has_update() {
        label.set_text(&format!(
            "update v{} available · current v{} · verified {}",
            state.latest,
            state.current,
            relative_time(state.checked_at)
        ));
    } else {
        label.set_text(&format!(
            "up to date · v{} · checked {}",
            state.current,
            relative_time(state.checked_at)
        ));
    }
}

fn relative_time(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "never".to_string();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64;
    let seconds = now.saturating_sub(timestamp);
    if seconds < 60 {
        "just now".to_string()
    } else if seconds < 3_600 {
        format!("{} min ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} h ago", seconds / 3_600)
    } else {
        format!("{} d ago", seconds / 86_400)
    }
}
