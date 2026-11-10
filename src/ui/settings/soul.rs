
use gtk::prelude::*;

use crate::config::Config;
use crate::ui::settings::pages::{block, push_editor, push_switch, row, text_view_sized, toggle};
use crate::ui::settings::Form;

pub fn page(config: &Config, form: &mut Form) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");

    let persona_block = block(&page, "PERSONALITY (GROWN TO PROMPT)");
    let persona = text_view_sized(&config.append_system_prompt, 220, 320);
    push_editor(form, "append_system_prompt", persona.clone());
    persona_block.append(&row(
        "person's text",
        "who the model is, as it speaks, what avoids — goes on --append-system-prompt",
        &persona,
    ));

    let persona_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    persona_actions.add_css_class("settings-actions");
    let load_persona = gtk::Button::with_label("load from file...");
    load_persona.add_css_class("settings-mini");
    let clear_persona = gtk::Button::with_label("clear");
    clear_persona.add_css_class("settings-mini");
    clear_persona.add_css_class("danger");
    persona_actions.append(&load_persona);
    persona_actions.append(&clear_persona);
    persona_block.append(&persona_actions);

    let replace_block = block(&page, "PROMPT OF THE SYSTEM");
    let replace = toggle(!config.system_prompt.trim().is_empty());
    push_switch(form, "soul_replace", replace.clone());
    replace_block.append(&row(
        "place More's default quickly",
        "connected: the text below turns the whole prompt (the Pi loses the native instructions)",
        &replace,
    ));

    let system = text_view_sized(&config.system_prompt, 140, 220);
    push_editor(form, "system_prompt", system.clone());
    let system_row = row(
        "system prompt",
        "goes in --system-prompt; only goes with the switch turned on",
        &system,
    );

    let system_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    system_actions.add_css_class("settings-actions");
    let load_system = gtk::Button::with_label("load from file...");
    load_system.add_css_class("settings-mini");
    let clear_system = gtk::Button::with_label("clear");
    clear_system.add_css_class("settings-mini");
    clear_system.add_css_class("danger");
    system_actions.append(&load_system);
    system_actions.append(&clear_system);

    let hint = gtk::Label::new(None);
    hint.add_css_class("settings-item-dec");
    hint.set_xalign(0.0);
    hint.set_visible(false);

    let everything = gtk::Box::new(gtk::Orientation::Vertical, 0);
    everything.append(&system_row);
    everything.append(&system_actions);
    everything.append(&hint);
    everything.set_visible(replace.state());
    replace_block.append(&everything);

    let note = gtk::Label::new(Some(
        "You're not alone; this goes in --append-system-prompt",
    ));
    note.add_css_class("settings-note");
    note.add_css_class("dim");
    note.set_xalign(0.0);
    note.set_wrap(true);
    page.append(&note);

    {
        let everything = everything.clone();
        replace.connect_state_set(move |_, state| {
            everything.set_visible(state);
            glib::Propagation::Proceed
        });
    }

    {
        let persona = persona.clone();
        let hint = hint.clone();
        load_persona.connect_clicked(move |button| {
            let persona = persona.clone();
            let hint = hint.clone();
            pick_file(button, move |path, text| {
                view_set(&persona, &text);
                hint.set_text(&format!("Loaded: {path}"));
                hint.set_visible(true);
            });
        });
    }
    {
        let system = system.clone();
        let hint = hint.clone();
        load_system.connect_clicked(move |button| {
            let system = system.clone();
            let hint = hint.clone();
            pick_file(button, move |path, text| {
                view_set(&system, &text);
                hint.set_text(&format!("Loaded: {path}"));
                hint.set_visible(true);
            });
        });
    }
    {
        let persona = persona.clone();
        clear_persona.connect_clicked(move |_| view_set(&persona, ""));
    }
    {
        let system = system.clone();
        clear_system.connect_clicked(move |_| view_set(&system, ""));
    }

    page
}

fn view_set(scroll: &gtk::ScrolledWindow, text: &str) {
    let Some(view) = scroll.child().and_then(|child| child.downcast::<gtk::TextView>().ok()) else {
        return;
    };
    view.buffer().set_text(text);
}

fn pick_file(button: &gtk::Button, on_load: impl Fn(String, String) + 'static) {
    let dialog = gtk::FileDialog::new();
    dialog.set_title("load text");
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("text and markdown"));
    for pattern in ["*.md", "*.txt", "*.markdown"] {
        filter.add_pattern(pattern);
    }
    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    let Some(window) = button.root().and_then(|root| root.downcast::<gtk::Window>().ok()) else {
        return;
    };
    dialog.open(Some(&window), gtk::gio::Cancellable::NONE, move |result| {
        let Ok(file) = result else {
            return;
        };
        let Some(path) = file.path() else {
            return;
        };
        match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > 256 * 1024 => return,
            Ok(_) => {}
            Err(_) => return,
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            on_load(path.display().to_string(), text);
        }
    });
}
