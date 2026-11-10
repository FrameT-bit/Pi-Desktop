
use gtk::prelude::*;

const WIDTH_FRACTION: f64 = 0.96;
const HEIGHT_FRACTION: f64 = 0.92;
const MIN_WIDTH: i32 = 360;
const MIN_HEIGHT: i32 = 320;

pub fn clamp(display: &gtk::gdk::Display, width: i32, height: i32) -> (i32, i32) {
    match usable_area(display) {
        Some(area) => fit_to(area, width, height),
        None => (width, height),
    }
}

pub fn fit_to(area: (i32, i32), width: i32, height: i32) -> (i32, i32) {
    let max_width = ((area.0 as f64) * WIDTH_FRACTION).round() as i32;
    let max_height = ((area.1 as f64) * HEIGHT_FRACTION).round() as i32;
    (
        width.min(max_width).max(MIN_WIDTH.min(max_width)),
        height.min(max_height).max(MIN_HEIGHT.min(max_height)),
    )
}

fn usable_area(display: &gtk::gdk::Display) -> Option<(i32, i32)> {
    let monitors = display.monitors();
    let monitor = monitors
        .item(0)
        .and_then(|item| item.downcast::<gtk::gdk::Monitor>().ok())?;
    let geometry = monitor.geometry();
    Some((geometry.width(), geometry.height().saturating_sub(48)))
}

pub fn fit(window: &gtk::Window, width: i32, height: i32) {
    let display = gtk::gdk::Display::default();
    let (width, height) = match display.as_ref() {
        Some(display) => clamp(display, width, height),
        None => (width, height),
    };
    window.set_default_size(width, height);
    window.set_size_request(width, height);
}


