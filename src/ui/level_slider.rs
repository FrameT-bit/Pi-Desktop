
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

const HEIGHT: i32 = 46;

pub fn level_label(level: &str) -> String {
    match level.trim().to_ascii_lowercase().as_str() {
        "off" => "OFF".to_string(),
        "minimal" => "MIN".to_string(),
        "low" => "LOW".to_string(),
        "medium" => "MED".to_string(),
        "high" => "HIGH".to_string(),
        "xhigh" => "XHIGH".to_string(),
        "max" => "MAX".to_string(),
        _ => level.to_uppercase(),
    }
}

pub fn index_for_x(x: f64, width: f64, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    if count == 1 {
        return 0;
    }
    let usable = (width - SIDE * 2.0).max(1.0);
    let ratio = ((x - SIDE) / usable).clamp(0.0, 1.0);
    (ratio * (count - 1) as f64).round() as usize
}
const TRACK_Y: f64 = 16.0;
const KNOB_RADIUS: f64 = 7.0;
const SIDE: f64 = 12.0;

pub struct LevelSlider {
    pub area: gtk::DrawingArea,
    levels: Rc<RefCell<Vec<String>>>,
    index: Rc<Cell<usize>>,
    turbo: Rc<Cell<bool>>,
    pressed: Rc<Cell<bool>>,
    drag_x: Rc<Cell<Option<f64>>>,
    on_change: Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
}

impl LevelSlider {
    pub fn new(levels: Vec<String>) -> Rc<Self> {
        let area = gtk::DrawingArea::builder()
            .content_height(HEIGHT)
            .hexpand(true)
            .build();
        area.add_css_class("think-slider");

        let this = Rc::new(Self {
            area,
            levels: Rc::new(RefCell::new(levels)),
            index: Rc::new(Cell::new(0)),
            turbo: Rc::new(Cell::new(false)),
            pressed: Rc::new(Cell::new(false)),
            drag_x: Rc::new(Cell::new(None)),
            on_change: Rc::new(RefCell::new(None)),
        });

        let weak = Rc::downgrade(&this);
        this.area.set_draw_func(move |_, cr, width, height| {
            if let Some(slider) = weak.upgrade() {
                slider.draw(cr, width, height);
            }
        });

        let click = gtk::GestureClick::new();
        let weak = Rc::downgrade(&this);
        click.connect_pressed(move |_, _, x, _| {
            if let Some(slider) = weak.upgrade() {
                slider.pressed.set(true);
                slider.drag_x.set(Some(x));
                let index = index_for_x(x, slider.area.width().max(1) as f64, slider.levels.borrow().len());
                slider.choose(index, true);
                slider.area.queue_draw();
            }
        });
        let weak = Rc::downgrade(&this);
        click.connect_released(move |_, _, x, _| {
            if let Some(slider) = weak.upgrade() {
                slider.pressed.set(false);
                slider.drag_x.set(None);
                let index = index_for_x(x, slider.area.width().max(1) as f64, slider.levels.borrow().len());
                slider.choose(index, true);
                slider.area.queue_draw();
            }
        });
        let weak = Rc::downgrade(&this);
        click.connect_stopped(move |_| {
            if let Some(slider) = weak.upgrade() {
                slider.pressed.set(false);
                slider.drag_x.set(None);
                slider.area.queue_draw();
            }
        });
        this.area.add_controller(click);


        this
    }

    pub fn track_on(self: &Rc<Self>, parent: &impl IsA<gtk::Widget>) {
        let parent_widget = parent.as_ref().clone();
        let start = Rc::new(Cell::new(0.0));
        let drag = gtk::GestureDrag::new();
        let start_begin = start.clone();
        drag.connect_drag_begin(move |_, x, _| {
            if std::env::var("PI_DESKTOP_DEBUG_SLIDER").is_ok() {
                eprintln!("drag beginn x={x}");
            }
            start_begin.set(x);
        });

        let weak = Rc::downgrade(self);
        let parent_update = parent_widget.clone();
        let start_update = start.clone();
        drag.connect_drag_update(move |_, offset_x, _| {
            let Some(slider) = weak.upgrade() else {
                return;
            };
            let Some(bounds) = slider.area.compute_bounds(&parent_update) else {
                return;
            };
            let local = start_update.get() + offset_x - bounds.x() as f64;
            if std::env::var("PI_DESKTOP_DEBUG_SLIDER").is_ok() {
                eprintln!("drag update offset={offset_x} local={local}");
            }
            slider.pressed.set(true);
            slider.drag_x.set(Some(local));
            let index = index_for_x(
                local,
                slider.area.width().max(1) as f64,
                slider.levels.borrow().len(),
            );
            slider.choose(index, true);
            slider.area.queue_draw();
        });

        let weak = Rc::downgrade(self);
        let parent_end = parent_widget.clone();
        let start_end = start.clone();
        drag.connect_drag_end(move |_, offset_x, _| {
            let Some(slider) = weak.upgrade() else {
                return;
            };
            let Some(bounds) = slider.area.compute_bounds(&parent_end) else {
                return;
            };
            let local = start_end.get() + offset_x - bounds.x() as f64;
            slider.pressed.set(false);
            slider.drag_x.set(None);
            let index = index_for_x(
                local,
                slider.area.width().max(1) as f64,
                slider.levels.borrow().len(),
            );
            slider.choose(index, true);
            slider.area.queue_draw();
        });

        parent.add_controller(drag);
    }

    pub fn set_on_change(&self, callback: impl Fn(usize) + 'static) {
        *self.on_change.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_index(self: &Rc<Self>, index: usize) {
        let levels = self.levels.borrow().len();
        if levels == 0 {
            return;
        }
        self.index.set(index.min(levels - 1));
        self.area.queue_draw();
    }

    pub fn set_turbo(&self, turbo: bool) {
        if self.turbo.replace(turbo) != turbo {
            self.area.queue_draw();
        }
    }

    fn choose(self: &Rc<Self>, index: usize, notify: bool) {
        let levels = self.levels.borrow().len();
        if levels == 0 {
            return;
        }
        let index = index.min(levels - 1);
        if self.index.replace(index) == index {
            return;
        }
        if notify {
            if let Some(callback) = self.on_change.borrow().as_ref() {
                callback(index);
            }
        }
    }

    fn draw(self: &Rc<Self>, cr: &gtk::cairo::Context, width: i32, height: i32) {
        let width = width.max(1) as f64;
        let levels = self.levels.borrow();
        let count = levels.len();
        if count == 0 {
            return;
        }
        let track_end = width - SIDE;
        let accent = crate::tokens::active("accent");
        let line = crate::tokens::active("borderMute");
        let text = crate::tokens::active("muted");
        let (red, green, blue) = crate::tokens::rgb(&accent);
        let (line_r, line_g, line_b) = crate::tokens::rgb(&line);
        let (text_r, text_g, text_b) = crate::tokens::rgb(&text);

        cr.set_source_rgb(line_r, line_g, line_b);
        cr.set_line_width(2.0);
        cr.move_to(SIDE, TRACK_Y);
        cr.line_to(track_end, TRACK_Y);
        let _ = cr.stroke();

        let step = if count > 1 {
            (track_end - SIDE) / (count - 1) as f64
        } else {
            0.0
        };

        cr.select_font_face(
            crate::theme::mono_font(),
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Normal,
        );
        cr.set_font_size(10.0);
        for (index, level) in levels.iter().enumerate() {
            let x = SIDE + step * index as f64;
            cr.set_source_rgb(line_r, line_g, line_b);
            cr.set_line_width(1.0);
            cr.move_to(x, 22.0);
            cr.line_to(x, 26.0);
            let _ = cr.stroke();

            let label = level_label(level);
            if let Ok(extents) = cr.text_extents(&label) {
                cr.set_source_rgb(text_r, text_g, text_b);
                cr.move_to(x - extents.width() / 2.0, 40.0);
                let _ = cr.show_text(&label);
            }
        }

        let active = self.index.get().min(count - 1);
        let snapped_x = SIDE + step * active as f64;
        let knob_x = self
            .drag_x
            .get()
            .map(|x| x.clamp(SIDE, track_end))
            .unwrap_or(snapped_x);
        cr.set_source_rgba(red, green, blue, 0.85);
        cr.set_line_width(2.0);
        cr.move_to(SIDE, TRACK_Y);
        cr.line_to(knob_x, TRACK_Y);
        let _ = cr.stroke();

        if self.turbo.get() {
            cr.set_source_rgba(red, green, blue, 0.22);
            cr.arc(knob_x, TRACK_Y, KNOB_RADIUS + 6.0, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        }
        cr.set_source_rgb(red, green, blue);
        cr.arc(knob_x, TRACK_Y, KNOB_RADIUS, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
        cr.set_source_rgb(line_r, line_g, line_b);
        cr.set_line_width(1.0);
        cr.arc(knob_x, TRACK_Y, KNOB_RADIUS, 0.0, std::f64::consts::TAU);
        let _ = cr.stroke();

        let _ = height;
    }
}


