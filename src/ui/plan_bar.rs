use std::cell::RefCell;
use std::rc::Rc;
use gtk::prelude::*;
use crate::extension_ui::{Method, Request, Response};

struct Pending {
    key: String,
    session: String,
    request: Request,
    reply: Rc<dyn Fn(Response)>,
}

pub struct PlanBar {
    pub root: gtk::Box,
    pending: RefCell<Vec<Pending>>,
}

impl PlanBar {
    pub fn build() -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        root.add_css_class("plan-approval");
        root.set_visible(false);
        root.set_valign(gtk::Align::Fill);
        Rc::new(Self { root, pending: RefCell::new(Vec::new()) })
    }

    pub fn push(self: &Rc<Self>, key: String, session: String, request: Request, reply: impl Fn(Response) + 'static) {
        if self.pending.borrow().iter().any(|p| p.key == key && p.request.id == request.id) { return; }
        self.pending.borrow_mut().push(Pending { key, session, request, reply: Rc::new(reply) });
        self.render();
    }

    fn respond(self: &Rc<Self>, key: &str, id: &str, response: Response) {
        let index = self.pending.borrow().iter().position(|p| p.key == key && p.request.id == id);
        let Some(index) = index else { return };
        let pending = self.pending.borrow_mut().remove(index);
        (pending.reply)(response);
        self.render();
    }

    fn render(self: &Rc<Self>) {
        while let Some(child) = self.root.first_child() { self.root.remove(&child); }
        let pending = self.pending.borrow();
        self.root.set_visible(!pending.is_empty());
        if pending.is_empty() { return; }
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true).valign(gtk::Align::Center).build();
        scroll.add_css_class("plan-scroll");
        if pending.len() == 1 {
            self.root.append(&plan_tag());
            scroll.set_child(Some(&self.actions(&pending[0], true)));
            self.root.append(&scroll);
            return;
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        for item in pending.iter() {
            let title = gtk::MenuButton::new();
            title.add_css_class("plan-approval-title");
            title.set_tooltip_text(Some(&format!("{} — Plan", item.session)));
            title.set_halign(gtk::Align::Start);
            title.set_valign(gtk::Align::Center);
            title.set_hexpand(false);
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let session = gtk::Label::new(Some(&item.session));
            session.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            session.set_max_width_chars(18);
            content.append(&session);
            content.append(&plan_tag());
            let arrow = crate::icons::icon("chevron-down", crate::icons::Tone::Muted);
            arrow.set_pixel_size(crate::icons::DETAIL_SIZE);
            content.append(&arrow);
            title.set_child(Some(&content));
            let popover = gtk::Popover::new();
            popover.set_position(gtk::PositionType::Top);
            popover.set_autohide(true);
            popover.add_css_class("plan-options");
            title.set_popover(Some(&popover));
            row.append(&title);
            popover.set_child(Some(&self.actions(item, false)));
        }
        scroll.set_child(Some(&row));
        self.root.append(&scroll);
    }

    fn actions(self: &Rc<Self>, item: &Pending, inline: bool) -> gtk::Box {
        let options: Vec<(String, Response)> = if item.request.method == Method::Confirm {
            vec![("Execute".into(), Response::Confirmed(true)), ("Stay in plan mode".into(), Response::Confirmed(false))]
        } else { item.request.options.iter().map(|option| (option.clone(), Response::Value(option.clone()))).collect() };
        let actions = gtk::Box::new(if inline { gtk::Orientation::Horizontal } else { gtk::Orientation::Vertical }, 8);
        for (label, response) in options {
            let action = gtk::Button::with_label(if inline { inline_label(&label) } else { &label });
            action.add_css_class("plan-choice");
            action.set_tooltip_text(Some(&label));
            actions.append(&action);
            let key = item.key.clone();
            let id = item.request.id.clone();
            let weak = Rc::downgrade(self);
            action.connect_clicked(move |_| {
                if let Some(me) = weak.upgrade() { me.respond(&key, &id, response.clone()); }
            });
        }
        actions
    }

    pub fn is_pending(&self) -> bool { !self.pending.borrow().is_empty() }
}

fn plan_tag() -> gtk::Label {
    let tag = gtk::Label::new(Some("Plan"));
    tag.add_css_class("plan-tag");
    tag.set_valign(gtk::Align::Center);
    tag
}

fn inline_label(option: &str) -> &str {
    match option {
        "Execute the plan (track progress)" => "Execute",
        "Stay in plan mode" => "Stay in plan",
        "Refine the plan" => "Refine",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_labels_are_display_aliases_not_protocol_changes() {
        assert_eq!(inline_label("Execute the plan (track progress)"), "Execute");
        assert_eq!(inline_label("Stay in plan mode"), "Stay in plan");
        assert_eq!(inline_label("Refine the plan"), "Refine");
        assert_eq!(inline_label("Custom extension option"), "Custom extension option");
    }
}
