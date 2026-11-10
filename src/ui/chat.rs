
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::prelude::*;
use serde_json::{json, Value};

use crate::diff;
use crate::icons::{self, Tone};
use crate::images::{self, ImageBlock};
use crate::markdown;
use crate::theme::Theme;

use super::thumb;

const CHAT_MARGIN: i32 = 24;

pub struct ChatView {
    pub root: gtk::Overlay,
    pub scroll: gtk::ScrolledWindow,
    pub list: gtk::Box,
    empty: gtk::Box,
    empty_quick: gtk::Box,
    empty_hint: gtk::Label,
    inner: RefCell<Inner>,
    messages: RefCell<Vec<Value>>,
    show_timestamps: Cell<bool>,
    auto_scroll: Rc<Cell<bool>>,
    follow: Rc<Cell<bool>>,
    follow_scheduled: Rc<Cell<bool>>,
    programmatic: Rc<Cell<bool>>,
    thinking_rows: RefCell<Vec<ThinkingRow>>,
    width: Cell<i32>,
    on_user_menu: RefCell<Option<Rc<dyn Fn(gtk::Box, String)>>>,
    on_assistant_menu: RefCell<Option<Rc<dyn Fn(gtk::Box)>>>,
    diff_cards: RefCell<Vec<std::rc::Weak<DiffState>>>,
    find: RefCell<Find>,
    last_think_follow: Cell<Option<std::time::Instant>>,
    on_empty_action: RefCell<Option<Box<dyn Fn(&str)>>>,
    chat_mode: Cell<bool>,
    tool_rows: RefCell<Vec<ToolRow>>,
}

struct Find {
    bar: gtk::Box,
    entry: gtk::Entry,
    count: gtk::Label,
    prev: gtk::Button,
    next: gtk::Button,
    close: gtk::Button,
    hits: Vec<(gtk::Label, usize)>,
    index: usize,
}

fn glyph_button(glyph: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::with_label(glyph);
    button.add_css_class("find-button");
    button.set_tooltip_text(Some(tooltip));
    button.set_valign(gtk::Align::Center);
    button
}

fn collect_labels(widget: &impl IsA<gtk::Widget>, callback: &mut impl FnMut(&gtk::Label)) {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Ok(label) = current.clone().downcast::<gtk::Label>() {
            callback(&label);
        }
        collect_labels(&current, callback);
        child = current.next_sibling();
    }
}

#[derive(Default)]
struct Inner {
    streams: HashMap<i64, Stream>,
    thinking: Option<ThinkingRow>,
    tools: HashMap<String, ToolRow>,
}

struct Stream {
    wrapper: gtk::Box,
    label: gtk::Label,
    text: String,
    rendered: usize,
}

impl ChatView {
    pub fn new(theme: &Theme) -> Rc<Self> {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
        list.add_css_class("chat-area");
        list.set_valign(gtk::Align::Start);
        list.set_margin_top(16);
        list.set_margin_bottom(16);
        list.set_margin_start(CHAT_MARGIN);
        list.set_margin_end(CHAT_MARGIN);

        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .build();
        scroll.add_css_class("chat-scroll");
        scroll.set_child(Some(&list));

        let mark = gtk::Image::new();
        mark.add_css_class("chat-empty-mark");
        mark.set_pixel_size(64);
        if let Ok(texture) = gtk::gdk::Texture::from_bytes(&glib::Bytes::from_static(include_bytes!(
            "../../resources/brand/logo-128.png"
        ))) {
            mark.set_paintable(Some(&texture));
        }
        let hint = gtk::Label::new(Some(
            "Enter sends · / commands · Ctrl+K all · Ctrl+F search · Ctrl+/ shortcuts",
        ));
        hint.add_css_class("chat-empty-hint");
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 14);
        empty.add_css_class("chat-empty");
        empty.set_halign(gtk::Align::Center);
        empty.set_valign(gtk::Align::Center);

        let quick = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        quick.set_halign(gtk::Align::Center);
        let empty_buttons: Vec<(String, gtk::Button)> = [
            ("provider", "configure provider"),
            ("sessions", "search for session"),
            ("new", "new prompt"),
            ("hotkeys", "atalhos"),
        ]
        .into_iter()
        .map(|(id, label)| {
            let button = gtk::Button::with_label(label);
            button.add_css_class("empty-chip");
            quick.append(&button);
            (id.to_string(), button)
        })
        .collect();
        empty.append(&mark);
        empty.append(&quick);
        empty.append(&hint);

        let root = gtk::Overlay::new();
        root.set_child(Some(&scroll));

        let find_entry = gtk::Entry::new();
        find_entry.add_css_class("find-entry");
        find_entry.set_placeholder_text(Some("search chat..."));
        find_entry.set_hexpand(true);

        let find_count = gtk::Label::new(None);
        find_count.add_css_class("find-count");

        let find_prev = glyph_button("↑", "Previous (Shift+Enter)");
        let find_next = glyph_button("↓", "Next (Enter)");
        let find_close = glyph_button("✕", "Close (Sc)");

        let find_bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        find_bar.add_css_class("find-bar");
        find_bar.set_halign(gtk::Align::End);
        find_bar.set_valign(gtk::Align::Start);
        find_bar.set_margin_top(8);
        find_bar.set_margin_end(18);
        find_bar.append(&find_entry);
        find_bar.append(&find_count);
        find_bar.append(&find_prev);
        find_bar.append(&find_next);
        find_bar.append(&find_close);
        find_bar.set_visible(false);
        root.add_overlay(&find_bar);

        root.add_overlay(&empty);
        root.set_hexpand(true);
        root.set_vexpand(true);

        let view = Rc::new(Self {
            root,
            scroll,
            list,
            empty,
            empty_quick: quick.clone(),
            empty_hint: hint.clone(),
            inner: RefCell::new(Inner::default()),
            messages: RefCell::new(Vec::new()),
            show_timestamps: Cell::new(true),
            auto_scroll: Rc::new(Cell::new(true)),
            follow: Rc::new(Cell::new(true)),
            follow_scheduled: Rc::new(Cell::new(false)),
            programmatic: Rc::new(Cell::new(false)),
            thinking_rows: RefCell::new(Vec::new()),
            width: Cell::new(78),
            on_user_menu: RefCell::new(None),
            on_assistant_menu: RefCell::new(None),
            diff_cards: RefCell::new(Vec::new()),
            on_empty_action: RefCell::new(None),
            chat_mode: Cell::new(false),
            tool_rows: RefCell::new(Vec::new()),
            last_think_follow: Cell::new(None),
            find: RefCell::new(Find {
                entry: find_entry,
                count: find_count,
                bar: find_bar,
                prev: find_prev,
                next: find_next,
                close: find_close,
                hits: Vec::new(),
                index: 0,
            }),
        });
        view.wire_follow();
        view.wire_find();
        view.wire_empty(&empty_buttons);
        let weak = Rc::downgrade(&view);
        theme.connect_changed(move || {
            if let Some(view) = weak.upgrade() {
                view.rerender();
            }
        });
        view
    }

    pub fn set_on_empty_action(&self, callback: impl Fn(&str) + 'static) {
        *self.on_empty_action.borrow_mut() = Some(Box::new(callback));
    }

    pub fn is_empty(&self) -> bool {
        self.messages.borrow().is_empty()
    }

    pub fn set_chat_mode(&self, chat: bool) {
        self.chat_mode.set(chat);
        self.root.remove_css_class("chat-mode");
        if chat {
            self.root.add_css_class("chat-mode");
        }
        self.empty_quick.set_visible(!chat);
        self.empty_hint.set_text(if chat {
            "Ask me something."
        } else {
            "Enter sends · / commands · Ctrl+K all · Ctrl+F search · Ctrl+/ shortcuts"
        });
        for row in self.tool_rows.borrow().iter() {
            row.set_chat_mode(chat);
        }
        for row in self.thinking_rows.borrow().iter() {
            row.set_chat_mode(chat);
        }
    }

    fn remember_tool(&self, row: &ToolRow) {
        row.set_chat_mode(self.chat_mode.get());
        let mut rows = self.tool_rows.borrow_mut();
        rows.push(row.clone());
        let excess = rows.len().saturating_sub(128);
        if excess > 0 {
            rows.drain(..excess);
        }
    }

    fn wire_empty(self: &Rc<Self>, buttons: &[(String, gtk::Button)]) {
        for (id, button) in buttons {
            let me = self.clone();
            let id = id.clone();
            button.connect_clicked(move |_| {
                if let Some(callback) = me.on_empty_action.borrow().as_ref() {
                    callback(&id);
                }
            });
        }
    }

    fn wire_find(self: &Rc<Self>) {
        let me = self.clone();
        self.find.borrow().entry.connect_changed(move |_| {
            let query = me.find.borrow().entry.text().to_string();
            me.find_apply(&query);
        });

        let me = self.clone();
        self.find.borrow().entry.connect_activate(move |_| me.find_step(1));

        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let me = self.clone();
        key.connect_key_pressed(move |_, key, _, state| {
            let shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            match key {
                gtk::gdk::Key::Escape => {
                    me.find_close();
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                    me.find_step(if shift { -1 } else { 1 });
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Up | gtk::gdk::Key::KP_Up => {
                    me.find_step(-1);
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Down | gtk::gdk::Key::KP_Down => {
                    me.find_step(1);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.find.borrow().entry.add_controller(key);

        let me = self.clone();
        self.find
            .borrow()
            .prev
            .connect_clicked(move |_| me.find_step(-1));
        let me = self.clone();
        self.find
            .borrow()
            .next
            .connect_clicked(move |_| me.find_step(1));
        let me = self.clone();
        self.find
            .borrow()
            .close
            .connect_clicked(move |_| me.find_close());
    }

    pub fn find_open(self: &Rc<Self>) {
        {
            let find = self.find.borrow();
            find.bar.set_visible(true);
            find.entry.grab_focus();
            find.entry.select_region(0, -1);
        }
        let query = self.find.borrow().entry.text().to_string();
        if !query.is_empty() {
            self.find_apply(&query);
        }
    }

    fn find_reset(&self) {
        let mut find = self.find.borrow_mut();
        find.hits.clear();
        find.index = 0;
        find.count.set_text("");
    }

    pub fn find_query(&self, text: &str) {
        let entry = self.find.borrow().entry.clone();
        entry.set_text(text);
    }

    pub fn find_is_open(&self) -> bool {
        self.find.borrow().bar.is_visible()
    }

    pub fn find_close(&self) {
        self.find.borrow().bar.set_visible(false);
        let hits = std::mem::take(&mut self.find.borrow_mut().hits);
        for (label, _) in hits {
            label.remove_css_class("find-hit");
            label.remove_css_class("find-current");
        }
        self.find.borrow_mut().index = 0;
        self.find.borrow().count.set_text("");
        let entry = self.find.borrow().entry.clone();
        entry.set_text("");
    }

    fn find_apply(self: &Rc<Self>, query: &str) {
        let mut find = self.find.borrow_mut();
        for (label, _) in &find.hits {
            label.remove_css_class("find-hit");
            label.remove_css_class("find-current");
        }
        find.hits.clear();
        find.index = 0;

        if query.trim().is_empty() {
            find.count.set_text("");
            return;
        }

        let needle = query.to_lowercase();
        collect_labels(&self.list, &mut |label| {
            if find.hits.len() >= 500 {
                return;
            }
            if label.text().to_lowercase().contains(&needle) {
                label.add_css_class("find-hit");
                find.hits.push((label.clone(), 0));
            }
        });

        let total = find.hits.len();
        find.count.set_text(&format!("0/{total}"));
        drop(find);
        if total > 0 {
            self.find_step(1);
        }
    }

    fn find_step(self: &Rc<Self>, delta: isize) {
        let mut find = self.find.borrow_mut();
        let total = find.hits.len();
        if total == 0 {
            find.count.set_text("0/0");
            return;
        }
        let next = (find.index as isize + delta).rem_euclid(total as isize) as usize;
        for (label, _) in &find.hits {
            label.remove_css_class("find-current");
        }
        find.index = next;
        let label = find.hits[next].0.clone();
        label.add_css_class("find-current");
        find.count.set_text(&format!("{}/{total}", next + 1));
        drop(find);
        self.scroll_to_label(&label);
    }

    fn scroll_to_label(&self, label: &gtk::Label) {
        let Some(bounds) = label.compute_bounds(&self.list) else {
            return;
        };
        let adj = self.scroll.vadjustment();
        let center = bounds.y() as f64 + bounds.height() as f64 / 2.0;
        let target = (center - adj.page_size() / 2.0).clamp(0.0, adj.upper() - adj.page_size());
        adj.set_value(target.max(0.0));
    }

    fn hide_empty(&self) {
        self.empty.set_visible(false);
    }

    pub fn set_show_timestamps(&self, enabled: bool) {
        if self.show_timestamps.get() == enabled {
            return;
        }
        self.show_timestamps.set(enabled);
        self.rerender();
    }

    pub fn set_auto_scroll(&self, enabled: bool) {
        self.auto_scroll.set(enabled);
        if enabled {
            self.scroll_to_bottom();
        }
    }

    pub fn set_chat_width(&self, width: i32) {
        let width = width.clamp(60, 120);
        if self.width.get() == width {
            return;
        }
        self.width.set(width);
        markdown::set_width(width);
        self.rerender();
    }

    fn stamp(&self) -> Option<gtk::Label> {
        if !self.show_timestamps.get() {
            return None;
        }
        let text = glib::DateTime::now_local()
            .ok()
            .and_then(|now| now.format("%H:%M").ok())
            .map(|text| text.to_string())
            .unwrap_or_default();
        let label = gtk::Label::new(Some(&text));
        label.add_css_class("chat-stamp");
        label.set_xalign(0.0);
        label.set_halign(gtk::Align::Start);
        Some(label)
    }

    fn push_stamped(&self, widget: &impl IsA<gtk::Widget>) {
        match self.stamp() {
            Some(stamp) => {
                let block = gtk::Box::new(gtk::Orientation::Vertical, 2);
                block.append(&stamp);
                block.append(widget);
                self.push(&block);
            }
            None => self.push(widget),
        }
    }

    fn wire_follow(self: &Rc<Self>) {
        let follow = self.follow.clone();
        let guard = self.programmatic.clone();
        self.scroll.vadjustment().connect_value_changed(move |adj| {
            if guard.get() {
                return;
            }
            follow.set(adj.upper() - adj.page_size() - adj.value() <= 24.0);
        });
    }

    fn maybe_follow(&self) {
        if !self.auto_scroll.get() || !self.follow.get() {
            return;
        }
        if self.follow_scheduled.replace(true) {
            return;
        }
        let adj = self.scroll.vadjustment();
        let flag = self.follow_scheduled.clone();
        let guard = self.programmatic.clone();
        let adj_idle = adj.clone();
        glib::idle_add_local_once(move || {
            flag.set(false);
            jump(&adj_idle, &guard);
        });

        let auto = self.auto_scroll.clone();
        let follow = self.follow.clone();
        let guard = self.programmatic.clone();
        for delay in [80u64, 240] {
            let adj = adj.clone();
            let auto = auto.clone();
            let follow = follow.clone();
            let guard = guard.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(delay), move || {
                if auto.get() && follow.get() {
                    jump(&adj, &guard);
                }
            });
        }
    }

    fn scroll_to_bottom(&self) {
        self.follow.set(true);
        self.maybe_follow();
        let adj = self.scroll.vadjustment();
        let guard = self.programmatic.clone();
        glib::idle_add_local_once(move || jump(&adj, &guard));
    }

    fn settle_to_bottom(&self) {
        let adj = self.scroll.vadjustment();
        for delay in [100u64, 300, 600] {
            let adj = adj.clone();
            let guard = self.programmatic.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(delay), move || {
                jump(&adj, &guard)
            });
        }
    }

    fn push(&self, widget: &impl IsA<gtk::Widget>) {
        self.hide_empty();
        widget.add_css_class("enter");
        self.list.append(widget);
        let widget = widget.as_ref().clone();
        glib::idle_add_local_once(move || widget.remove_css_class("enter"));
        self.maybe_follow();
    }

    pub fn append_user(&self, text: &str) {
        self.append_user_mark(text, None, &[]);
    }

    pub fn append_user_marked(&self, text: &str, mark: &str) {
        self.append_user_mark(text, Some(mark), &[]);
    }

    pub fn append_user_images(&self, text: &str, images: &[ImageBlock]) {
        self.append_user_mark(text, None, images);
    }

    fn append_user_mark(&self, text: &str, mark: Option<&str>, images: &[ImageBlock]) {
        let message = user_message(text, mark, images);
        self.messages.borrow_mut().push(message);
        let block = user_block(text, mark, self.width.get(), images);
        self.wire_user_menu(&block, text);
        self.push_stamped(&block);
    }

    pub fn append_user_record(&self, message: &Value) {
        let images = images::blocks_from_message(message);
        if images.is_empty() {
            return;
        }
        let text = message_text(message);
        if let Some(last) = self.messages.borrow().last() {
            if last.get("role").and_then(Value::as_str) == Some("user")
                && same_user_content(last, message)
            {
                return;
            }
        }
        self.messages.borrow_mut().push(message.clone());
        let block = user_block(&text, None, self.width.get(), &images);
        self.wire_user_menu(&block, &text);
        self.push_stamped(&block);
    }

    pub fn set_on_user_menu(&self, callback: impl Fn(gtk::Box, String) + 'static) {
        *self.on_user_menu.borrow_mut() = Some(Rc::new(callback));
    }

    fn wire_user_menu(&self, block: &gtk::Box, text: &str) {
        let Some(callback) = self.on_user_menu.borrow().as_ref().cloned() else {
            return;
        };
        let block_c = block.clone();
        let text = text.to_string();
        let gesture = gtk::GestureClick::new();
        gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        gesture.connect_pressed(move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            callback(block_c.clone(), text.clone());
        });
        block.add_controller(gesture);
    }

    pub fn set_on_assistant_menu(&self, callback: impl Fn(gtk::Box) + 'static) {
        *self.on_assistant_menu.borrow_mut() = Some(Rc::new(callback));
    }

    fn wire_assistant_menu(&self, block: &gtk::Box) {
        let Some(callback) = self.on_assistant_menu.borrow().as_ref().cloned() else {
            return;
        };
        let block_c = block.clone();
        let gesture = gtk::GestureClick::new();
        gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        gesture.connect_pressed(move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            callback(block_c.clone());
        });
        block.add_controller(gesture);
    }

    pub fn expand_diffs(&self) {
        let cards = self.diff_cards.borrow().clone();
        for weak in cards {
            if let Some(state) = weak.upgrade() {
                if !state.expanded.get() {
                    state.button.emit_clicked();
                }
            }
        }
    }

    pub fn scroll_to_last_diff(&self) {
        let cards = self.diff_cards.borrow().clone();
        let Some(state) = cards.iter().rev().find_map(|weak| weak.upgrade()) else {
            return;
        };
        let anchor = state.anchor.clone();
        let list = self.list.clone();
        let adj = self.scroll.vadjustment();
        glib::idle_add_local_once(move || {
            let point = anchor.compute_point(&list, &gtk::graphene::Point::new(0.0, 0.0));
            if let Some(point) = point {
                adj.set_value((point.y() as f64 - 16.0).max(0.0));
            }
        });
    }

    fn register_diff(&self, state: &Rc<DiffState>) {
        let mut cards = self.diff_cards.borrow_mut();
        cards.retain(|weak| weak.strong_count() > 0);
        cards.push(Rc::downgrade(state));
    }

    pub fn begin_assistant(&self) {
        self.inner.borrow_mut().streams.clear();
    }

    fn new_stream(&self) -> Stream {
        let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 8);
        wrapper.set_halign(gtk::Align::Fill);
        wrapper.add_css_class("assistant-msg");

        let label = gtk::Label::new(None);
        label.add_css_class("assistant-text");
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_xalign(0.0);
        label.set_halign(gtk::Align::Start);
        label.set_selectable(true);
        label.set_max_width_chars(self.width.get());
        self.wire_assistant_menu(&wrapper);
        if let Some(stamp) = self.stamp() {
            wrapper.append(&stamp);
        }
        wrapper.append(&label);

        self.hide_empty();
        self.list.append(&wrapper);
        self.maybe_follow();

        Stream {
            wrapper,
            label,
            text: String::new(),
            rendered: 0,
        }
    }

    pub fn assistant_delta(&self, index: i64, delta: &str) {
        let mut inner = self.inner.borrow_mut();
        if !inner.streams.contains_key(&index) {
            let stream = self.new_stream();
            inner.streams.insert(index, stream);
        }
        let Some(stream) = inner.streams.get_mut(&index) else {
            return;
        };
        stream.text.push_str(delta);
        if stream.text.len().saturating_sub(stream.rendered) >= 240
            || delta.contains('\n')
        {
            stream.rendered = stream.text.len();
            stream
                .label
                .set_text(&format!("{}▍", tool_display(&stream.text)));
        }
        drop(inner);
        self.maybe_follow();
    }

    pub fn assistant_final(&self, message: &Value) {
        self.messages.borrow_mut().push(message.clone());
        let blocks = text_blocks(message);
        let mut inner = self.inner.borrow_mut();
        let mut streams = std::mem::take(&mut inner.streams);

        if blocks.is_empty() {
            for (_, stream) in streams {
                self.list.remove(&stream.wrapper);
            }
            return;
        }

        for (index, text) in blocks {
            if let Some(stream) = streams.remove(&index) {
                if text.trim().is_empty() {
                    self.list.remove(&stream.wrapper);
                    continue;
                }
                while let Some(child) = stream.wrapper.first_child() {
                    stream.wrapper.remove(&child);
                }
                for widget in markdown::render(&text) {
                    stream.wrapper.append(&widget);
                }
            } else if !text.trim().is_empty() {
                let stream = self.new_stream();
                for widget in markdown::render(&text) {
                    stream.wrapper.append(&widget);
                }
            }
        }

        for (_, stream) in streams {
            if stream.text.trim().is_empty() {
                self.list.remove(&stream.wrapper);
            }
        }

        self.maybe_follow();
    }

    pub fn seal_stream(&self) {
        let mut inner = self.inner.borrow_mut();
        let streams = std::mem::take(&mut inner.streams);
        for (_, stream) in streams {
            if stream.text.trim().is_empty() {
                self.list.remove(&stream.wrapper);
            } else {
                stream.label.set_text(&tool_display(&stream.text));
            }
        }
    }

    pub fn thinking_start(&self, level: &str) {
        self.seal_stream();
        let row = ThinkingRow::new(level, self.width.get());
        self.remember_thinking(&row);
        let mut inner = self.inner.borrow_mut();
        let handle = row.clone();
        inner.thinking = Some(row);
        drop(inner);
        self.push_stamped(&handle.root);
        handle.show_running();
    }

    fn remember_thinking(&self, row: &ThinkingRow) {
        row.set_chat_mode(self.chat_mode.get());
        let mut rows = self.thinking_rows.borrow_mut();
        rows.push(row.clone());
        let excess = rows.len().saturating_sub(64);
        if excess > 0 {
            rows.drain(..excess);
        }
    }

    pub fn thinking_delta(&self, delta: &str) {
        {
            let inner = self.inner.borrow();
            if let Some(row) = inner.thinking.as_ref() {
                row.push(delta);
            } else {
                drop(inner);
                self.thinking_start("medium");
                self.thinking_delta(delta);
                return;
            }
        }
        let now = std::time::Instant::now();
        let should_follow = self
            .last_think_follow
            .get()
            .map_or(true, |last| now.duration_since(last).as_millis() >= 90);
        if should_follow {
            self.last_think_follow.set(Some(now));
            self.maybe_follow();
        }
    }

    pub fn expand_thinking(&self) {
        {
            let inner = self.inner.borrow();
            if let Some(row) = inner.thinking.as_ref() {
                row.set_expanded(true);
                return;
            }
        }
        if let Some(row) = self.thinking_rows.borrow().last() {
            row.set_expanded(true);
        }
    }

    pub fn thinking_end(&self, content: &str) {
        let mut inner = self.inner.borrow_mut();
        if let Some(row) = inner.thinking.take() {
            if !content.is_empty() {
                row.set_text(content);
                scroll_thinking_to_end(&row.body);
            }
            row.show_done();
        }
        drop(inner);
        self.maybe_follow();
    }

    pub fn tool_start(&self, id: &str, name: &str, args: &Value) {
        self.seal_stream();
        let row = ToolRow::new(name, args, self.width.get());
        row.set_state("pending");
        self.push_stamped(&row.card);
        self.remember_tool(&row);
        self.inner.borrow_mut().tools.insert(id.to_string(), row);
    }

    pub fn tool_update(&self, id: &str, partial: &Value) {
        {
            let inner = self.inner.borrow();
            if let Some(row) = inner.tools.get(id) {
                let text = text_of_result(partial);
                if !text.is_empty() {
                    row.set_output(&text);
                }
            }
        }
        self.maybe_follow();
    }

    pub fn tool_append(&self, id: &str, delta: &str) {
        let mut inner = self.inner.borrow_mut();
        if !inner.tools.contains_key(id) {
            let row = ToolRow::new("bash", &Value::Null, self.width.get());
            row.set_state("pending");
            self.push_stamped(&row.card);
            self.remember_tool(&row);
            inner.tools.insert(id.to_string(), row);
        }
        if let Some(row) = inner.tools.get(id) {
            row.append_output(delta);
        }
        drop(inner);
        self.maybe_follow();
    }

    pub fn bash_finish(
        &self,
        id: &str,
        output: &str,
        exit_code: Option<i64>,
        cancelled: bool,
        truncated: bool,
    ) {
        let mut inner = self.inner.borrow_mut();
        let row = match inner.tools.remove(id) {
            Some(row) => row,
            None => {
                let row = ToolRow::new("bash", &Value::Null, self.width.get());
                self.push_stamped(&row.card);
                self.remember_tool(&row);
                row
            }
        };
        if !output.is_empty() && (!truncated || row.output_text().is_empty()) {
            row.set_output(output);
        }
        let ok = !cancelled && exit_code.unwrap_or(1) == 0;
        row.set_state(if ok { "success" } else { "error" });
        let meta = if cancelled {
            "cancelado".to_string()
        } else {
            let mut meta = match exit_code {
                Some(code) => format!("output {code}"),
                None => "no code".to_string(),
            };
            if truncated {
                meta.push_str("· truncated");
            }
            meta
        };
        row.set_meta(&meta);

        self.messages.borrow_mut().push(json!({
            "role": "toolResult",
            "toolCallId": id,
            "tool": "bash",
            "content": [{"type": "text", "text": row.output_text()}],
            "isError": !ok,
        }));
        drop(inner);
        self.maybe_follow();
    }

    pub fn tool_end(&self, id: &str, result: &Value, is_error: bool, duration_ms: Option<f64>) {
        let mut inner = self.inner.borrow_mut();
        if let Some(row) = inner.tools.remove(id) {
            let text = text_of_result(result);
            if !text.is_empty() {
                row.set_output(&text);
            }
            if let Some(patch) = result
                .get("details")
                .and_then(|details| details.get("patch"))
                .and_then(Value::as_str)
            {
                if row.set_diff(&diff::parse(patch)) {
                    if let Some(state) = row.diff_state() {
                        self.register_diff(&state);
                    }
                }
            }
            row.set_chat_mode(self.chat_mode.get());
            row.set_state(if is_error { "error" } else { "success" });
            if let Some(ms) = duration_ms {
                row.set_meta(&format!("{:.1}s", ms / 1000.0));
            }
            let mut entry = json!({
                "role": "toolResult",
                "toolCallId": id,
                "tool": row.name,
                "content": [{"type": "text", "text": row.output.text().to_string()}],
                "isError": is_error,
            });
            if let Some(details) = result.get("details") {
                entry["details"] = details.clone();
            }
            self.messages.borrow_mut().push(entry);
        }
        drop(inner);
        self.maybe_follow();
    }

    pub fn note(&self, text: &str, is_error: bool) {
        let label = gtk::Label::new(None);
        label.add_css_class("chat-note");
        if is_error {
            label.add_css_class("error");
        }
        label.set_text(text);
        label.set_wrap(true);
        label.set_xalign(0.0);
        label.set_halign(gtk::Align::Start);
        label.set_selectable(true);
        label.set_max_width_chars(self.width.get());
        self.push_stamped(&label);
    }

    pub fn clear(&self) {
        self.find_reset();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        *self.inner.borrow_mut() = Inner::default();
        self.messages.borrow_mut().clear();
        self.diff_cards.borrow_mut().clear();
        self.thinking_rows.borrow_mut().clear();
        self.tool_rows.borrow_mut().clear();
        self.empty.set_visible(true);
    }

    pub fn load_messages(&self, messages: &[Value]) {
        self.clear();
        *self.messages.borrow_mut() = messages.to_vec();
        self.render_messages(messages);
        self.scroll_to_bottom();
        self.settle_to_bottom();
    }

    fn render_messages(&self, messages: &[Value]) {
        for message in messages {
            let role = message.get("role").and_then(Value::as_str).unwrap_or("");
            match role {
                "user" => {
                    let text = message_text(message);
                    let images = images::blocks_from_message(message);
                    if !text.trim().is_empty() || !images.is_empty() {
                        let mark = message.get("mark").and_then(Value::as_str);
                        let block = user_block(&text, mark, self.width.get(), &images);
                        self.wire_user_menu(&block, &text);
                        self.push_stamped(&block);
                    }
                }
                "assistant" => {
                    for block in history_blocks(message) {
                        match block {
                            HistBlock::Thinking(text) => {
                                if text.trim().is_empty() {
                                    continue;
                                }
                                let row = ThinkingRow::new("medium", self.width.get());
                                row.set_chat_mode(self.chat_mode.get());
                                row.set_text(&text);
                                row.show_done();
                                self.remember_thinking(&row);
                                self.push_stamped(&row.root);
                            }
                            HistBlock::Text(text) => {
                                if text.trim().is_empty() {
                                    continue;
                                }
                                self.append_assistant(&text);
                            }
                        }
                    }
                }
                "toolResult" => {
                    let row = ToolRow::from_history(message, self.width.get());
                    if let Some(state) = row.diff_state() {
                        self.register_diff(&state);
                    }
                    self.remember_tool(&row);
                    self.push_stamped(&row.card);
                }
                _ => {}
            }
        }
    }

    fn rerender(&self) {
        self.find_reset();
        let messages = self.messages.borrow().clone();
        let live_think: Option<String> = {
            let inner = self.inner.borrow();
            inner
                .thinking
                .as_ref()
                .map(|row| row.text.borrow().clone())
                .filter(|text| !text.trim().is_empty())
        };
        let partial: Vec<(i64, String)> = {
            let inner = self.inner.borrow();
            let mut streams: Vec<(i64, String)> = inner
                .streams
                .iter()
                .map(|(index, stream)| (*index, stream.text.clone()))
                .collect();
            streams.sort_by_key(|(index, _)| *index);
            streams
                .into_iter()
                .filter(|(_, text)| !text.trim().is_empty())
                .collect()
        };
        if messages.is_empty() && partial.is_empty() {
            return;
        }

        *self.inner.borrow_mut() = Inner::default();
        self.diff_cards.borrow_mut().clear();
        let _ = &live_think;
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.render_messages(&messages);
        for (index, text) in partial {
            let mut stream = self.new_stream();
            stream.label.set_text(&text);
            stream.text = text;
            self.inner.borrow_mut().streams.insert(index, stream);
        }
        if let Some(text) = live_think {
            if !text.trim().is_empty() {
                let row = ThinkingRow::new("medium", self.width.get());
                row.set_chat_mode(self.chat_mode.get());
                row.set_text(&text);
                row.show_running();
                scroll_thinking_to_end(&row.body);
                self.push_stamped(&row.root);
                self.inner.borrow_mut().thinking = Some(row);
            }
        }
        self.empty
            .set_visible(messages.is_empty() && self.inner.borrow().streams.is_empty());
        self.scroll_to_bottom();
    }

    fn append_assistant(&self, text: &str) {
        let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 8);
        wrapper.set_halign(gtk::Align::Fill);
        wrapper.add_css_class("assistant-msg");
        self.wire_assistant_menu(&wrapper);
        for widget in markdown::render(text) {
            wrapper.append(&widget);
        }
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&wrapper));
        let actions = message_actions(text);
        actions.set_halign(gtk::Align::End);
        actions.set_valign(gtk::Align::Start);
        overlay.add_overlay(&actions);

        let motion = gtk::EventControllerMotion::new();
        {
            let actions = actions.clone();
            motion.connect_enter(move |_, _, _| actions.set_visible(true));
        }
        {
            let actions = actions.clone();
            motion.connect_leave(move |_| actions.set_visible(false));
        }
        overlay.add_controller(motion);

        self.push_stamped(&overlay);
    }
}

fn message_actions(text: &str) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    bar.add_css_class("msg-actions");
    bar.set_visible(std::env::var("PI_DESKTOP_FORCE_ACTIONS").is_ok());

    let copy = gtk::Button::new();
    copy.add_css_class("msg-action");
    copy.set_tooltip_text(Some("copy"));
    copy.set_child(Some(&crate::icons::icon("copy", crate::icons::Tone::Muted)));
    let payload = text.to_string();
    copy.connect_clicked(move |_| {
        if let Some(display) = gtk::gdk::Display::default() {
            display.clipboard().set_text(&payload);
        }
    });
    bar.append(&copy);
    bar
}

fn user_block(text: &str, mark: Option<&str>, width: i32, images: &[ImageBlock]) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    block.set_halign(gtk::Align::Start);
    block.add_css_class("user-msg");

    if let Some(mark) = mark {
        let badge = gtk::Label::new(Some(mark));
        badge.add_css_class("user-mark");
        if mark != "STEER" {
            badge.add_css_class("queue");
        }
        badge.set_xalign(0.0);
        badge.set_halign(gtk::Align::Start);
        badge.set_margin_bottom(4);
        block.append(&badge);
    }

    if !text.trim().is_empty() {
        let label = gtk::Label::new(None);
        label.set_text(text);
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_xalign(0.0);
        label.set_selectable(true);
        label.set_max_width_chars(width);
        block.append(&label);
    }

    for image in images {
        if let Some(widget) = thumb::from_base64(&image.data, 220, "user-image") {
            widget.set_tooltip_text(Some(&image.mime));
            block.append(&widget);
        }
    }

    block
}

fn user_message(text: &str, mark: Option<&str>, images: &[ImageBlock]) -> Value {
    let mut content = Vec::new();
    if !text.is_empty() {
        content.push(json!({"type": "text", "text": text}));
    }
    for image in images {
        content.push(json!({
            "type": "image",
            "data": image.data,
            "mimeType": image.mime,
        }));
    }
    let mut message = json!({"role": "user", "content": content});
    if let Some(mark) = mark {
        message["mark"] = json!(mark);
    }
    message
}

fn same_user_content(left: &Value, right: &Value) -> bool {
    message_text(left).trim() == message_text(right).trim()
        && images::blocks_from_message(left) == images::blocks_from_message(right)
}

enum HistBlock {
    Text(String),
    Thinking(String),
}

fn history_blocks(message: &Value) -> Vec<HistBlock> {
    let Some(Value::Array(blocks)) = message.get("content") else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter_map(|block| match block.get("type").and_then(Value::as_str) {
            Some("text") => block
                .get("text")
                .and_then(Value::as_str)
                .map(|text| HistBlock::Text(text.to_string())),
            Some("thinking") => block
                .get("thinking")
                .and_then(Value::as_str)
                .map(|text| HistBlock::Thinking(text.to_string())),
            _ => None,
        })
        .collect()
}

fn text_blocks(message: &Value) -> Vec<(i64, String)> {
    let Some(Value::Array(blocks)) = message.get("content") else {
        return Vec::new();
    };
    blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            if block.get("type").and_then(Value::as_str) != Some("text") {
                return None;
            }
            let text = block.get("text").and_then(Value::as_str)?.to_string();
            Some((index as i64, text))
        })
        .collect()
}

fn message_text(message: &Value) -> String {
    match message.get("content") {
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

const MAX_TOOL_OUTPUT: usize = 20_000;

fn truncate_on_char_boundary(mut text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }
    let mut cut = max;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text.truncate(cut);
    text
}

pub(crate) fn text_of_result(result: &Value) -> String {
    let mut out = String::new();
    match result.get("content") {
        Some(Value::Array(blocks)) => {
            for block in blocks {
                if block.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        out.push_str(text);
                    }
                }
            }
        }
        Some(Value::String(text)) => out.push_str(text),
        _ => {
            if let Some(text) = result.get("output").and_then(Value::as_str) {
                out.push_str(text);
            } else if let Some(text) = result.as_str() {
                out.push_str(text);
            }
        }
    }
    if out.len() > MAX_TOOL_OUTPUT {
        out = truncate_on_char_boundary(out, MAX_TOOL_OUTPUT);
        out.push_str("\n... (output truncated into IU)");
    }
    out
}

fn tool_result_name(message: &Value) -> String {
    for key in ["tool", "tool", "name"] {
        if let Some(name) = message.get(key).and_then(Value::as_str) {
            if !name.trim().is_empty() {
                return name.to_string();
            }
        }
    }
    if let Some(id) = message.get("toolCallId").and_then(Value::as_str) {
        if !id.trim().is_empty() {
            return id.to_string();
        }
    }
    "tool".to_string()
}

fn tool_result_text(message: &Value) -> String {
    text_of_result(message)
}

fn tool_result_is_error(message: &Value) -> bool {
    message
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}


const DIFF_COLLAPSED_LINES: usize = 8;

struct DiffState {
    rows: Vec<gtk::Widget>,
    button: gtk::Button,
    chevron: gtk::Image,
    anchor: gtk::Widget,
    expanded: Cell<bool>,
}

impl DiffState {
    fn apply(&self, expanded: bool) {
        self.expanded.set(expanded);
        for (index, row) in self.rows.iter().enumerate() {
            row.set_visible(expanded || index < DIFF_COLLAPSED_LINES);
        }
        icons::set_icon(
            &self.chevron,
            if expanded { "chevron-down" } else { "chevron-right" },
            Tone::Muted,
        );
    }
}

#[derive(Clone)]
struct ToolRow {
    card: gtk::Box,
    output: gtk::Label,
    meta: gtk::Label,
    title: gtk::Label,
    adds: gtk::Label,
    dels: gtk::Label,
    status: gtk::Label,
    status_icon: gtk::Image,
    head_button: gtk::Button,
    chevron: gtk::Image,
    file_icon: gtk::Image,
    diff_body: gtk::Box,
    diff_state: Rc<RefCell<Option<Rc<DiffState>>>>,
    name: String,
    text: RefCell<String>,
    rendered: Cell<usize>,
    expanded: Rc<Cell<bool>>,
    chat_mode: Rc<Cell<bool>>,
}

impl ToolRow {
    fn new(name: &str, args: &Value, width: i32) -> Self {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("tool-card");
        card.set_halign(gtk::Align::Fill);

        let chevron = icons::icon("chevron-down", Tone::Muted);
        chevron.add_css_class("tool-chevron");

        let file_icon = icons::icon("edit", Tone::Muted);
        file_icon.add_css_class("tool-file-icon");
        file_icon.set_visible(false);

        let title = gtk::Label::new(None);
        title.add_css_class("tool-title");
        title.set_text(name);
        title.set_xalign(0.0);

        let meta = gtk::Label::new(None);
        meta.add_css_class("tool-meta");
        meta.set_xalign(0.0);
        meta.set_hexpand(true);
        meta.set_ellipsize(gtk::pango::EllipsizeMode::End);
        meta.set_text(&args_preview(args));

        let adds = gtk::Label::new(None);
        adds.add_css_class("tool-adds");
        adds.set_visible(false);
        let dels = gtk::Label::new(None);
        dels.add_css_class("tool-dels");
        dels.set_visible(false);

        let status = gtk::Label::new(None);
        status.add_css_class("tool-status");
        status.set_xalign(1.0);

        let status_icon = icons::icon("check", Tone::Accent);
        status_icon.add_css_class("tool-status-icon");
        status_icon.set_visible(false);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.append(&chevron);
        header.append(&file_icon);
        header.append(&title);
        header.append(&meta);
        header.append(&adds);
        header.append(&dels);
        header.append(&status_icon);
        header.append(&status);

        let head_button = gtk::Button::new();
        head_button.add_css_class("tool-head");
        head_button.set_child(Some(&header));

        let output = gtk::Label::new(None);
        output.add_css_class("tool-output");
        output.set_xalign(0.0);
        output.set_wrap(true);
        output.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        output.set_selectable(true);
        output.set_max_width_chars(width);

        let diff_body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        diff_body.add_css_class("diff-body");
        diff_body.set_visible(false);

        card.append(&head_button);
        card.append(&output);
        card.append(&diff_body);

        let expanded = Rc::new(Cell::new(true));
        let expanded_c = expanded.clone();
        let chat_mode = Rc::new(Cell::new(false));
        let chat_mode_c = chat_mode.clone();
        let diff_body_c = diff_body.clone();

        let diff_state: Rc<RefCell<Option<Rc<DiffState>>>> = Rc::new(RefCell::new(None));
        let chevron_c = chevron.clone();
        let output_c = output.clone();
        let diff_state_c = diff_state.clone();
        head_button.connect_clicked(move |_| {
            if let Some(state) = diff_state_c.borrow().as_ref() {
                let next = !state.expanded.get();
                state.apply(next);
                if chat_mode_c.get() {
                    diff_body_c.set_visible(next);
                }
                return;
            }
            let next = !expanded_c.get();
            expanded_c.set(next);
            icons::set_icon(
                &chevron_c,
                if next { "chevron-down" } else { "chevron-right" },
                Tone::Muted,
            );
            output_c.set_visible(next);
        });

        Self {
            card,
            output,
            meta,
            title,
            adds,
            dels,
            status,
            status_icon,
            head_button,
            chevron,
            file_icon,
            diff_body,
            diff_state,
            name: name.to_string(),
            text: RefCell::new(String::new()),
            rendered: Cell::new(0),
            expanded,
            chat_mode,
        }
    }

    fn apply_expanded(&self, next: bool) {
        self.expanded.set(next);
        icons::set_icon(
            &self.chevron,
            if next { "chevron-down" } else { "chevron-right" },
            Tone::Muted,
        );
        if let Some(state) = self.diff_state.borrow().as_ref() {
            state.apply(next);
            return;
        }
        self.diff_body.set_visible(false);
        self.output.set_visible(next);
    }

    fn set_chat_mode(&self, chat: bool) {
        self.chat_mode.set(chat);
        self.card.remove_css_class("tool-compact");
        if chat {
            self.card.add_css_class("tool-compact");
            self.apply_expanded(false);
            self.diff_body.set_visible(false);
            if self.card.has_css_class("error") {
                self.apply_expanded(true);
                self.diff_body.set_visible(false);
            }
        } else {
            self.apply_expanded(true);
            self.diff_body.set_visible(self.diff_state.borrow().is_some());
        }
    }

    fn from_history(message: &Value, width: i32) -> Self {
        let row = Self::new(&tool_result_name(message), &Value::Null, width);
        let text = tool_result_text(message);
        if !text.is_empty() {
            row.set_output(&text);
        }
        if let Some(patch) = message
            .get("details")
            .and_then(|details| details.get("patch"))
            .and_then(Value::as_str)
        {
            row.set_diff(&diff::parse(patch));
        }
        row.set_state(if tool_result_is_error(message) {
            "error"
        } else {
            "success"
        });
        row
    }

    fn set_diff(&self, diff: &diff::Diff) -> bool {
        if diff.is_empty() {
            return false;
        }

        while let Some(child) = self.diff_body.first_child() {
            self.diff_body.remove(&child);
        }
        let mut rows: Vec<gtk::Widget> = Vec::new();
        let mut first_path = String::new();
        for file in &diff.files {
            if first_path.is_empty() {
                first_path = file.path.clone();
            }
            for hunk in &file.hunks {
                let split = hunk
                    .lines
                    .iter()
                    .take_while(|line| line.kind == diff::LineKind::Meta)
                    .count();
                let header = diff::Line {
                    kind: diff::LineKind::Meta,
                    old_no: None,
                    new_no: None,
                    text: hunk.header.clone(),
                };
                for line in &hunk.lines[..split] {
                    let row = diff_line_widget(line);
                    self.diff_body.append(&row);
                    rows.push(row.upcast());
                }
                let row = diff_line_widget(&header);
                self.diff_body.append(&row);
                rows.push(row.upcast());
                for line in &hunk.lines[split..] {
                    let row = diff_line_widget(line);
                    self.diff_body.append(&row);
                    rows.push(row.upcast());
                }
            }
        }

        let file = &diff.files[0];
        self.file_icon.set_visible(true);
        self.title.set_text(file.name());
        self.title.set_tooltip_text(Some(&first_path));
        self.meta.set_text("");

        let adds = diff.adds();
        if adds > 0 {
            self.adds.set_text(&format!("+{adds}"));
            self.adds.set_visible(true);
        } else {
            self.adds.set_visible(false);
        }
        let dels = diff.dels();
        if dels > 0 {
            self.dels.set_text(&format!("-{dels}"));
            self.dels.set_visible(true);
        } else {
            self.dels.set_visible(false);
        }

        let state = Rc::new(DiffState {
            rows,
            button: self.head_button.clone(),
            chevron: self.chevron.clone(),
            anchor: self.diff_body.clone().upcast(),
            expanded: Cell::new(false),
        });
        state.apply(state.rows.len() <= DIFF_COLLAPSED_LINES);
        *self.diff_state.borrow_mut() = Some(state);

        self.diff_body.set_visible(true);
        self.output.set_visible(false);
        true
    }

    fn diff_state(&self) -> Option<Rc<DiffState>> {
        self.diff_state.borrow().clone()
    }

    fn set_state(&self, state: &str) {
        for class in ["pending", "success", "error", "ok"] {
            self.card.remove_css_class(class);
            self.status.remove_css_class(class);
        }
        self.card.add_css_class(state);
        let (label, class) = match state {
            "success" => ("ok", "ok"),
            "error" => ("error", "error"),
            _ => ("…", "pending"),
        };
        match state {
            "success" => {
                icons::set_icon(&self.status_icon, "check", Tone::Accent);
                self.status_icon.set_visible(true);
            }
            "error" => {
                icons::set_icon(&self.status_icon, "cross", Tone::Accent);
                self.status_icon.set_visible(true);
            }
            _ => {
                self.status_icon.set_visible(false);
            }
        }
        self.status.add_css_class(class);
        self.status.set_text(label);
        if state == "error" && self.chat_mode.get() {
            self.apply_expanded(true);
        }
    }

    fn set_output(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
        self.rendered.set(text.len());
        self.output.set_text(&tool_display(text));
    }

    fn append_output(&self, delta: &str) {
        let mut text = self.text.borrow_mut();
        text.push_str(delta);
        if text.len() > TOOL_STORE_CHARS * 2 {
            let keep_from = text.len() - TOOL_STORE_CHARS;
            text.drain(..keep_from);
            self.rendered.set(0);
        }
        if text.len().saturating_sub(self.rendered.get()) >= TOOL_RENDER_STEP
            || delta.contains('\n')
        {
            self.rendered.set(text.len());
            self.output.set_text(&tool_display(&text));
        }
    }

    fn output_text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_meta(&self, text: &str) {
        self.meta.set_text(text);
    }
}

fn diff_line_widget(line: &diff::Line) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.add_css_class("diff-line");
    match line.kind {
        diff::LineKind::Add => row.add_css_class("add"),
        diff::LineKind::Del => row.add_css_class("del"),
        diff::LineKind::Meta => row.add_css_class("meta"),
        diff::LineKind::Context => row.add_css_class("context"),
    }

    if line.kind != diff::LineKind::Meta {
        row.append(&diff_number(line.old_no));
        row.append(&diff_number(line.new_no));
    }

    let text = gtk::Label::new(Some(&line.text));
    text.add_css_class("diff-text");
    text.set_xalign(0.0);
    text.set_halign(gtk::Align::Fill);
    text.set_hexpand(true);
    text.set_selectable(true);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&text);
    row
}

fn diff_number(number: Option<u32>) -> gtk::Label {
    let text = number.map(|value| value.to_string()).unwrap_or_default();
    let label = gtk::Label::new(Some(&text));
    label.add_css_class("diff-no");
    label.set_width_chars(4);
    label.set_xalign(1.0);
    label.set_halign(gtk::Align::Start);
    label
}

fn args_preview(args: &Value) -> String {
    let Some(map) = args.as_object() else {
        return String::new();
    };
    let keys = [
        "command", "path", "pattern", "query", "url", "file", "description", "prompt",
    ];
    for key in keys {
        if let Some(value) = map.get(key).and_then(Value::as_str) {
            return truncate_line(value);
        }
    }
    if let Some((key, value)) = map.iter().next() {
        let rendered = match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        return truncate_line(&format!("{key}: {rendered}"));
    }
    String::new()
}

fn truncate_line(text: &str) -> String {
    let single = text.lines().next().unwrap_or("");
    if single.chars().count() > 64 {
        let mut out: String = single.chars().take(64).collect();
        out.push('…');
        out
    } else {
        single.to_string()
    }
}

fn jump(adj: &gtk::Adjustment, guard: &Cell<bool>) {
    guard.set(true);
    adj.set_value((adj.upper() - adj.page_size()).max(0.0));
    guard.set(false);
}


const THINK_LINES: i32 = 4;
const THINK_LINE_HEIGHT: i32 = 18;
const THINK_MAX_HEIGHT: i32 = THINK_LINES * THINK_LINE_HEIGHT;

const THINK_RENDER_MS: u64 = 90;
const THINK_TAIL_CHARS: usize = 1_200;
const THINK_FULL_CHARS: usize = 40_000;
const THINK_STORE_CHARS: usize = 400_000;
const THINK_EXPANDED_HEIGHT: i32 = 360;

const TOOL_RENDER_STEP: usize = 800;
const TOOL_DISPLAY_CHARS: usize = 60_000;
const TOOL_STORE_CHARS: usize = 400_000;

fn tool_display(text: &str) -> String {
    let count = text.chars().count();
    if count <= TOOL_DISPLAY_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(TOOL_DISPLAY_CHARS / 2).collect();
    let tail: String = text
        .chars()
        .rev()
        .take(TOOL_DISPLAY_CHARS / 2)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!(
        "{head}\n… [{} characters hidden in display] …\n{tail}",
        count - TOOL_DISPLAY_CHARS
    )
}

fn think_tail(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let start = text
        .char_indices()
        .rev()
        .nth(max)
        .map(|(index, _)| index)
        .unwrap_or(0);
    format!("…{}", &text[start..])
}

fn think_clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .nth(max)
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    format!("{}…\n[thinking truncated in display]", &text[..end])
}

fn thinking_height(expanded: bool) -> i32 {
    if expanded {
        THINK_EXPANDED_HEIGHT
    } else {
        THINK_MAX_HEIGHT
    }
}

#[derive(Clone)]
struct ThinkingRow {
    root: gtk::Box,
    title: gtk::Label,
    chevron: gtk::Image,
    body: gtk::ScrolledWindow,
    content: gtk::Label,
    text: Rc<RefCell<String>>,
    expanded: Rc<Cell<bool>>,
    render_scheduled: Rc<Cell<bool>>,
    chat_mode: Cell<bool>,
}

impl ThinkingRow {
    fn new(level: &str, width: i32) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let chevron = icons::icon("chevron-right", Tone::Muted);
        chevron.add_css_class("tool-chevron");

        let title = gtk::Label::new(Some("Thinking..."));
        title.add_css_class("thinking-title");
        title.set_xalign(0.0);
        set_thinking_level(&title, level);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        head.append(&chevron);
        head.append(&title);

        let head_button = gtk::Button::new();
        head_button.add_css_class("thinking-head");
        head_button.set_halign(gtk::Align::Fill);
        head_button.set_tooltip_text(Some("click to expand or collapse"));
        head_button.set_cursor_from_name(Some("pointer"));
        head.set_hexpand(true);
        head_button.set_child(Some(&head));

        let content = gtk::Label::new(None);
        content.add_css_class("thinking-text");
        content.set_xalign(0.0);
        content.set_wrap(true);
        content.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        content.set_selectable(true);
        content.set_max_width_chars(width);
        content.set_hexpand(true);

        let body = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .max_content_height(thinking_height(false))
            .build();
        body.add_css_class("thinking-body");
        body.set_hexpand(true);
        body.set_child(Some(&content));

        root.append(&head_button);
        root.append(&body);

        let expanded = Rc::new(Cell::new(false));
        let row = Self {
            root,
            title,
            chevron,
            body,
            content,
            text: Rc::new(RefCell::new(String::new())),
            expanded,
            render_scheduled: Rc::new(Cell::new(false)),
            chat_mode: Cell::new(false),
        };
        let row_for_click = row.clone();
        head_button.connect_clicked(move |_| {
            let next = !row_for_click.expanded.get();
            row_for_click.set_expanded(next);
        });

        row
    }

    fn set_expanded(&self, expanded: bool) {
        self.expanded.set(expanded);
        icons::set_icon(
            &self.chevron,
            if expanded { "chevron-down" } else { "chevron-right" },
            Tone::Muted,
        );
        self.body.set_max_content_height(thinking_height(expanded));
        self.body.set_min_content_height(if expanded {
            thinking_height(true)
        } else {
            0
        });
        self.body.set_visible(!self.chat_mode.get() || expanded);
        self.body.queue_resize();
        self.root.queue_resize();
        self.render_now();
    }

    fn show_running(&self) {
        self.title.set_text("Thinking...");
    }

    fn show_done(&self) {
        self.title.set_text("thought");
        self.render_now();
    }

    fn set_chat_mode(&self, chat: bool) {
        self.chat_mode.set(chat);
        if chat {
            self.set_expanded(false);
        } else {
            self.body.set_visible(true);
        }
    }

    fn push(&self, delta: &str) {
        {
            let mut text = self.text.borrow_mut();
            text.push_str(delta);
            if text.len() > THINK_STORE_CHARS * 2 {
                let keep_from = text.len() - THINK_STORE_CHARS;
                text.drain(..keep_from);
            }
        }
        self.schedule_render();
    }

    fn schedule_render(&self) {
        if self.render_scheduled.replace(true) {
            return;
        }
        let row = self.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(THINK_RENDER_MS), move || {
            row.render_scheduled.set(false);
            row.render_now();
        });
    }

    fn render_now(&self) {
        let text = self.text.borrow();
        let display = if self.expanded.get() {
            think_clip(&text, THINK_FULL_CHARS)
        } else {
            think_tail(&text, THINK_TAIL_CHARS)
        };
        self.content.set_text(&display);
        if !self.expanded.get() {
            scroll_thinking_to_end(&self.body);
        }
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
        self.render_now();
    }
}

fn set_thinking_level(label: &gtk::Label, level: &str) {
    for class in ["off", "minimal", "low", "medium", "high", "xhigh", "max"] {
        label.remove_css_class(class);
    }
    let class = match level {
        "off" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" => level,
        _ => "medium",
    };
    label.add_css_class(class);
}

fn scroll_thinking_to_end(body: &gtk::ScrolledWindow) {
    let adjustment = body.vadjustment();
    let bottom = (adjustment.upper() - adjustment.page_size()).max(0.0);
    adjustment.set_value(bottom);
}




