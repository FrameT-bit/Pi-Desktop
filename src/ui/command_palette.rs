
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use super::popup::InlineList;

#[derive(Clone)]
pub struct Item {
    pub name: String,
    pub hint: String,
    pub group: &'static str,
    pub action: Rc<dyn Fn()>,
}

impl Item {
    pub fn new(
        name: impl Into<String>,
        hint: impl Into<String>,
        group: &'static str,
        action: impl Fn() + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            hint: hint.into(),
            group,
            action: Rc::new(action),
        }
    }

    fn haystack(&self) -> String {
        format!("{} {}", self.name, self.hint).to_lowercase()
    }
}

pub fn fuzzy_score(haystack: &str, needle: &str) -> Option<i32> {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return Some(0);
    }
    let haystack = haystack.to_lowercase();
    let mut chars = haystack.char_indices();
    let mut score = 0;
    let mut previous: Option<usize> = None;
    let mut needle_chars = needle.chars();

    let mut wanted = needle_chars.next()?;
    while let Some((index, ch)) = chars.next() {
        if ch != wanted {
            continue;
        }
        let word_start = index == 0
            || haystack[..index]
                .chars()
                .next_back()
                .is_some_and(|prev| !prev.is_alphanumeric());
        if word_start {
            score += 6;
        }
        if previous.is_some_and(|prev| prev + 1 == index) {
            score += 4;
        }
        score += 1;
        previous = Some(index);
        match needle_chars.next() {
            Some(next) => wanted = next,
            None => return Some(score),
        }
    }
    None
}

pub struct Palette {
    pub window: gtk::Window,
    entry: gtk::Entry,
    list: Rc<InlineList>,
    items: RefCell<Vec<Item>>,
    shown: RefCell<Vec<usize>>,
}

impl Palette {
    pub fn new(parent: &impl IsA<gtk::Window>, items: Vec<Item>) -> Rc<Self> {
        let window = gtk::Window::new();
        window.set_transient_for(Some(parent));
        window.set_modal(true);
        window.set_destroy_with_parent(true);
        window.set_decorated(false);
        window.set_resizable(false);
        if let Some(app) = parent.as_ref().application() {
            window.set_application(Some(&app));
        }
        window.add_css_class("pi-dialog");
        window.set_title(Some("Commands"));
        crate::ui::fit::fit(&window, 620, 560);

        let title = gtk::Label::new(Some("Commands"));
        title.add_css_class("dialog-title");
        title.set_xalign(0.0);
        title.set_hexpand(true);

        let escape = gtk::Label::new(Some("esc"));
        escape.add_css_class("dialog-note");
        escape.set_valign(gtk::Align::Center);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.add_css_class("pi-dialog-header");
        header.append(&title);
        header.append(&escape);

        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("action, session or configuration..."));
        entry.set_hexpand(true);

        let list = InlineList::new();
        list.set_max_height(360);

        let footer = gtk::Label::new(Some("↑↓ Navigate · enter open · esc close"));
        footer.add_css_class("dialog-note");
        footer.set_xalign(0.0);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.add_css_class("pi-dialog-body");
        body.set_vexpand(true);
        body.append(&entry);
        body.append(list.widget());
        body.append(&footer);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("pi-dialog");
        root.append(&header);
        root.append(&body);
        window.set_child(Some(&root));

        let palette = Rc::new(Self {
            window: window.clone(),
            entry: entry.clone(),
            list,
            items: RefCell::new(items),
            shown: RefCell::new(Vec::new()),
        });

        let me = palette.clone();
        entry.connect_changed(move |_| me.rebuild());

        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let me = palette.clone();
        key.connect_key_pressed(move |_, key, _, _| match key {
            gtk::gdk::Key::Escape => {
                me.window.close();
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Up | gtk::gdk::Key::KP_Up => {
                me.list.move_selection(-1);
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Down | gtk::gdk::Key::KP_Down => {
                me.list.move_selection(1);
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                me.list.activate_selected();
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        window.add_controller(key);

        palette.rebuild();
        palette
    }

    pub fn present(self: &Rc<Self>) {
        self.window.present();
        self.entry.grab_focus();
        self.list.select_first();
    }

    fn rebuild(self: &Rc<Self>) {
        let query = self.entry.text().to_string();
        let items = self.items.borrow();
        let mut matches: Vec<(i32, usize)> = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                fuzzy_score(&item.haystack(), &query).map(|score| (score, index))
            })
            .collect();
        matches.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        matches.truncate(40);

        self.list.clear();
        self.shown.borrow_mut().clear();

        let mut group = "";
        for (_, index) in &matches {
            let item = &items[*index];
            if item.group != group {
                group = item.group;
                if !group.is_empty() {
                    self.list.note(group);
                }
            }
            let row = row_widget(&item.name, &item.hint);
            let action = item.action.clone();
            let me = self.clone();
            self.list.add(&row, move || {
                me.window.close();
                action();
            });
            self.shown.borrow_mut().push(*index);
        }

        if matches.is_empty() {
            self.list.note("Nothing married to the filter");
        }
        self.list.open();
    }

    #[allow(dead_code)]
    pub fn visible(&self) -> usize {
        self.shown.borrow().len()
    }
}

fn row_widget(name: &str, hint: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some(name));
    label.add_css_class("palette-name");
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&label);
    if !hint.is_empty() {
        let hint = gtk::Label::new(Some(hint));
        hint.add_css_class("palette-desc");
        hint.set_xalign(1.0);
        hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&hint);
    }
    row
}

pub fn show(parent: &impl IsA<gtk::Window>, items: Vec<Item>) -> Rc<Palette> {
    let palette = Palette::new(parent, items);
    palette.present();
    palette
}


