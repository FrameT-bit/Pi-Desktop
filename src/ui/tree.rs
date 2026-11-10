
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::icons::{self, Tone};
use crate::tree::{Kind, Node, TreeView};

use super::menu;

pub type ReloadFn = Box<dyn Fn()>;
pub type ForkFn = Box<dyn Fn(String)>;

pub struct TreeUi {
    pub root: gtk::Box,
    status: gtk::Label,
    list: gtk::Box,
    scroll: gtk::ScrolledWindow,
    reload: gtk::Button,
    view: RefCell<Option<TreeView>>,
    on_reload: RefCell<Option<ReloadFn>>,
    on_fork: RefCell<Option<ForkFn>>,
}

impl TreeUi {
    pub fn build() -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("tree-page");
        root.set_hexpand(true);
        root.set_vexpand(true);

        let inner = gtk::Box::new(gtk::Orientation::Vertical, 16);
        inner.add_css_class("tree-inner");
        inner.set_hexpand(true);
        inner.set_vexpand(true);

        let title = gtk::Label::new(Some("Session Tree"));
        title.add_css_class("panel-title");
        title.set_xalign(0.0);

        let status = gtk::Label::new(Some("carrying a tree..."));
        status.add_css_class("tree-status");
        status.set_xalign(0.0);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let reload = gtk::Button::with_label("Reload");
        reload.add_css_class("tree-reload");
        reload.set_valign(gtk::Align::Center);

        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        heading.set_hexpand(true);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
        words.set_hexpand(true);
        words.append(&title);
        words.append(&status);
        heading.append(&words);
        heading.append(&reload);
        inner.append(&heading);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("tree-list");

        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&list)
            .build();
        scroll.add_css_class("tree-scroll");
        inner.append(&scroll);
        root.append(&inner);

        let ui = Rc::new(Self {
            root,
            status,
            list,
            scroll,
            reload,
            view: RefCell::new(None),
            on_reload: RefCell::new(None),
            on_fork: RefCell::new(None),
        });

        {
            let ui = ui.clone();
            let reload = ui.reload.clone();
            reload.connect_clicked(move |_| {
                if let Some(callback) = ui.on_reload.borrow().as_ref() {
                    callback();
                }
            });
        }

        ui
    }

    pub fn set_on_reload(&self, callback: impl Fn() + 'static) {
        *self.on_reload.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_on_fork(&self, callback: impl Fn(String) + 'static) {
        *self.on_fork.borrow_mut() = Some(Box::new(callback));
    }

    pub fn count(&self) -> usize {
        self.view.borrow().as_ref().map(TreeView::count).unwrap_or(0)
    }

    pub fn set_loading(&self) {
        self.reload.set_sensitive(false);
        self.status.remove_css_class("error");
        self.status.set_text("carrying a tree...");
        self.view.replace(None);
        self.clear_list();
    }

    pub fn set_error(&self, message: &str) {
        self.reload.set_sensitive(true);
        self.status.add_css_class("error");
        self.status.set_text(message);
        self.view.replace(None);
        self.clear_list();
    }

    pub fn set_view(self: &Rc<Self>, view: &TreeView) {
        self.reload.set_sensitive(true);
        self.status.remove_css_class("error");
        let leaf = view.leaf_id.as_deref().unwrap_or("—");
        let entries = if view.count() == 1 {
            "1 entry".to_string()
        } else {
            format!("{} entradas", view.count())
        };
        let roots = match view.roots() {
            1 => "1 root".to_string(),
            roots => format!("{roots} roots"),
        };
        self.status
            .set_text(&format!("{entries} · {roots} · folha {leaf}"));
        self.view.replace(Some(view.clone()));
        self.clear_list();
        if view.nodes.is_empty() {
            let empty = gtk::Label::new(Some("no entry in this session"));
            empty.add_css_class("tree-empty");
            empty.set_xalign(0.0);
            self.list.append(&empty);
            return;
        }
        for node in &view.nodes {
            self.list.append(&self.build_row(node));
        }
        glib::idle_add_local_once({
            let adj = self.scroll.vadjustment();
            move || adj.set_value(0.0)
        });
    }

    fn clear_list(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
    }

    fn build_row(self: &Rc<Self>, node: &Node) -> gtk::Box {
        let tone = if node.is_leaf {
            Tone::Accent
        } else if node.is_on_leaf_path {
            Tone::Ink
        } else {
            Tone::Muted
        };

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("tree-row");
        row.set_hexpand(true);
        if node.is_leaf {
            row.add_css_class("active");
        }
        if node.is_on_leaf_path {
            row.add_css_class("on-path");
        } else {
            row.add_css_class("off-path");
        }

        let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bar.add_css_class("tree-bar");
        bar.set_width_request(2);
        bar.set_visible(node.is_leaf);
        row.append(&bar);

        let indent = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        indent.set_width_request((node.depth as i32) * 12);
        row.append(&indent);

        let icon = icons::icon(node.kind.icon(), tone);
        icon.set_valign(gtk::Align::Start);
        icon.set_margin_top(2);
        row.append(&icon);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
        body.set_hexpand(true);
        body.set_valign(gtk::Align::Center);

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        top.set_hexpand(true);
        let label = gtk::Label::new(Some(&node.label));
        label.add_css_class("tree-label");
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_lines(1);
        top.append(&label);

        if node.is_leaf {
            let current = gtk::Label::new(Some("CURRENT"));
            current.add_css_class("tree-current");
            current.set_valign(gtk::Align::Center);
            top.append(&current);
        }
        let time = short_time(&node.timestamp);
        if !time.is_empty() {
            let time = gtk::Label::new(Some(&time));
            time.add_css_class("tree-time");
            time.set_valign(gtk::Align::Center);
            top.append(&time);
        }
        body.append(&top);

        let preview = preview_text(&node.text, &node.label);
        if let Some(preview) = preview {
            let text = gtk::Label::new(Some(&preview));
            text.add_css_class("tree-text");
            text.set_xalign(0.0);
            text.set_wrap(true);
            text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            text.set_lines(2);
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);
            body.append(&text);
        }

        row.append(&body);

        let detail = if node.timestamp.is_empty() {
            node.entry_type.clone()
        } else {
            format!("{} · {}", node.entry_type, node.timestamp)
        };
        row.set_tooltip_text(Some(&detail));

        if node.kind == Kind::User {
            row.set_cursor_from_name(Some("pointer"));
            row.set_tooltip_text(Some(&format!("click to branch here · {detail}")));
            {
                let ui = self.clone();
                let id = node.id.clone();
                let click = gtk::GestureClick::new();
                click.set_button(gtk::gdk::BUTTON_PRIMARY);
                click.connect_pressed(move |_, n_press, _, _| {
                    if n_press != 1 {
                        return;
                    }
                    if let Some(callback) = ui.on_fork.borrow().as_ref() {
                        callback(id.clone());
                    }
                });
                row.add_controller(click);
            }
            {
                let ui = self.clone();
                let id = node.id.clone();
                let anchor = row.clone();
                let menu_gesture = gtk::GestureClick::new();
                menu_gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
                menu_gesture.connect_pressed(move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    let ui_c = ui.clone();
                    let id_c = id.clone();
                    menu::show(
                        &anchor,
                        vec![(
                            "Ramify from here",
                            Rc::new(move || {
                                if let Some(callback) = ui_c.on_fork.borrow().as_ref() {
                                    callback(id_c.clone());
                                }
                            }),
                        )],
                    );
                });
                row.add_controller(menu_gesture);
            }
        }

        row
    }
}

fn preview_text(text: &str, label: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || label.contains(text) {
        return None;
    }
    let head = label.trim().trim_end_matches('…').trim();
    if head.is_empty() {
        return None;
    }

    let common = head
        .chars()
        .zip(text.chars())
        .take_while(|(a, b)| a == b)
        .count();
    if common >= 12 {
        return None;
    }

    let mut out: String = text.chars().take(320).collect();
    if text.chars().count() > 320 {
        out.push('…');
    }
    Some(out)
}

fn short_time(timestamp: &str) -> String {
    timestamp
        .split_once('T')
        .and_then(|(_, time)| time.get(0..5))
        .unwrap_or("")
        .to_string()
}


