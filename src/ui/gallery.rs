use std::{cell::Cell, io::Read, path::PathBuf, rc::Rc};

use gtk::prelude::*;

use crate::images;

const INLINE_HEIGHT: i32 = 240;
const INLINE_DECODE_SIDE: i32 = 720;
const VIEWER_DECODE_SIDE: i32 = 2048;

#[derive(Debug, PartialEq)]
pub(super) enum Source {
    File(PathBuf, String),
    Data(String, String),
}

impl Source {
    fn label(&self) -> &str {
        match self {
            Self::File(_, label) | Self::Data(_, label) => label,
        }
    }

    fn texture(&self, side: i32) -> Option<gtk::gdk::Texture> {
        let bytes = match self {
            Self::File(path, _) => {
                let metadata = path.metadata().ok()?;
                if !metadata.is_file() || metadata.len() > images::MAX_BYTES as u64 {
                    return None;
                }
                let mut bytes = Vec::new();
                std::fs::File::open(path).ok()?.take(images::MAX_BYTES as u64 + 1)
                    .read_to_end(&mut bytes).ok()?;
                bytes
            }
            Self::Data(data, _) => {
                if data.len() > images::MAX_BYTES.div_ceil(3) * 4 { return None; }
                images::decode_base64(data)?
            }
        };
        if images::too_large(bytes.len()) { return None; }
        images::sniff_mime(&bytes)?;
        let bytes = glib::Bytes::from_owned(bytes);
        let stream = gtk::gio::MemoryInputStream::from_bytes(&bytes);
        let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_stream_at_scale(
            &stream, side, side, true, gtk::gio::Cancellable::NONE,
        ).ok()?;
        Some(gtk::gdk::Texture::for_pixbuf(&pixbuf))
    }
}

impl From<images::ImageBlock> for Source {
    fn from(image: images::ImageBlock) -> Self {
        Self::Data(image.data, image.mime)
    }
}

pub(super) enum Part<'a> {
    Text(&'a str),
    Images(Vec<Source>),
}

/// Only top-level, closed fences are markers; examples inside code stay prose.
pub(super) fn parts(text: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let mut lines = text.split_inclusive('\n').peekable();
    let mut offset = 0;
    let mut start = 0;
    let mut fence = 0;
    while let Some(line) = lines.next() {
        let line_start = offset;
        offset += line.len();
        let trimmed = line.trim();
        let ticks = trimmed.bytes().take_while(|byte| *byte == b'`').count();
        if fence > 0 {
            if ticks >= fence && trimmed[ticks..].is_empty() { fence = 0; }
            continue;
        }
        if ticks < 3 { continue; }
        if !trimmed[ticks..].trim().eq_ignore_ascii_case("pi-image") {
            fence = ticks;
            continue;
        }
        let payload_start = offset;
        let mut payload_end = None;
        for next in lines.by_ref() {
            let end = offset;
            offset += next.len();
            let marker = next.trim();
            if marker.len() >= ticks && marker.bytes().all(|byte| byte == b'`') {
                payload_end = Some(end);
                break;
            }
        }
        let Some(end) = payload_end else { break };
        let Some(image) = file_source(text[payload_start..end].trim()) else { continue };
        let prose = &text[start..line_start];
        if !prose.trim().is_empty() { parts.push(Part::Text(prose)); }
        if let Some(Part::Images(images)) = parts.last_mut() {
            images.push(image);
        } else {
            parts.push(Part::Images(vec![image]));
        }
        start = offset;
    }
    if !text[start..].trim().is_empty() { parts.push(Part::Text(&text[start..])); }
    parts
}

fn file_source(payload: &str) -> Option<Source> {
    let value = serde_json::from_str::<serde_json::Value>(payload).ok();
    let (path, alt) = if let Some(value) = &value {
        (value.get("path").or_else(|| value.get("file"))?.as_str()?,
            value.get("alt").or_else(|| value.get("caption")).and_then(|s| s.as_str()))
    } else {
        (payload, None)
    };
    if path.contains('\n') || path.contains("://") || path.is_empty() || path.len() > 4096 {
        return None;
    }
    let path = crate::config::expand_home(path);
    // Pi may run over SSH or from a different cwd. Never guess a local path.
    if !path.is_absolute() { return None; }
    let title = alt.filter(|label| !label.is_empty())
        .or_else(|| path.file_name().and_then(|name| name.to_str()))
        .unwrap_or("Image").to_string();
    Some(Source::File(path, title))
}

pub(super) fn widget(images: Vec<Source>) -> gtk::Box {
    let images = Rc::new(images);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("chat-gallery");
    if images.is_empty() { return root; }
    let picture = picture();
    picture.set_size_request(-1, INLINE_HEIGHT);
    let open = gtk::Button::new();
    open.add_css_class("chat-gallery-image");
    open.set_hexpand(true);
    open.set_cursor_from_name(Some("zoom-in"));
    open.set_tooltip_text(Some("Enlarge image"));
    open.update_property(&[gtk::accessible::Property::Label("Enlarge image")]);
    open.set_child(Some(&picture));
    root.append(&open);
    let index = controls(&root, &picture, &images, INLINE_DECODE_SIDE, 0);
    {
        let images = images.clone();
        open.connect_clicked(move |button| {
            let Some(parent) = button.root().and_downcast::<gtk::Window>() else { return };
            show(&parent, images.clone(), index.get());
        });
    }
    root
}

fn picture() -> gtk::Picture {
    gtk::Picture::builder().can_shrink(true).content_fit(gtk::ContentFit::Contain)
        .hexpand(true).build()
}

fn controls(root: &gtk::Box, picture: &gtk::Picture, images: &Rc<Vec<Source>>, side: i32, start: usize) -> Rc<Cell<usize>> {
    let index = Rc::new(Cell::new(0));
    let count = gtk::Label::new(None);
    count.add_css_class("gallery-count");
    let previous = arrow("chevron-left", "Previous image");
    let next = arrow("chevron-right", "Next image");
    let nav = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    nav.set_halign(gtk::Align::Center);
    nav.append(&previous);
    nav.append(&count);
    nav.append(&next);
    root.append(&nav);
    let message = gtk::Label::new(None);
    message.add_css_class("gallery-count");
    message.set_wrap(true);
    root.append(&message);
    nav.set_visible(images.len() > 1);
    let show_page = Rc::new({
        let images = images.clone();
        let picture = picture.clone();
        let index = index.clone();
        move |page: usize| {
            index.set(page);
            let image = &images[page];
            let texture = image.texture(side);
            picture.set_paintable(texture.as_ref());
            picture.set_visible(texture.is_some());
            let alt = format!("Image {}: {}", page + 1, image.label());
            picture.set_alternative_text(Some(&alt));
            picture.set_tooltip_text(Some(&alt));
            count.set_text(&format!("{} / {}", page + 1, images.len()));
            message.set_visible(texture.is_none());
            if texture.is_none() {
                message.set_text(&format!("Image unavailable · {}", image.label()));
            }
        }
    });
    show_page(start);
    if images.len() > 1 {
        let step = Rc::new({
            let index = index.clone();
            let pages = images.len() as isize;
            move |delta: isize| show_page((index.get() as isize + delta).rem_euclid(pages) as usize)
        });
        for (button, delta) in [(previous, -1), (next, 1)] {
            let step = step.clone();
            button.connect_clicked(move |_| step(delta));
        }
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, _| {
            match key {
                gtk::gdk::Key::Left => step(-1),
                gtk::gdk::Key::Right => step(1),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        root.add_controller(keys);
    }
    index
}

fn arrow(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("gallery-arrow");
    button.set_child(Some(&crate::icons::icon(icon, crate::icons::Tone::Ink)));
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

fn show(parent: &gtk::Window, images: Rc<Vec<Source>>, start: usize) {
    let window = gtk::Window::builder().title("Image preview").transient_for(parent)
        .modal(true).destroy_with_parent(true).build();
    super::fit::fit(&window, 1000, 760);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("image-viewer");
    let picture = picture();
    picture.set_vexpand(true);
    root.append(&picture);
    controls(&root, &picture, &images, VIEWER_DECODE_SIDE, start);
    window.set_child(Some(&root));
    let keys = gtk::EventControllerKey::new();
    let weak = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(window) = weak.upgrade() { window.close(); }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_prose_and_groups_only_adjacent_images() {
        let text = "before\n```pi-image\n/tmp/a.png\n```\n\n```pi-image\n/tmp/b.png\n```\nafter\n```pi-image\n/tmp/c.png\n```\nend";
        let parts = parts(text);
        assert!(matches!(parts.as_slice(), [Part::Text("before\n"), Part::Images(first), Part::Text("after\n"), Part::Images(last), Part::Text("end")] if first.len() == 2 && last.len() == 1));
    }

    #[test]
    fn code_examples_and_incomplete_markers_are_not_loaded() {
        for text in ["````markdown\n```pi-image\n/tmp/a.png\n```\n````", "```pi-image\n/tmp/a.png"] {
            assert!(matches!(parts(text).as_slice(), [Part::Text(value)] if *value == text));
        }
    }
}
