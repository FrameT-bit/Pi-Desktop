
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

pub const WIDTH: i32 = 320;
pub const HEIGHT: i32 = 150;

pub struct Particles {
    pub area: gtk::DrawingArea,
    active: Rc<Cell<bool>>,
    source: Rc<RefCell<Option<glib::SourceId>>>,
}

impl Particles {
    pub fn new() -> Rc<Self> {
        let area = gtk::DrawingArea::builder()
            .content_width(WIDTH)
            .content_height(HEIGHT)
            .hexpand(true)
            .vexpand(true)
            .build();
        area.add_css_class("think-particles");
        area.set_can_target(false);

        let this = Rc::new(Self {
            area,
            active: Rc::new(Cell::new(false)),
            source: Rc::new(RefCell::new(None)),
        });

        let weak = Rc::downgrade(&this);
        this.area.set_draw_func(move |_, cr, width, height| {
            let Some(particles) = weak.upgrade() else {
                return;
            };
            particles.draw(cr, width, height);
        });
        this
    }

    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.active.replace(active) == active {
            return;
        }
        if active {
            self.start();
        } else {
            self.stop();
            self.area.queue_draw();
        }
    }

    fn start(self: &Rc<Self>) {
        let mut source = self.source.borrow_mut();
        if source.is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        *source = Some(glib::timeout_add_local(
            std::time::Duration::from_millis(50),
            move || {
                let Some(particles) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if !particles.active.get() || !particles.area.is_mapped() {
                    particles.source.borrow_mut().take();
                    return glib::ControlFlow::Break;
                }
                particles.area.queue_draw();
                glib::ControlFlow::Continue
            },
        ));
    }

    fn stop(&self) {
        if let Some(source) = self.source.borrow_mut().take() {
            source.remove();
        }
    }

    fn draw(self: &Rc<Self>, cr: &gtk::cairo::Context, width: i32, height: i32) {
        if !self.active.get() {
            return;
        }
        let width = width.max(1) as f64;
        let height = height.max(1) as f64;
        let accent = crate::tokens::active("accent");
        let (red, green, blue) = crate::tokens::rgb(&accent);
        let now = glib::monotonic_time() as f64 / 1_000_000.0;

        let drops = 34;
        for index in 0..drops {
            let seed = pseudo(index);
            let speed = 0.045 + seed * 0.075; 
            let phase = (now * speed + seed) % 1.0;
            let x = ((index as f64 * 37.0) % width).round();
            let y = (height - phase * (height + 8.0)).round();
            let size = if seed > 0.68 { 3.0 } else { 2.0 };
            let alpha = (0.10 + (1.0 - phase) * 0.32).min(0.42);
            cr.set_source_rgba(red, green, blue, alpha);
            cr.rectangle(x, y, size, size);
            let _ = cr.fill();
        }
    }
}

fn pseudo(index: usize) -> f64 {
    let value = (index as f64 * 12.9898 + 78.233).sin() * 43758.5453;
    value.fract().abs()
}
