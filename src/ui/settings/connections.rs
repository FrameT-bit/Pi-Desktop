
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::icons::{self, Tone};

use crate::ui::settings::pages::{block, dropdown, row, text_entry, toggle};
use crate::websearch::{self, Kind, WebSearch};

pub type SaveConnectionsFn = Rc<dyn Fn() -> Result<(), String>>;

pub fn page() -> (gtk::Box, SaveConnectionsFn) {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("settings-page-content");

    let store = WebSearch::load();
    let path = store.path().display().to_string();

    let status = gtk::Label::new(None);
    status.add_css_class("settings-error");
    status.set_xalign(0.0);
    status.set_visible(false);

    let saved_hooks: Rc<RefCell<Vec<Box<dyn Fn()>>>> = Rc::new(RefCell::new(Vec::new()));
    let mut controls: Vec<(&'static str, Control)> = Vec::new();
    let mut blocks: Vec<(&'static str, gtk::Box)> = Vec::new();

    for field in websearch::FIELDS {
        let group = if field.key == "enabled" {
            "GENERAL"
        } else if field.key.starts_with("firecrawl") {
            "FIRECRAWL"
        } else if field.key.starts_with("searxng") {
            "SEARXNG"
        } else {
            "OTHER SEARCHERS"
        };
        if !blocks.iter().any(|(name, _)| *name == group) {
            blocks.push((group, block(&page, group)));
        }
        let current_block = blocks
            .iter()
            .find(|(name, _)| *name == group)
            .map(|(_, widget)| widget.clone())
            .expect("block created above");

        match field.kind {
            Kind::Bool => {
                let switch = toggle(store.get_bool(field.key));
                current_block.append(&row(field.label, field.hint, &switch));
                controls.push((field.key, Control::Switch(switch)));
            }
            Kind::Text | Kind::Url => {
                let value = store.get_str(field.key).unwrap_or_default();
                let entry = text_entry(if field.secret { "" } else { &value }, 320);
                if field.secret {
                    entry.set_visibility(false);
                    entry.set_placeholder_text(Some(if value.is_empty() {
                        "paste the key"
                    } else {
                        "leave empty to keep the key saved"
                    }));

                    let has = Rc::new(Cell::new(!value.is_empty()));
                    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    line.append(&entry);
                    let clear = gtk::Button::with_label("remove");
                    clear.add_css_class("settings-mini");
                    clear.add_css_class("danger");
                    clear.set_valign(gtk::Align::Center);
                    line.append(&clear);

                    let state_icon = icons::icon("check", Tone::Accent);
                    state_icon.set_pixel_size(14);
                    state_icon.set_valign(gtk::Align::Center);
                    let state_label = gtk::Label::new(None);
                    state_label.add_css_class("settings-secret-state");
                    state_label.set_xalign(0.0);
                    let state = gtk::Box::new(gtk::Orientation::Horizontal, 5);
                    state.append(&state_icon);
                    state.append(&state_label);

                    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
                    body.append(&line);
                    body.append(&state);
                    current_block.append(&row(field.label, field.hint, &body));

                    let refresh = {
                        let entry = entry.clone();
                        let has = has.clone();
                        let state_label = state_label.clone();
                        let state_icon = state_icon.clone();
                        let clear = clear.clone();
                        Rc::new(move || {
                            for class in ["saved", "editing", "missing"] {
                                state_label.remove_css_class(class);
                            }
                            let editing = !entry.text().trim().is_empty();
                            if editing {
                                state_label.set_text("new key — save to replace");
                                state_label.add_css_class("editing");
                                state_icon.set_visible(false);
                                clear.set_visible(true);
                            } else if has.get() {
                                state_label.set_text("key saved");
                                state_label.add_css_class("saved");
                                state_icon.set_visible(true);
                                clear.set_visible(true);
                            } else {
                                state_label.set_text("no key");
                                state_label.add_css_class("missing");
                                state_icon.set_visible(false);
                                clear.set_visible(false);
                            }
                        })
                    };
                    refresh();
                    {
                        let refresh = refresh.clone();
                        entry.connect_changed(move |_| refresh());
                    }

                    {
                        let status = status.clone();
                        let key = field.key;
                        let entry = entry.clone();
                        let has = has.clone();
                        let refresh = refresh.clone();
                        clear.connect_clicked(move |_| {
                            let mut store = WebSearch::load();
                            store.remove(key);
                            match store.save_atomic() {
                                Ok(()) => {
                                    has.set(false);
                                    entry.set_text("");
                                    refresh();
                                    status.set_text(&format!("{key} removed"));
                                    status.set_visible(true);
                                    status.remove_css_class("settings-error");
                                    status.add_css_class("settings-ok");
                                }
                                Err(err) => {
                                    status.set_text(&format!("failed to save: {err}"));
                                    status.set_visible(true);
                                }
                            }
                        });
                    }

                    {
                        let entry = entry.clone();
                        let has = has.clone();
                        let refresh = refresh.clone();
                        let key = field.key;
                        saved_hooks.borrow_mut().push(Box::new(move || {
                            if !entry.text().trim().is_empty() {
                                has.set(true);
                            }
                            entry.set_text("");
                            has.set(WebSearch::load().get_str(key).is_some());
                            refresh();
                        }));
                    }
                } else {
                    current_block.append(&row(field.label, field.hint, &entry));
                }
                controls.push((field.key, Control::Text(entry, field.secret)));
            }
            Kind::Choice(values) => {
                let selected = store
                    .get_str(field.key)
                    .and_then(|value| values.iter().position(|item| *item == value))
                    .unwrap_or(0);
                let choice = dropdown(values, values, selected);
                current_block.append(&row(field.label, field.hint, &choice));
                controls.push((field.key, Control::Choice(choice, values)));
            }
        }
    }

    let controls = Rc::new(controls);
    let save_store: SaveConnectionsFn = {
        let controls = controls.clone();
        let saved_hooks = saved_hooks.clone();
        Rc::new(move || {
            let mut store = WebSearch::load();
            for (key, control) in controls.iter() {
                match control {
                    Control::Switch(switch) => store.set_bool(key, switch.state()),
                    Control::Text(entry, secret) => {
                        let text = entry.text().trim().to_string();
                        if *secret && text.is_empty() {
                            continue;
                        }
                        if text.is_empty() {
                            store.remove(key);
                            continue;
                        }
                        if matches!(
                            websearch::field(key).map(|field| field.kind),
                            Some(Kind::Url)
                        ) {
                            if let Err(message) = websearch::validate_url(&text) {
                                return Err(format!("{key}: {message}"));
                            }
                        }
                        store.set_str(key, &text);
                    }
                    Control::Choice(choice, values) => {
                        let index = choice.selected() as usize;
                        if let Some(value) = values.get(index) {
                            store.set_str(key, value);
                        }
                    }
                }
            }
            let result = store
                .save_atomic()
                .map_err(|err| format!("failure to record web-search.json: {err}"));
            if result.is_ok() {
                for hook in saved_hooks.borrow().iter() {
                    hook();
                }
            }
            result
        })
    };

    let save = gtk::Button::with_label("Save Connections");
    save.add_css_class("dialog-accept");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.add_css_class("settings-actions");
    actions.append(&save);

    let path_label = gtk::Label::new(Some(&format!("file: {path}")));
    path_label.add_css_class("settings-item-desc");
    path_label.set_xalign(0.0);
    actions.append(&path_label);

    let footer = block(&page, "SAVE");
    footer.append(&actions);
    page.append(&status);

    {
        let status = status.clone();
        let save_store = save_store.clone();
        save.connect_clicked(move |_| match save_store() {
            Ok(()) => {
                status.set_text("Connections saved");
                status.set_visible(true);
                status.remove_css_class("settings-error");
                status.add_css_class("settings-ok");
            }
            Err(err) => {
                status.set_text(&err);
                status.set_visible(true);
                status.add_css_class("settings-error");
                status.remove_css_class("settings-ok");
            }
        });
    }

    (page, save_store)
}

enum Control {
    Switch(gtk::Switch),
    Text(gtk::Entry, bool),
    Choice(gtk::DropDown, &'static [&'static str]),
}
