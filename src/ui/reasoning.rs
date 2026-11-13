use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

pub fn level_label(level: &str) -> String {
    match level {
        "off" => "Off".into(),
        "minimal" => "Minimal".into(),
        "low" => "Low".into(),
        "medium" => "Medium".into(),
        "high" => "High".into(),
        "xhigh" => "Extra high".into(),
        "max" => "Max".into(),
        _ => level.to_string(),
    }
}

fn description(level: &str) -> &'static str {
    match level {
        "off" => "Reasoning is off for the next response.",
        "minimal" | "low" => "Less reasoning for quicker responses.",
        "medium" => "A balance between response time and reasoning effort.",
        _ => "More reasoning effort. Responses may take longer.",
    }
}

pub struct LevelPicker {
    pub root: gtk::Box,
    buttons: Vec<(String, gtk::ToggleButton)>,
    detail: gtk::Label,
    syncing: Cell<bool>,
    on_change: RefCell<Option<Box<dyn Fn(String)>>>,
}

impl LevelPicker {
    pub fn new(levels: &[String]) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let grid = gtk::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_column_spacing(4);
        grid.set_row_spacing(4);
        let mut buttons: Vec<(String, gtk::ToggleButton)> = Vec::new();
        for (index, level) in levels.iter().enumerate() {
            let button = gtk::ToggleButton::with_label(&level_label(level));
            button.add_css_class("reasoning-option");
            button.set_hexpand(true);
            button.set_tooltip_text(Some(description(level)));
            if let Some((_, first)) = buttons.first() { button.set_group(Some(first)); }
            grid.attach(&button, index as i32 % 3, index as i32 / 3, 1, 1);
            buttons.push((level.clone(), button));
        }
        root.append(&grid);
        let detail = gtk::Label::new(None);
        detail.add_css_class("reasoning-description");
        detail.set_xalign(0.0);
        detail.set_wrap(true);
        detail.set_max_width_chars(40);
        root.append(&detail);
        let picker = Rc::new(Self { root, buttons, detail, syncing: Cell::new(false), on_change: RefCell::new(None) });
        for (level, button) in &picker.buttons {
            let weak = Rc::downgrade(&picker);
            let level = level.clone();
            button.connect_toggled(move |button| {
                let Some(picker) = weak.upgrade() else { return };
                if !button.is_active() { return; }
                picker.detail.set_text(description(&level));
                if !picker.syncing.get() {
                    if let Some(callback) = picker.on_change.borrow().as_ref() { callback(level.clone()); }
                }
            });
        }
        picker
    }

    pub fn set_on_change(&self, callback: impl Fn(String) + 'static) {
        *self.on_change.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_level(&self, level: &str) {
        self.syncing.set(true);
        for (value, button) in &self.buttons { button.set_active(value == level); }
        self.detail.set_text(description(level));
        self.syncing.set(false);
    }
}
