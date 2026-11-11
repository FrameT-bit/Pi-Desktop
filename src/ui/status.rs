
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use crate::state::Connection;

pub struct StatusUi {
    pub root: gtk::Box,
    pub target: gtk::Label,
    pub tokens: gtk::Label,
    pub cost: gtk::Label,
    pub cost_sep: gtk::Label,
    pub ctx: gtk::Label,
    pub rate: gtk::Label,
    pub time: gtk::Label,
    pub ext_sep: gtk::Label,
    pub ext: gtk::Label,
    pub plan_button: gtk::Button,
    pub notice: gtk::Label,
    pub notice_action: gtk::Button,
    notice_callback: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    pub notice_seq: Rc<Cell<u64>>,
    pub perm_button: gtk::Button,
    pub perm: gtk::Label,
    pub conn_dot: gtk::Box,
    pub conn_text: gtk::Label,
    pub conn_button: gtk::Button,
    code_widgets: Vec<gtk::Widget>,
    chat_mode: Cell<bool>,
    show_costs: Cell<bool>,
}

pub fn build() -> StatusUi {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    root.add_css_class("statusbar");
    root.set_height_request(32);
    root.set_valign(gtk::Align::End);

    let target = value_label("LOCAL · ~/projects");
    target.add_css_class("status-target");
    target.set_tooltip_text(Some("LOCAL · ~/projects"));

    let tokens = value_label("0 / 0");
    let cost = value_label("$0");
    let cost_sep = separator();
    let ctx = value_label("—");
    ctx.add_css_class("dim");
    let rate = value_label("0 tok/s");
    let time = value_label("00:00:00");

    let ext_sep = separator();
    let ext = gtk::Label::new(None);
    ext.add_css_class("status-ext");
    let plan_button = gtk::Button::with_label("Exit plan");
    plan_button.add_css_class("status-action");
    plan_button.set_tooltip_text(Some("turn off plan mode (read-only)"));
    plan_button.set_visible(false);
    ext.set_ellipsize(gtk::pango::EllipsizeMode::End);
    ext.set_max_width_chars(48);
    ext_sep.set_visible(false);
    ext.set_visible(false);

    let notice = gtk::Label::new(None);
    notice.add_css_class("status-notice");
    notice.set_ellipsize(gtk::pango::EllipsizeMode::End);
    notice.set_max_width_chars(60);
    notice.set_visible(false);
    let notice_seq = Rc::new(Cell::new(0u64));

    let notice_action = gtk::Button::with_label("open");
    notice_action.add_css_class("status-action");
    notice_action.set_visible(false);
    let notice_callback: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    {
        let callback = notice_callback.clone();
        let action = notice_action.clone();
        let label = notice.clone();
        notice_action.connect_clicked(move |_| {
            if let Some(callback) = callback.borrow_mut().take() {
                callback();
            }
            action.set_visible(false);
            label.set_visible(false);
        });
    }

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    let perm = value_label("Smart");
    let perm_button = gtk::Button::new();
    perm_button.add_css_class("status-button");
    perm_button.set_tooltip_text(Some("change permission mode"));
    let perm_body = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    perm_body.append(&key_label("Permissions:"));
    perm_body.append(&perm);
    perm_button.set_child(Some(&perm_body));

    let conn_dot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    conn_dot.add_css_class("conn-dot");
    conn_dot.add_css_class("wait");
    conn_dot.set_valign(gtk::Align::Center);

    let conn_text = value_label("connecting");
    let conn_button = gtk::Button::with_label("reconnect");
    conn_button.add_css_class("status-action");
    conn_button.set_tooltip_text(Some("reopens the RPC session with the current configuration"));
    conn_button.set_visible(false);

    let conn = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    conn.set_valign(gtk::Align::Center);
    conn.append(&conn_dot);
    conn.append(&conn_text);
    conn.append(&conn_button);

    let sep_target = separator();
    let sep_cost = separator();
    let ctx_label = key_label("ctx");
    let sep_perm = separator();
    let sep_conn = separator();

    root.append(&target);
    root.append(&sep_target);
    root.append(&cost_sep);
    root.append(&cost);
    root.append(&sep_cost);
    root.append(&ctx_label);
    root.append(&ctx);

    root.append(&spacer);
    root.append(&ext_sep);
    root.append(&ext);
    root.append(&plan_button);
    root.append(&notice);
    root.append(&notice_action);
    root.append(&sep_perm);
    root.append(&perm_button);
    root.append(&sep_conn);
    root.append(&conn);

    let code_widgets: Vec<gtk::Widget> = vec![
        target.clone().upcast(),
        sep_target.upcast(),
        cost_sep.clone().upcast(),
        cost.clone().upcast(),
        sep_cost.upcast(),
        ctx_label.upcast(),
        ctx.clone().upcast(),
        sep_perm.upcast(),
        perm_button.clone().upcast(),
        sep_conn.upcast(),
    ];

    StatusUi {
        root,
        target,
        tokens,
        cost,
        cost_sep,
        ctx,
        rate,
        time,
        ext_sep,
        ext,
        plan_button,
        notice,
        notice_action,
        notice_callback,
        notice_seq,
        perm_button,
        perm,
        conn_dot,
        conn_text,
        conn_button,
        code_widgets,
        chat_mode: Cell::new(false),
        show_costs: Cell::new(true),
    }
}

impl StatusUi {
    pub fn set_show_costs(&self, enabled: bool) {
        self.show_costs.set(enabled);
        if !self.chat_mode.get() {
            self.cost.set_visible(enabled);
            self.cost_sep.set_visible(enabled);
        }
    }

    pub fn set_chat_mode(&self, chat: bool) {
        self.chat_mode.set(chat);
        for widget in &self.code_widgets {
            widget.set_visible(!chat);
        }
        if !chat {
            self.cost.set_visible(self.show_costs.get());
            self.cost_sep.set_visible(self.show_costs.get());
        }
        let show_ext = !chat && !self.ext.text().is_empty();
        self.ext.set_visible(show_ext);
        self.ext_sep.set_visible(show_ext);
        self.root.set_height_request(if chat { 26 } else { 32 });
    }

    pub fn set_target(&self, text: &str) {
        self.target.set_text(text);
        self.target.set_tooltip_text(Some(text));
    }

    pub fn flash_notice(&self, text: &str) {
        let seq = self.notice_seq.get() + 1;
        self.notice_seq.set(seq);
        self.notice_action.set_visible(false);
        self.notice_callback.borrow_mut().take();
        self.notice.set_text(text);
        self.notice.set_tooltip_text(Some(text));
        self.notice.set_visible(true);

        let label = self.notice.clone();
        let seq_c = self.notice_seq.clone();
        glib::timeout_add_seconds_local(4, move || {
            if seq_c.get() == seq {
                label.set_text("");
                label.set_visible(false);
            }
            glib::ControlFlow::Break
        });
    }

    pub fn flash_action(&self, text: &str, action_label: &str, callback: impl Fn() + 'static) {
        let seq = self.notice_seq.get() + 1;
        self.notice_seq.set(seq);
        self.notice.set_text(text);
        self.notice.set_tooltip_text(Some(text));
        self.notice.set_visible(true);
        self.notice_action.set_label(action_label);
        self.notice_action.set_visible(true);
        *self.notice_callback.borrow_mut() = Some(Box::new(callback));

        let label = self.notice.clone();
        let action = self.notice_action.clone();
        let callback = self.notice_callback.clone();
        let seq_c = self.notice_seq.clone();
        glib::timeout_add_seconds_local(6, move || {
            if seq_c.get() == seq {
                label.set_text("");
                label.set_visible(false);
                action.set_visible(false);
                callback.borrow_mut().take();
            }
            glib::ControlFlow::Break
        });
    }

    pub fn set_connection(&self, connection: &Connection, detail: &str) {
        for class in ["ok", "bad", "wait"] {
            self.conn_dot.remove_css_class(class);
        }
        self.conn_dot.add_css_class(match connection {
            Connection::Connected => "ok",
            Connection::Connecting => "wait",
            Connection::Disconnected(_) => "bad",
        });
        let text = match connection {
            Connection::Disconnected(detail) if !detail.trim().is_empty() => {
                truncate(detail.trim(), 48)
            }
            _ => connection.label().to_string(),
        };
        self.conn_text.set_text(&text);
        self.conn_text.set_tooltip_text(Some(detail.trim()));
        self.conn_button
            .set_visible(matches!(connection, Connection::Disconnected(_)));
    }

    pub fn set_context(&self, percent: Option<f64>) {
        self.ctx.remove_css_class("dim");
        self.ctx.remove_css_class("warn");
        match percent {
            Some(percent) => {
                self.ctx.set_text(&format!("{percent:.0}%"));
                if percent > 80.0 {
                    self.ctx.add_css_class("warn");
                }
            }
            None => {
                self.ctx.set_text("—");
                self.ctx.add_css_class("dim");
            }
        }
    }

    pub fn set_extension_status(&self, text: Option<&str>) {
        self.plan_button
            .set_visible(text.is_some_and(|value| value.contains("[plan]")));
        if self.chat_mode.get() {
            self.ext.set_visible(false);
            self.ext_sep.set_visible(false);
            return;
        }
        match text.filter(|text| !text.trim().is_empty()) {
            Some(text) => {
                self.ext.set_text(text);
                self.ext.set_tooltip_text(Some(text));
                self.ext.set_visible(true);
                self.ext_sep.set_visible(true);
            }
            None => {
                self.ext.set_text("");
                self.ext.set_visible(false);
                self.ext_sep.set_visible(false);
            }
        }
    }
}

fn key_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("status-key");
    label
}

fn value_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("status-value");
    label
}

fn separator() -> gtk::Label {
    let label = gtk::Label::new(Some("·"));
    label.add_css_class("status-sep");
    label
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
