
use std::rc::Rc;

use gtk::prelude::*;

pub type Action = Rc<dyn Fn()>;

pub struct Item {
    pub label: String,
    pub action: Option<Action>,
    pub tooltip: Option<String>,
}

impl Item {
    pub fn new(label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            action: Some(Rc::new(action)),
            tooltip: None,
        }
    }

    pub fn disabled(label: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action: None,
            tooltip: Some(tooltip.into()),
        }
    }
}

pub fn show(anchor: &impl IsA<gtk::Widget>, items: Vec<(&str, Action)>) {
    show_at(anchor, None, items);
}

/// Opens a menu anchored to `anchor`, pointing at `(x, y)` in anchor coordinates.
pub fn show_at(
    anchor: &impl IsA<gtk::Widget>,
    point: Option<(f64, f64)>,
    items: Vec<(&str, Action)>,
) {
    let items = items
        .into_iter()
        .map(|(label, action)| Item {
            label: label.to_string(),
            action: Some(action),
            tooltip: None,
        })
        .collect();
    show_items_at(anchor, point, items);
}

pub fn show_items(anchor: &impl IsA<gtk::Widget>, items: Vec<Item>) {
    show_items_at(anchor, None, items);
}

fn show_items_at(
    anchor: &impl IsA<gtk::Widget>,
    point: Option<(f64, f64)>,
    items: Vec<Item>,
) {
    if items.is_empty() {
        return;
    }

    let popover = gtk::Popover::new();
    popover.add_css_class("context-popover");
    popover.set_has_arrow(false);
    popover.set_autohide(true);
    if let Some((x, y)) = point {
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
            x.max(0.0) as i32,
            y.max(0.0) as i32,
            1,
            1,
        )));
    }

    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.add_css_class("context-list");
    for item in items {
        let button = gtk::Button::with_label(&item.label);
        button.add_css_class("context-item");
        button.set_halign(gtk::Align::Fill);
        if let Some(tooltip) = &item.tooltip {
            button.set_tooltip_text(Some(tooltip));
        }
        match item.action {
            Some(action) => {
                let popover_c = popover.clone();
                button.connect_clicked(move |_| {
                    popover_c.popdown();
                    action();
                });
            }
            None => button.set_sensitive(false),
        }
        list.append(&button);
    }
    popover.set_child(Some(&list));
    popover.set_parent(anchor);
    popover.connect_closed(|popover| popover.unparent());
    popover.popup();
}
