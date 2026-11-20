
use gtk::prelude::*;

use crate::state::PI_VERSION;
use crate::updater;

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
        ("issues", updater::ISSUES_URL),
    ] {
        let button = gtk::Button::with_label(label);
        button.add_css_class("settings-mini");
        let target = url.to_string();
        button.connect_clicked(move |_| updater::open(&target));
        links.append(&button);
    }
    about.append(&links);

    let plugins = block(&page, "PREINSTALLED EXTENSIONS");
    let plugin_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    plugin_rows.add_css_class("settings-rows");
    plugins.append(&plugin_rows);
    for (name, description) in PLUGINS {
        let value = gtk::Label::new(Some("installed"));
        value.add_css_class("settings-row-desc");
        value.set_xalign(1.0);
        plugin_rows.append(&row(name, description, &value));
    }

    let licenses = block(&page, "THIRD-PARTY LICENSES");
    let license_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    license_rows.add_css_class("settings-rows");
    licenses.append(&license_rows);
    for (name, license) in [
        ("pixelarticons", "MIT"),
        ("GTK4 / libadwaita", "LGPL-2.1-or-later"),
        ("Pixelify Sans / JetBrains Mono", "SIL Open Font License 1.1"),
        ("pi-web-access and Pi packages", "licenses of the respective projects"),
    ] {
        let value = gtk::Label::new(Some(license));
        value.add_css_class("settings-row-desc");
        value.set_xalign(1.0);
        license_rows.append(&row(name, "", &value));
    }

    page
}
