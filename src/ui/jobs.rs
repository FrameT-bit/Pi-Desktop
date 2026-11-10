
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::loops;

pub type StopFn = Box<dyn Fn()>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopView {
    pub task: String,
    pub iteration: u64,
    pub interval: u64,
    pub remaining: u64,
}

pub struct JobsUi {
    pub root: gtk::Box,
    empty: gtk::Box,
    list: gtk::Box,
    card: gtk::Box,
    task: gtk::Label,
    meta: gtk::Label,
    next: gtk::Label,
    on_stop: Rc<RefCell<Option<StopFn>>>,
}

impl JobsUi {
    pub fn build() -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("jobs-page");
        root.set_hexpand(true);
        root.set_vexpand(true);

        let inner = gtk::Box::new(gtk::Orientation::Vertical, 16);
        inner.add_css_class("jobs-inner");
        inner.set_halign(gtk::Align::Fill);
        inner.set_valign(gtk::Align::Fill);
        inner.set_hexpand(true);
        inner.set_vexpand(true);

        let title = gtk::Label::new(Some("Tasks"));
        title.add_css_class("panel-title");
        title.set_xalign(0.0);

        let subtitle = gtk::Label::new(Some(
            "self-paced tasks · /loop <task> in the composer",
        ));
        subtitle.add_css_class("jobs-subtitle");
        subtitle.set_xalign(0.0);

        let heading = gtk::Box::new(gtk::Orientation::Vertical, 4);
        heading.append(&title);
        heading.append(&subtitle);
        inner.append(&heading);

        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.set_halign(gtk::Align::Center);
        empty.set_valign(gtk::Align::Center);
        empty.set_vexpand(true);
        let empty_label = gtk::Label::new(Some("no active loops"));
        empty_label.add_css_class("empty-page");
        empty.append(&empty_label);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("jobs-list");
        list.set_visible(false);

        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.add_css_class("jobs-card");

        let task = gtk::Label::new(None);
        task.add_css_class("jobs-task");
        task.set_xalign(0.0);
        task.set_wrap(true);
        task.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        task.set_selectable(true);
        task.set_max_width_chars(90);

        let meta = gtk::Label::new(None);
        meta.add_css_class("jobs-meta");
        meta.set_xalign(0.0);

        let next = gtk::Label::new(None);
        next.add_css_class("jobs-next");
        next.set_xalign(0.0);
        next.set_hexpand(true);

        let stop = gtk::Button::with_label("Stop");
        stop.add_css_class("jobs-stop");
        stop.set_valign(gtk::Align::Center);

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.append(&next);
        footer.append(&stop);

        card.append(&task);
        card.append(&meta);
        card.append(&footer);
        list.append(&card);

        inner.append(&empty);
        inner.append(&list);
        root.append(&inner);

        let on_stop: Rc<RefCell<Option<StopFn>>> = Rc::new(RefCell::new(None));
        let on_stop_c = on_stop.clone();
        stop.connect_clicked(move |_| {
            if let Some(callback) = on_stop_c.borrow().as_ref() {
                callback();
            }
        });

        Rc::new(Self {
            root,
            empty,
            list,
            card,
            task,
            meta,
            next,
            on_stop,
        })
    }

    pub fn set_on_stop(&self, callback: impl Fn() + 'static) {
        *self.on_stop.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_loop(&self, view: Option<&LoopView>) {
        let Some(view) = view else {
            self.empty.set_visible(true);
            self.list.set_visible(false);
            return;
        };

        self.task.set_text(&view.task);
        self.task.set_tooltip_text(Some(&view.task));
        self.meta.set_text(&format!(
            "iteration {} · interval {}",
            view.iteration,
            loops::format_interval(view.interval)
        ));
        self.next.set_text(&format!(
            "next in {}",
            loops::format_countdown(view.remaining)
        ));
        self.card.set_visible(true);
        self.list.set_visible(true);
        self.empty.set_visible(false);
    }
}
