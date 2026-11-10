
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;

use crate::icons::{self, Tone};
use crate::session_ops::{project_label, session_slug};
use crate::state::{AppState, ModelItem, RemoteSession, PI_VERSION};

use super::menu;

pub type SelectFn = Box<dyn Fn(usize)>;
pub type RowFn = Box<dyn Fn(usize)>;
pub type RenameFn = Box<dyn Fn(usize, String)>;

const SEARCH_DEBOUNCE_MS: u64 = 150;
const CLICK_DELAY_MS: u64 = 200;

pub struct NavRow {
    pub button: gtk::Button,
    pub indicator: gtk::Box,
    pub count: gtk::Label,
    icon: gtk::Image,
    icon_name: &'static str,
}

#[derive(Clone)]
pub struct SidebarUi {
    inner: Rc<SidebarInner>,
}

pub struct SidebarInner {
    pub root: gtk::Box,
    pub new_session: gtk::Button,
    new_session_label: gtk::Label,
    nav_box: gtk::Box,
    pub nav: Vec<NavRow>,
    section_label: gtk::Label,
    divider: gtk::Separator,
    footer: gtk::Box,
    pub session_list: gtk::Box,
    pub session_scroll: gtk::ScrolledWindow,
    pub search_entry: gtk::Entry,
    search_toggle: gtk::Button,
    on_session: RefCell<Option<SelectFn>>,
    on_rename: RefCell<Option<RenameFn>>,
    on_duplicate: RefCell<Option<RowFn>>,
    on_delete: RefCell<Option<RowFn>>,
    on_export: RefCell<Option<RowFn>>,
    on_open_side: RefCell<Option<RowFn>>,
    current_session: RefCell<String>,
    all: RefCell<Vec<(usize, RemoteSession)>>,
    models: RefCell<Vec<ModelItem>>,
    active: Cell<Option<usize>>,
    collapsed: RefCell<HashSet<String>>,
    query: RefCell<String>,
    debounce: RefCell<Option<glib::SourceId>>,
    pending_select: RefCell<Option<glib::SourceId>>,
    row_widgets: RefCell<Vec<(usize, gtk::Box, gtk::Box, gtk::Label, String)>>,
}

impl Deref for SidebarUi {
    type Target = SidebarInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl SidebarUi {
    pub fn build(state: &AppState) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("sidebar");
        root.set_width_request(264);
        root.set_vexpand(true);

        let new_session = gtk::Button::new();
        new_session.add_css_class("new-session");
        new_session.set_halign(gtk::Align::Fill);
        let new_session_body = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        new_session_body.set_halign(gtk::Align::Center);
        let new_session_icon = icons::icon("plus", Tone::Inverse);
        let new_session_label = gtk::Label::new(Some("New session"));
        new_session_label.add_css_class("new-session-label");
        new_session_body.append(&new_session_icon);
        new_session_body.append(&new_session_label);
        new_session.set_child(Some(&new_session_body));
        new_session.set_tooltip_text(Some("new session (Ctrl+N)"));
        root.append(&new_session);

        let nav_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let items = [
            ("Artifacts", "box", state.artifacts),
            ("Tasks", "clock", state.jobs),
            ("Sessions", "list", state.sessions.len()),
            ("Tree", "tree", 0),
            ("Extensions", "plug", state.plugins.len()),
            ("Git", "fork", 0),
        ];
        let mut nav = Vec::new();
        for (index, (label, icon, count)) in items.into_iter().enumerate() {
            let row = build_nav_row(label, icon, count);
            row.button.set_tooltip_text(Some(match label {
                "Artifacts" => "artifacts generated in this session",
                "Tasks" => "loops and auto-chained tasks",
                "Sessions" => "history of project sessions",
                "Tree" => "ramifications and fork points of the session",
                "Extensions" => "Pi packages installed and catalog",
                "Git" => "review of workspace: diff, internship and commit",
                _ => "",
            }));
            if index == 2 {
                row.button.add_css_class("active");
                row.indicator.set_visible(true);
            }
            nav_box.append(&row.button);
            nav.push(row);
        }
        root.append(&nav_box);

        let section_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        section_row.add_css_class("section-row");
        let section = gtk::Label::new(Some("Recent"));
        section.add_css_class("section-label");
        section.set_xalign(0.0);
        section.set_hexpand(true);
        let search_toggle = gtk::Button::new();
        search_toggle.add_css_class("sidebar-search-toggle");
        search_toggle.set_child(Some(&icons::icon("search", Tone::Muted)));
        search_toggle.set_tooltip_text(Some("search sessions (Ctrl+Shift+F)"));
        search_toggle.set_valign(gtk::Align::Center);
        section_row.append(&section);
        section_row.append(&search_toggle);
        root.append(&section_row);

        let search_entry = gtk::Entry::new();
        search_entry.add_css_class("sidebar-search");
        search_entry.set_placeholder_text(Some("name, model or first prompt"));
        search_entry.set_visible(false);
        search_entry.set_margin_start(12);
        search_entry.set_margin_end(12);
        search_entry.set_margin_bottom(4);
        root.append(&search_entry);

        let session_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        session_list.set_margin_bottom(12);

        let session_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Always)
            .overlay_scrolling(false)
            .vexpand(true)
            .child(&session_list)
            .build();
        session_scroll.add_css_class("session-scroll");
        session_scroll.set_margin_bottom(12);
        root.append(&session_scroll);

        let divider = gtk::Separator::new(gtk::Orientation::Horizontal);
        divider.add_css_class("sidebar-divider");
        root.append(&divider);

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.add_css_class("sidebar-footer");

        let version = gtk::Label::new(Some(&format!("pi {PI_VERSION}")));
        version.add_css_class("footer-version");

        let spacer_footer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer_footer.set_hexpand(true);

        footer.append(&version);
        footer.append(&spacer_footer);
        root.append(&footer);

        let inner = Rc::new(SidebarInner {
            root,
            new_session,
            new_session_label,
            nav_box,
            nav,
            section_label: section,
            divider,
            footer,
            session_list,
            session_scroll,
            search_entry,
            search_toggle,
            on_session: RefCell::new(None),
            on_rename: RefCell::new(None),
            on_duplicate: RefCell::new(None),
            on_delete: RefCell::new(None),
            on_export: RefCell::new(None),
            on_open_side: RefCell::new(None),
            current_session: RefCell::new(String::new()),
            all: RefCell::new(Vec::new()),
            models: RefCell::new(Vec::new()),
            active: Cell::new(None),
            collapsed: RefCell::new(HashSet::new()),
            query: RefCell::new(String::new()),
            debounce: RefCell::new(None),
            pending_select: RefCell::new(None),
            row_widgets: RefCell::new(Vec::new()),
        });
        let sidebar = Self { inner };
        sidebar.set_active(2);
        sidebar.wire_search();
        sidebar.set_sessions(&state.sessions, state.active_session, &state.models);
        sidebar
    }

    pub fn set_chat_mode(&self, chat: bool) {
        self.nav_box.set_visible(!chat);
        self.new_session_label
            .set_text(if chat { "New conversation" } else { "New session" });
        self.divider.set_visible(!chat);
        self.footer.set_visible(!chat);
    }

    fn wire_search(self: &Self) {
        {
            let inner = self.inner.clone();
            self.search_toggle.connect_clicked(move |_| {
                if gtk::prelude::WidgetExt::is_visible(&inner.search_entry) {
                    inner.clear_search();
                } else {
                    inner.search_entry.set_visible(true);
                    inner.search_entry.grab_focus();
                }
            });
        }

        {
            let inner = self.inner.clone();
            self.search_entry.connect_changed(move |entry| {
                inner.schedule_search(entry.text().to_string());
            });
        }

        {
            let inner = self.inner.clone();
            let key = gtk::EventControllerKey::new();
            key.set_propagation_phase(gtk::PropagationPhase::Capture);
            key.connect_key_pressed(move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape {
                    inner.clear_search();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            self.search_entry.add_controller(key);
        }
    }

    pub fn open_search(&self) {
        self.search_entry.set_visible(true);
        self.search_entry.grab_focus();
    }

    pub fn set_query(&self, query: &str) {
        self.search_entry.set_visible(true);
        self.search_entry.set_text(query);
    }

    pub fn set_on_session(&self, callback: impl Fn(usize) + 'static) {
        *self.on_session.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_rename(&self, callback: impl Fn(usize, String) + 'static) {
        *self.on_rename.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_duplicate(&self, callback: impl Fn(usize) + 'static) {
        *self.on_duplicate.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_delete(&self, callback: impl Fn(usize) + 'static) {
        *self.on_delete.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_export(&self, callback: impl Fn(usize) + 'static) {
        *self.on_export.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_open_side(&self, callback: impl Fn(usize) + 'static) {
        *self.on_open_side.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_current_session(&self, id: &str) {
        *self.current_session.borrow_mut() = id.to_string();
    }

    pub fn set_sessions(
        &self,
        sessions: &[RemoteSession],
        active: Option<usize>,
        models: &[ModelItem],
    ) {
        *self.all.borrow_mut() = sessions.iter().cloned().enumerate().collect();
        *self.models.borrow_mut() = models.to_vec();
        self.active.set(active);
        self.inner.rebuild();
    }

    pub fn set_counts(
        &self,
        artifacts: usize,
        jobs: usize,
        sessions: usize,
        tree: usize,
        plugins: usize,
    ) {
        let counts = [artifacts, jobs, sessions, tree, plugins];
        for (row, count) in self.nav.iter().zip(counts) {
            row.count.set_text(&count.to_string());
            row.count.set_visible(count > 0);
        }
    }

    pub fn set_artifact_count(&self, count: usize) {
        if let Some(row) = self.nav.first() {
            row.count.set_text(&count.to_string());
            row.count.set_visible(count > 0);
        }
    }

    pub fn set_git_count(&self, count: usize) {
        if let Some(row) = self.nav.get(5) {
            row.count.set_text(&count.to_string());
            row.count.set_visible(count > 0);
        }
    }

    pub fn set_job_count(&self, count: usize) {
        if let Some(row) = self.nav.get(1) {
            row.count.set_text(&count.to_string());
            row.count.set_visible(count > 0);
        }
    }

    pub fn set_active(&self, index: usize) {
        for (i, row) in self.nav.iter().enumerate() {
            if i == index {
                row.button.add_css_class("active");
                row.indicator.set_visible(true);
                icons::set_icon(&row.icon, row.icon_name, Tone::Accent);
            } else {
                row.button.remove_css_class("active");
                row.indicator.set_visible(false);
                icons::set_icon(&row.icon, row.icon_name, Tone::Muted);
            }
        }
    }

    pub fn scroll_sessions_top(&self) {
        glib::idle_add_local_once({
            let adj = self.session_scroll.vadjustment();
            move || adj.set_value(0.0)
        });
    }

    pub fn open_context_menu(&self, index: usize) {
        let found = self
            .row_widgets
            .borrow()
            .iter()
            .find(|entry| entry.0 == index)
            .map(|entry| {
                (
                    entry.1.clone(),
                    entry.2.clone(),
                    entry.3.clone(),
                    entry.4.clone(),
                )
            });
        if let Some((row, body, name, title)) = found {
            self.inner
                .show_row_menu(&row, index, &body, &name, &title);
        }
    }

    pub fn open_rename(&self, index: usize) {
        let found = self
            .row_widgets
            .borrow()
            .iter()
            .find(|entry| entry.0 == index)
            .map(|entry| (entry.2.clone(), entry.3.clone(), entry.4.clone()));
        if let Some((body, name, title)) = found {
            self.inner.start_rename(&body, &name, index, &title);
        }
    }
}

impl SidebarInner {
    fn rebuild(self: &Rc<Self>) {
        while let Some(child) = self.session_list.first_child() {
            self.session_list.remove(&child);
        }
        self.row_widgets.borrow_mut().clear();

        let models = self.models.borrow();
        let query = self.query.borrow().trim().to_lowercase();
        let sessions = self.all.borrow();
        if sessions.is_empty() {
            self.list_message(if query.is_empty() {
                "no session loaded"
            } else {
                "no session found"
            });
            return;
        }

        let mut groups: Vec<(String, i64, Vec<(usize, RemoteSession)>)> = Vec::new();
        for (index, session) in sessions.iter() {
            if !query.is_empty() && !matches_query(session, &models, &query) {
                continue;
            }
            let slug = session_slug(&session.path);
            match groups.iter_mut().find(|(key, ..)| key == &slug) {
                Some((_, mtime, rows)) => {
                    *mtime = (*mtime).max(session.mtime);
                    rows.push((*index, session.clone()));
                }
                None => groups.push((slug, session.mtime, vec![(*index, session.clone())])),
            }
        }

        if groups.is_empty() {
            self.list_message("no session found");
            return;
        }

        groups.sort_by(|left, right| right.1.cmp(&left.1));
        for (slug, _, mut rows) in groups {
            rows.sort_by(|left, right| right.1.mtime.cmp(&left.1.mtime));
            let collapsed = query.is_empty() && self.collapsed.borrow().contains(&slug);
            self.session_list
                .append(&self.group_header(&slug, rows.len(), collapsed));
            if collapsed {
                continue;
            }
            for (index, session) in rows {
                let row = self.build_row(index, &session, &models);
                self.session_list.append(&row);
            }
        }
    }

    fn list_message(&self, text: &str) {
        let empty = gtk::Label::new(Some(text));
        empty.add_css_class("session-empty");
        empty.set_xalign(0.0);
        empty.set_margin_start(12);
        empty.set_margin_end(12);
        self.session_list.append(&empty);
    }

    fn schedule_search(self: &Rc<Self>, text: String) {
        if let Some(source) = self.debounce.borrow_mut().take() {
            source.remove();
        }
        let inner = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(SEARCH_DEBOUNCE_MS), move || {
            inner.debounce.borrow_mut().take();
            *inner.query.borrow_mut() = text;
            inner.rebuild();
        });
        *self.debounce.borrow_mut() = Some(id);
    }

    fn clear_search(self: &Rc<Self>) {
        self.search_entry.set_text("");
        self.search_entry.set_visible(false);
        *self.query.borrow_mut() = String::new();
        self.rebuild();
    }

    fn group_header(
        self: &Rc<Self>,
        slug: &str,
        count: usize,
        collapsed: bool,
    ) -> gtk::Button {
        let label = if slug.is_empty() {
            "no design".to_string()
        } else {
            project_label(slug)
        };
        let chevron = icons::icon(
            if collapsed { "chevron-right" } else { "chevron-down" },
            Tone::Muted,
        );
        chevron.add_css_class("group-chevron");

        let name = gtk::Label::new(Some(&label));
        name.add_css_class("session-group-name");
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_hexpand(true);
        name.set_tooltip_text(Some(slug));

        let count_label = gtk::Label::new(Some(&count.to_string()));
        count_label.add_css_class("session-group-count");

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        body.set_hexpand(true);
        body.append(&chevron);
        body.append(&name);
        body.append(&count_label);

        let button = gtk::Button::new();
        button.add_css_class("session-group");
        button.set_halign(gtk::Align::Fill);
        button.set_child(Some(&body));

        let key = slug.to_string();
        let inner = self.clone();
        button.connect_clicked(move |_| {
            if !inner.collapsed.borrow_mut().remove(&key) {
                inner.collapsed.borrow_mut().insert(key.clone());
            }
            inner.rebuild();
        });
        button
    }

    fn build_row(
        self: &Rc<Self>,
        index: usize,
        session: &RemoteSession,
        models: &[ModelItem],
    ) -> gtk::Box {
        let is_active = self.active.get() == Some(index);
        let title = session.title();

        let indicator = gtk::Box::new(gtk::Orientation::Vertical, 0);
        indicator.add_css_class("nav-indicator");
        indicator.set_valign(gtk::Align::Fill);
        indicator.set_visible(is_active);

        let name = gtk::Label::new(Some(&title));
        name.add_css_class("session-name");
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_lines(1);
        name.set_tooltip_text(Some(&title));

        let meta = gtk::Label::new(Some(&session.model_label(models)));
        meta.add_css_class("session-meta");
        meta.set_xalign(0.0);
        meta.set_ellipsize(gtk::pango::EllipsizeMode::End);
        meta.set_lines(1);
        meta.set_hexpand(true);

        let age = gtk::Label::new(Some(&session.age()));
        age.add_css_class("session-age");
        age.set_xalign(1.0);
        age.set_valign(gtk::Align::End);

        let meta_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        meta_row.set_hexpand(true);
        meta_row.append(&meta);
        if !age.text().is_empty() {
            meta_row.append(&key_dot());
            meta_row.append(&age);
        }

        let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
        body.set_hexpand(true);
        body.append(&name);
        body.append(&meta_row);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.add_css_class("session-item");
        row.set_hexpand(true);
        row.append(&indicator);
        row.append(&body);
        body.set_margin_start(10);
        if is_active {
            row.add_css_class("active");
        }
        row.set_cursor_from_name(Some("pointer"));
        row.set_tooltip_text(Some(&format!("{} · {}", title, session.age())));
        self.row_widgets.borrow_mut().push((
            index,
            row.clone(),
            body.clone(),
            name.clone(),
            title.clone(),
        ));

        {
            let inner = self.clone();
            let click = gtk::GestureClick::new();
            click.set_button(gtk::gdk::BUTTON_PRIMARY);
            click.connect_pressed(move |_, n_press, _, _| {
                if n_press != 1 {
                    return;
                }
                inner.cancel_pending_select();
                let inner_c = inner.clone();
                let source =
                    glib::timeout_add_local_once(Duration::from_millis(CLICK_DELAY_MS), move || {
                        inner_c.pending_select.borrow_mut().take();
                        if let Some(callback) = inner_c.on_session.borrow().as_ref() {
                            callback(index);
                        }
                    });
                *inner.pending_select.borrow_mut() = Some(source);
            });
            row.add_controller(click);
        }

        {
            let inner = self.clone();
            let name_c = name.clone();
            let body_c = body.clone();
            let title_c = title.clone();
            let rename = gtk::GestureClick::new();
            rename.set_button(gtk::gdk::BUTTON_PRIMARY);
            rename.connect_pressed(move |gesture, n_press, _, _| {
                if n_press != 2 {
                    return;
                }
                gesture.set_state(gtk::EventSequenceState::Claimed);
                inner.cancel_pending_select();
                inner.start_rename(&body_c, &name_c, index, &title_c);
            });
            name.add_controller(rename);
        }

        {
            let inner = self.clone();
            let row_c = row.clone();
            let body_c = body.clone();
            let name_c = name.clone();
            let title_c = title.clone();
            let menu_gesture = gtk::GestureClick::new();
            menu_gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
            menu_gesture.connect_pressed(move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                inner.cancel_pending_select();
                inner.show_row_menu(&row_c, index, &body_c, &name_c, &title_c);
            });
            row.add_controller(menu_gesture);
        }

        row
    }

    fn cancel_pending_select(&self) {
        if let Some(source) = self.pending_select.borrow_mut().take() {
            source.remove();
        }
    }

    fn start_rename(self: &Rc<Self>, body: &gtk::Box, name: &gtk::Label, index: usize, current: &str) {
        if body.first_child().is_some_and(|child| child.is::<gtk::Entry>()) {
            return;
        }
        let entry = gtk::Entry::new();
        entry.add_css_class("session-rename");
        entry.set_text(current);
        entry.set_width_chars(8);
        entry.set_hexpand(true);
        body.prepend(&entry);
        name.set_visible(false);

        let finished = Rc::new(Cell::new(false));
        let body_c = body.clone();
        let name_c = name.clone();
        let entry_c = entry.clone();
        let current = current.to_string();

        let commit: Rc<dyn Fn()> = Rc::new({
            let finished = finished.clone();
            let inner = self.clone();
            let body_c = body_c.clone();
            let name_c = name_c.clone();
            let entry_c = entry_c.clone();
            let current = current.clone();
            move || {
                if finished.replace(true) {
                    return;
                }
                let value = entry_c.text().trim().to_string();
                body_c.remove(&entry_c);
                name_c.set_visible(true);
                if value.is_empty() || value == current {
                    return;
                }
                name_c.set_text(&value);
                if let Some(callback) = inner.on_rename.borrow().as_ref() {
                    callback(index, value);
                }
            }
        });
        {
            let commit = commit.clone();
            entry.connect_activate(move |_| commit());
        }
        {
            let commit = commit.clone();
            let focus = gtk::EventControllerFocus::new();
            focus.connect_leave(move |_| commit());
            entry.add_controller(focus);
        }
        {
            let finished = finished.clone();
            let body_c = body_c.clone();
            let name_c = name_c.clone();
            let entry_c = entry_c.clone();
            let key = gtk::EventControllerKey::new();
            key.set_propagation_phase(gtk::PropagationPhase::Capture);
            key.connect_key_pressed(move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape {
                    finished.set(true);
                    body_c.remove(&entry_c);
                    name_c.set_visible(true);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            entry.add_controller(key);
        }

        glib::idle_add_local_once({
            let entry = entry.clone();
            move || {
                entry.grab_focus();
                entry.select_region(0, -1);
            }
        });
    }

    fn show_row_menu(
        self: &Rc<Self>,
        row: &gtk::Box,
        index: usize,
        body: &gtk::Box,
        name: &gtk::Label,
        title: &str,
    ) {
        let mut items: Vec<menu::Item> = Vec::new();

        {
            let inner = self.clone();
            let body = body.clone();
            let name = name.clone();
            let title = title.to_string();
            items.push(menu::Item::new("Rename", move || {
                inner.start_rename(&body, &name, index, &title)
            }));
        }
        {
            let inner = self.clone();
            items.push(menu::Item::new("Duplicate", move || {
                if let Some(callback) = inner.on_duplicate.borrow().as_ref() {
                    callback(index);
                }
            }));
        }
        {
            let session_id = self
                .all
                .borrow()
                .iter()
                .find(|(row_index, _)| *row_index == index)
                .map(|(_, session)| session.session_id.clone())
                .unwrap_or_default();
            if session_id.is_empty() {
                items.push(menu::Item::disabled(
                    "Open Next",
                    "session without id — can't open next door",
                ));
            } else if session_id == *self.current_session.borrow() {
                items.push(menu::Item::disabled(
                    "Open Next",
                    "this session is already open in this window",
                ));
            } else {
                let inner = self.clone();
                items.push(menu::Item::new("Open Next", move || {
                    if let Some(callback) = inner.on_open_side.borrow().as_ref() {
                        callback(index);
                    }
                }));
            }
        }
        {
            let inner = self.clone();
            items.push(menu::Item::new("Delete", move || {
                if let Some(callback) = inner.on_delete.borrow().as_ref() {
                    callback(index);
                }
            }));
        }
        {
            let inner = self.clone();
            items.push(menu::Item::new("Export html", move || {
                if let Some(callback) = inner.on_export.borrow().as_ref() {
                    callback(index);
                }
            }));
        }

        menu::show_items(row, items);
    }
}

fn key_dot() -> gtk::Label {
    let dot = gtk::Label::new(Some("·"));
    dot.add_css_class("session-dot");
    dot.set_valign(gtk::Align::End);
    dot
}

fn matches_query(session: &RemoteSession, models: &[ModelItem], query: &str) -> bool {
    if session.title().to_lowercase().contains(query) {
        return true;
    }
    if session.model_label(models).to_lowercase().contains(query) {
        return true;
    }
    session
        .preview
        .as_deref()
        .map(|preview| preview.to_lowercase().contains(query))
        .unwrap_or(false)
}

fn build_nav_row(label: &str, icon_name: &'static str, count: usize) -> NavRow {
    let indicator = gtk::Box::new(gtk::Orientation::Vertical, 0);
    indicator.add_css_class("nav-indicator");
    indicator.set_valign(gtk::Align::Fill);
    indicator.set_visible(false);

    let image = icons::icon(icon_name, Tone::Muted);
    image.add_css_class("nav-icon");

    let text = gtk::Label::new(Some(label));
    text.add_css_class("nav-label");
    text.set_xalign(0.0);

    let count_label = gtk::Label::new(Some(&count.to_string()));
    count_label.add_css_class("nav-count");
    count_label.set_xalign(1.0);
    count_label.set_visible(count > 0);

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    body.set_hexpand(true);
    body.append(&image);
    body.append(&text);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    body.append(&spacer);
    body.append(&count_label);

    let button = gtk::Button::new();
    button.add_css_class("nav-item");
    button.set_halign(gtk::Align::Fill);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_hexpand(true);
    row.append(&indicator);
    row.append(&body);
    body.set_margin_start(10);
    button.set_child(Some(&row));

    NavRow {
        button,
        indicator,
        count: count_label,
        icon: image,
        icon_name,
    }
}
