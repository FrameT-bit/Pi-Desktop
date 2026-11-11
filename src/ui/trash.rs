use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::prelude::*;

use crate::session_ops;

use super::fit;

const DAY: u64 = 86_400;

pub fn show(parent: &impl IsA<gtk::Window>, on_restore: impl Fn() + 'static) {
    let window = gtk::Window::new();
    window.set_transient_for(Some(parent));
    window.set_modal(true);
    window.set_destroy_with_parent(true);
    window.set_decorated(false);
    window.add_css_class("trash-window");
    fit::fit(&window, 620, 480);
    if let Some(app) = parent.as_ref().application() {
        window.set_application(Some(&app));
    }
    window.set_title(Some("Deleted workspaces"));

    let title = gtk::Label::new(Some("DELETED WORKSPACES"));
    title.add_css_class("settings-title");
    title.set_xalign(0.0);
    let subtitle = gtk::Label::new(Some("kept for 14 days"));
    subtitle.add_css_class("plugin-config-path");
    subtitle.set_xalign(0.0);
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 1);
    titles.set_hexpand(true);
    titles.append(&title);
    titles.append(&subtitle);
    let controls = gtk::WindowControls::new(gtk::PackType::End);
    controls.set_valign(gtk::Align::Center);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("settings-header");
    header.append(&titles);
    header.append(&controls);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.add_css_class("trash-list");
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&list)
        .build();
    scroll.add_css_class("trash-scroll");

    let status = gtk::Label::new(None);
    status.add_css_class("trash-status");
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let close = gtk::Button::with_label("Close");
    close.add_css_class("settings-cancel");
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.add_css_class("settings-footer");
    footer.append(&status);
    footer.append(&close);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header);
    root.append(&scroll);
    root.append(&footer);
    window.set_child(Some(&root));

    {
        let window = window.clone();
        close.connect_clicked(move |_| window.close());
    }

    let on_restore: Rc<dyn Fn()> = Rc::new(on_restore);
    render(&list, &status, on_restore);
    window.present();
}

fn render(list: &gtk::Box, status: &gtk::Label, on_restore: Rc<dyn Fn()>) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let entries = session_ops::list_workspace_trash();
    if entries.is_empty() {
        let empty = gtk::Label::new(Some(
            "trash is empty — deleted workspaces stay here for 14 days",
        ));
        empty.add_css_class("trash-empty");
        empty.set_xalign(0.0);
        empty.set_margin_start(16);
        empty.set_margin_end(16);
        empty.set_margin_top(20);
        list.append(&empty);
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    for entry in entries {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("trash-row");

        let name = gtk::Label::new(Some(&entry.label));
        name.add_css_class("trash-name");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let age = now.saturating_sub(entry.deleted) / DAY;
        let left = session_ops::WORKSPACE_TRASH_DAYS.saturating_sub(age);
        let path = gtk::Label::new(Some(&entry.original.to_string_lossy()));
        path.add_css_class("trash-path");
        path.set_xalign(0.0);
        path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path.set_tooltip_text(Some(&entry.original.to_string_lossy()));
        let sessions = if entry.sessions == 1 {
            "1 session".to_string()
        } else {
            format!("{} sessions", entry.sessions)
        };
        let days = if left == 1 {
            "1 day".to_string()
        } else {
            format!("{left} days")
        };
        let meta = gtk::Label::new(Some(&format!(
            "{sessions} · deleted {} · {days} left",
            date_label(entry.deleted)
        )));
        meta.add_css_class("trash-meta");
        meta.set_xalign(0.0);

        let info = gtk::Box::new(gtk::Orientation::Vertical, 2);
        info.set_hexpand(true);
        info.append(&name);
        info.append(&path);
        info.append(&meta);

        let restore = gtk::Button::with_label("restore");
        restore.add_css_class("settings-save");
        restore.set_valign(gtk::Align::Center);

        let list_c = list.clone();
        let status_c = status.clone();
        let on_restore_c = on_restore.clone();
        restore.connect_clicked(move |_| {
            match session_ops::restore_workspace(&entry) {
                Ok(()) => {
                    status_c.remove_css_class("error");
                    status_c.set_text(&format!("restored {}", entry.label));
                    render(&list_c, &status_c, on_restore_c.clone());
                    on_restore_c();
                }
                Err(error) => {
                    status_c.add_css_class("error");
                    status_c.set_text(&format!("restore failed: {error}"));
                }
            }
        });

        row.append(&info);
        row.append(&restore);
        list.append(&row);
    }
}

fn date_label(seconds: u64) -> String {
    glib::DateTime::from_unix_local(seconds as i64)
        .ok()
        .and_then(|date| date.format("%Y-%m-%d %H:%M").ok())
        .map(|text| text.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
