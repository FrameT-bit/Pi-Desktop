
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

type Action = Rc<dyn Fn()>;

struct Row {
    widget: gtk::Box,
    action: Action,
}

const ROW_HEIGHT: i32 = 30;

fn scroll_body(list: &gtk::Box, max_height: i32) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .max_content_height(max_height)
        .propagate_natural_height(true)
        .can_focus(false)
        .child(list)
        .build()
}

fn clickable_row(
    content: &impl IsA<gtk::Widget>,
    action: impl Fn() + 'static,
) -> (gtk::Box, Action) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("palette-row");
    row.set_cursor_from_name(Some("pointer"));
    row.set_can_focus(false);
    row.append(content);

    let action: Action = Rc::new(action);
    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    let action_c = action.clone();
    click.connect_released(move |_, _, _, _| action_c());
    row.add_controller(click);
    (row, action)
}

fn select_row(
    rows: &[Row],
    selected: &Cell<usize>,
    index: usize,
    list: &gtk::Box,
    scroll: &gtk::ScrolledWindow,
) {
    if rows.is_empty() {
        return;
    }
    let index = index.min(rows.len() - 1);
    for (i, row) in rows.iter().enumerate() {
        if i == index {
            row.widget.add_css_class("selected");
        } else {
            row.widget.remove_css_class("selected");
        }
    }
    selected.set(index);

    if let Some(row) = rows.get(index) {
        if let Some(bounds) = row.widget.compute_bounds(list) {
            let adj = scroll.vadjustment();
            let top = bounds.y() as f64;
            let bottom = top + bounds.height() as f64;
            if top < adj.value() {
                adj.set_value(top);
            } else if bottom > adj.value() + adj.page_size() {
                adj.set_value(bottom - adj.page_size());
            }
        }
    }
}


pub struct InlineList {
    card: gtk::Box,
    list: gtk::Box,
    scroll: gtk::ScrolledWindow,
    rows: RefCell<Vec<Row>>,
    selected: Cell<usize>,
}

impl InlineList {
    pub fn new() -> Rc<Self> {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("palette-inline");
        card.set_can_focus(false);
        card.set_visible(false);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("palette-list");
        list.set_can_focus(false);

        let scroll = scroll_body(&list, 320);
        scroll.add_css_class("palette-scroll");
        card.append(&scroll);

        Rc::new(Self {
            card,
            list,
            scroll,
            rows: RefCell::new(Vec::new()),
            selected: Cell::new(0),
        })
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.card
    }

    pub fn set_bottom_margin(&self, bottom: i32) {
        self.card.set_margin_bottom(bottom);
    }

    pub fn clear(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.rows.borrow_mut().clear();
        self.selected.set(0);
    }

    pub fn is_open(&self) -> bool {
        self.card.is_visible()
    }

    pub fn len(&self) -> usize {
        self.rows.borrow().len()
    }

    pub fn selected_index(&self) -> usize {
        self.selected.get()
    }

    pub fn add(&self, content: &impl IsA<gtk::Widget>, action: impl Fn() + 'static) {
        let (row, action) = clickable_row(content, action);
        self.list.append(&row);
        self.rows.borrow_mut().push(Row { widget: row, action });
    }

    pub fn note(&self, text: &str) {
        let label = gtk::Label::new(Some(text));
        label.add_css_class("palette-note");
        label.set_xalign(0.0);
        self.list.append(&label);
    }

    pub fn select_first(&self) {
        select_row(&self.rows.borrow(), &self.selected, 0, &self.list, &self.scroll);
    }

    pub fn move_selection(&self, delta: isize) {
        let rows = self.rows.borrow();
        if rows.is_empty() {
            return;
        }
        let current = self.selected.get() as isize;
        let next = (current + delta).rem_euclid(rows.len() as isize);
        select_row(&rows, &self.selected, next as usize, &self.list, &self.scroll);
    }

    pub fn activate_selected(&self) {
        let rows = self.rows.borrow();
        if let Some(row) = rows.get(self.selected.get()) {
            let action = row.action.clone();
            drop(rows);
            action();
        }
    }

    pub fn set_max_height(&self, height: i32) {
        self.scroll.set_max_content_height(height);
    }

    pub fn open(&self) {
        let rows = self.rows.borrow().len() as i32;
        let height = if rows == 0 { 30 } else { rows * ROW_HEIGHT };
        let cap = self.scroll.max_content_height().max(ROW_HEIGHT);
        self.scroll.set_min_content_height(height.clamp(ROW_HEIGHT, cap));
        if self.is_open() {
            self.select_first();
            return;
        }
        self.card.set_visible(true);
        self.select_first();
    }

    pub fn close(&self) {
        if self.is_open() {
            self.card.set_visible(false);
        }
    }
}


pub struct Popup {
    pub popover: gtk::Popover,
    pub list: gtk::Box,
    pub search: gtk::Entry,
    scroll: gtk::ScrolledWindow,
    rows: RefCell<Vec<Row>>,
    selected: Cell<usize>,
    width: i32,
}

impl Popup {
    pub fn new(position: gtk::PositionType, width: i32) -> Rc<Self> {
        let popover = gtk::Popover::new();
        popover.add_css_class("palette-popup");
        popover.set_has_arrow(false);
        popover.set_autohide(true);
        popover.set_position(position);
        popover.set_can_focus(false);
        popover.set_size_request(width, -1);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("palette-list");

        let search = gtk::Entry::new();
        search.add_css_class("palette-search-entry");
        search.set_placeholder_text(Some("filter model..."));
        search.set_visible(false);

        let scroll = scroll_body(&list, 420);
        scroll.add_css_class("palette-scroll");

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.append(&search);
        body.append(&scroll);
        popover.set_child(Some(&body));

        Rc::new(Self {
            popover,
            list,
            search,
            scroll,
            rows: RefCell::new(Vec::new()),
            selected: Cell::new(0),
            width,
        })
    }

    pub fn attach(&self, widget: &impl IsA<gtk::Widget>) {
        self.popover.set_parent(widget);
    }

    pub fn clear(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.rows.borrow_mut().clear();
        self.selected.set(0);
    }

    pub fn is_open(&self) -> bool {
        self.popover.is_visible()
    }

    pub fn len(&self) -> usize {
        self.rows.borrow().len()
    }

    pub fn selected_index(&self) -> usize {
        self.selected.get()
    }

    pub fn add(&self, content: &impl IsA<gtk::Widget>, action: impl Fn() + 'static) {
        let (row, action) = clickable_row(content, action);
        self.list.append(&row);
        self.rows.borrow_mut().push(Row { widget: row, action });
    }

    pub fn heading(&self, text: &str) {
        let label = gtk::Label::new(Some(text));
        label.add_css_class("palette-group");
        label.set_xalign(0.0);
        self.list.append(&label);
    }

    pub fn note(&self, text: &str) {
        let label = gtk::Label::new(Some(text));
        label.add_css_class("palette-note");
        label.set_xalign(0.0);
        self.list.append(&label);
    }

    pub fn select_first(&self) {
        self.select_at(0);
    }

    pub fn move_selection(&self, delta: isize) {
        let rows = self.rows.borrow();
        if rows.is_empty() {
            return;
        }
        let current = self.selected.get() as isize;
        let next = (current + delta).rem_euclid(rows.len() as isize);
        select_row(&rows, &self.selected, next as usize, &self.list, &self.scroll);
    }

    pub fn activate_selected(&self) {
        let rows = self.rows.borrow();
        if let Some(row) = rows.get(self.selected.get()) {
            let action = row.action.clone();
            drop(rows);
            action();
        }
    }

    pub fn enable_search(&self) {
        self.search.set_visible(true);
    }

    pub fn focus_search(&self) {
        if gtk::prelude::WidgetExt::is_visible(&self.search) {
            self.search.grab_focus();
        }
    }

    pub fn select_index(&self, index: usize) {
        self.select_at(index);
    }

    fn select_at(&self, index: usize) {
        select_row(&self.rows.borrow(), &self.selected, index, &self.list, &self.scroll);
    }

    pub fn open(&self, width: i32) {
        if width > 0 && width != self.width {
            self.popover.set_size_request(width, -1);
        }
        if self.is_open() {
            return;
        }
        self.popover.popup();
        self.select_first();
        self.focus_search();
    }

    pub fn close(&self) {
        if gtk::prelude::WidgetExt::is_visible(&self.search) && !self.search.text().is_empty() {
            self.search.set_text("");
        }
        self.popover.popdown();
    }
}
