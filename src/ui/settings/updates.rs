use gtk::prelude::*;

use crate::updater::{self, UpdateState};

use super::pages::{block, row};

pub(super) fn page() -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");
    let release = block(&page, "GITHUB RELEASES");
    let status = gtk::Label::new(None);
    status.add_css_class("settings-row-desc");
    status.set_wrap(true);
    status.set_selectable(true);
    status.set_xalign(0.0);
    release.append(&row("Installed", "", &gtk::Label::new(Some(crate::APP_VERSION))));
    release.append(&status);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class("settings-actions");
    let check = gtk::Button::with_label("Check now");
    check.add_css_class("settings-mini");
    check.add_css_class("compact-action");
    actions.append(&check);
    release.append(&actions);
    let links = gtk::Box::new(gtk::Orientation::Vertical, 0);
    release.append(&links);
    render(&status, &links, &updater::load());

    check.connect_clicked(move |check| {
        check.set_sensitive(false);
        status.set_text("Checking GitHub…");
        let (tx, rx) = async_channel::bounded(1);
        updater::spawn_check(tx);
        let status = status.downgrade();
        let links = links.downgrade();
        let check = check.downgrade();
        glib::spawn_future_local(async move {
            let result = rx.recv().await;
            let (Some(status), Some(links), Some(check)) =
                (status.upgrade(), links.upgrade(), check.upgrade()) else { return };
            check.set_sensitive(true);
            match result {
                Ok(state) => render(&status, &links, &state),
                Err(_) => status.set_text("Could not start the update check."),
            }
        });
    });
    page
}

fn render(label: &gtk::Label, links: &gtk::Box, state: &UpdateState) {
    let version = format!("v{}{}", state.latest, if state.prerelease { " (preview)" } else { "" });
    let status = if !state.error.is_empty() {
        format!("Check failed: {}", state.error)
    } else if !state.is_checked() {
        "Not checked for this build yet.".into()
    } else if state.latest.is_empty() {
        "No public releases found.".into()
    } else if state.has_update() {
        format!("{version} is available.")
    } else if updater::version_gt(&state.current, &state.latest) {
        format!("This build is newer than the latest public release, {version}.")
    } else {
        format!("You’re up to date · {version}.")
    };
    label.set_text(&status);

    while let Some(child) = links.first_child() { links.remove(&child); }
    if state.is_checked() && state.error.is_empty() && state.url.starts_with(updater::RELEASES_URL) {
        add_link(links, "Open release on GitHub", &state.url);
        if state.has_update() {
            if let Some(asset) = state.appimage.as_deref().filter(|url| url.starts_with(updater::RELEASES_URL)) {
                add_link(links, "Download AppImage", asset);
            }
        }
    } else {
        add_link(links, "Browse GitHub releases", updater::RELEASES_URL);
    }
}

fn add_link(parent: &gtk::Box, label: &str, uri: &str) {
    let link = gtk::LinkButton::with_label(uri, label);
    link.set_halign(gtk::Align::Start);
    link.add_css_class("compact-action");
    parent.append(&link);
}
