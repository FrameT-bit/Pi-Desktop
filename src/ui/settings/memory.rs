
use gtk::prelude::*;

use crate::config;
use crate::ui::settings::pages::{block, row, text_view_sized, view_text};

fn read_file(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn set_view(scroll: &gtk::ScrolledWindow, text: &str) {
    let Some(view) = scroll
        .child()
        .and_then(|child| child.downcast::<gtk::TextView>().ok())
    else {
        return;
    };
    view.buffer().set_text(text);
}

pub fn page() -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");

    let dir = config::pi_agent_dir().join("memory");
    let user_path = dir.join("USER.md");
    let memory_path = dir.join("MEMORY.md");

    let status = gtk::Label::new(None);
    status.add_css_class("settings-ok");
    status.set_xalign(0.0);
    status.set_visible(false);
    page.append(&status);

    let user_view = text_view_sized(&read_file(&user_path), 120, 220);
    let user_block = block(&page, "USER'S MEMORY");
    user_block.append(&row(
        "durable preferences and facts",
        "the model injects a summary into the prompt; details are available on demand (memory search)",
        &user_view,
    ));

    let memory_view = text_view_sized(&read_file(&memory_path), 120, 220);
    let memory_block = block(&page, "PROJECT MEMORY");
    memory_block.append(&row(
        "decisions that last across sessions",
        "one entry per line, starting with \"- \"; do not keep keys or secrets",
        &memory_view,
    ));

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.add_css_class("settings-actions");
    let save = gtk::Button::with_label("Save memory");
    save.add_css_class("settings-mini");
    let reload = gtk::Button::with_label("Reload from disk");
    reload.add_css_class("settings-mini");
    actions.append(&save);
    actions.append(&reload);
    page.append(&actions);

    let note = gtk::Label::new(Some(&format!(
        "the agent writes here alone (review every 10 shifts or when you ask to remember); files in {}",
        dir.display()
    )));
    note.add_css_class("settings-item-dec");
    note.set_xalign(0.0);
    note.set_wrap(true);
    page.append(&note);

    {
        let status = status.clone();
        let user_view = user_view.clone();
        let memory_view = memory_view.clone();
        let user_path = user_path.clone();
        let memory_path = memory_path.clone();
        let dir = dir.clone();
        save.connect_clicked(move |_| {
            let user = view_text(&user_view);
            let memory = view_text(&memory_view);
            if user.chars().count() > 40_000 || memory.chars().count() > 40_000 {
                status.remove_css_class("settings-ok");
                status.add_css_class("settings-error");
                status.set_text("memory too large (max 40,000 characters per file)");
                status.set_visible(true);
                return;
            }
            let result = std::fs::create_dir_all(&dir)
                .and_then(|_| config::write_atomic(&user_path, &user))
                .and_then(|_| config::write_atomic(&memory_path, &memory));
            match result {
                Ok(()) => {
                    status.remove_css_class("settings-error");
                    status.add_css_class("settings-ok");
                    status.set_text("memory saved · effective next turn");
                    status.set_visible(true);
                }
                Err(err) => {
                    status.remove_css_class("settings-ok");
                    status.add_css_class("settings-error");
                    status.set_text(&format!("failed to save memory: {err}"));
                    status.set_visible(true);
                }
            }
        });
    }

    {
        let status = status.clone();
        let user_view = user_view.clone();
        let memory_view = memory_view.clone();
        reload.connect_clicked(move |_| {
            set_view(&user_view, &read_file(&user_path));
            set_view(&memory_view, &read_file(&memory_path));
            status.remove_css_class("settings-error");
            status.add_css_class("settings-ok");
            status.set_text("reloaded from disk");
            status.set_visible(true);
        });
    }

    page
}
