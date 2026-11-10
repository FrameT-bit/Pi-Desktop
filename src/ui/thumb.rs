
use std::f64::consts::PI;

use gtk::prelude::*;

use crate::images;

pub fn from_base64(data: &str, max_side: i32, class: &str) -> Option<gtk::Box> {
    let bytes = images::decode_base64(data)?;
    from_bytes(&bytes, max_side, class)
}

pub fn from_bytes(bytes: &[u8], max_side: i32, class: &str) -> Option<gtk::Box> {
    let texture = gtk::gdk::Texture::from_bytes(&glib::Bytes::from(bytes)).ok()?;
    from_texture(&texture, max_side, class)
}

pub fn from_texture(
    texture: &gtk::gdk::Texture,
    max_side: i32,
    class: &str,
) -> Option<gtk::Box> {
    let source_w = texture.width().max(1);
    let source_h = texture.height().max(1);
    let max_side = max_side.max(1);

    let scale = (max_side as f64 / source_w as f64).min(max_side as f64 / source_h as f64);
    let width = ((source_w as f64 * scale).round() as i32).max(1);
    let height = ((source_h as f64 * scale).round() as i32).max(1);

    let mut downloader = gtk::gdk::TextureDownloader::new(texture);
    downloader.set_format(if cfg!(target_endian = "big") {
        gtk::gdk::MemoryFormat::A8r8g8b8Premultiplied
    } else {
        gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied
    });
    let (pixels, stride) = downloader.download_bytes();
    let data = pixels.as_ref().to_vec();
    let surface = gtk::cairo::ImageSurface::create_for_data(
        data,
        gtk::cairo::Format::ARgb32,
        source_w,
        source_h,
        stride as i32,
    )
    .ok()?;

    let area = gtk::DrawingArea::new();
    area.set_content_width(width);
    area.set_content_height(height);
    area.set_halign(gtk::Align::Start);
    area.set_valign(gtk::Align::Start);

    area.set_draw_func(move |_, cr, width, height| {
        rounded_rect(cr, width as f64, height as f64, 6.0);
        cr.clip();
        cr.scale(
            width as f64 / source_w as f64,
            height as f64 / source_h as f64,
        );
        let _ = cr.set_source_surface(&surface, 0.0, 0.0);
        let _ = cr.paint();
    });

    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class(class);
    frame.set_halign(gtk::Align::Start);
    frame.set_valign(gtk::Align::Start);
    frame.set_overflow(gtk::Overflow::Hidden);
    frame.append(&area);
    Some(frame)
}

fn rounded_rect(cr: &gtk::cairo::Context, width: f64, height: f64, radius: f64) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
    if radius <= 0.0 {
        cr.rectangle(0.0, 0.0, width, height);
        return;
    }
    cr.new_sub_path();
    cr.arc(width - radius, radius, radius, -PI / 2.0, 0.0);
    cr.arc(width - radius, height - radius, radius, 0.0, PI / 2.0);
    cr.arc(radius, height - radius, radius, PI / 2.0, PI);
    cr.arc(radius, radius, radius, PI, 3.0 * PI / 2.0);
    cr.close_path();
}
