
use std::cell::RefCell;

use gtk::prelude::*;

use crate::theme::Theme;
use crate::tokens;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub const ICON_SIZE: i32 = 24;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Ink,
    Accent,
    Muted,
    Inverse,
}

impl Tone {
    fn file_tone(self) -> &'static str {
        match self {
            Tone::Ink | Tone::Inverse => "ink",
            Tone::Accent => "accent",
            Tone::Muted => "muted",
        }
    }

    fn dark(self) -> bool {
        match self {
            Tone::Inverse => true,
            Tone::Ink | Tone::Accent | Tone::Muted => tokens::active_section() == "dark",
        }
    }
}

struct Slot {
    image: glib::WeakRef<gtk::Image>,
    name: String,
    tone: Tone,
}

thread_local! {
    static SLOTS: RefCell<Vec<Slot>> = const { RefCell::new(Vec::new()) };
}

pub fn icon(name: &str, tone: Tone) -> gtk::Image {
    let image = gtk::Image::new();
    image.add_css_class("ui-icon");
    image.set_pixel_size(ICON_SIZE);
    register(&image, name, tone);

    let name = name.to_string();
    image.connect_map(move |image| {
        apply(image, &name, tone);
    });
    image
}

pub fn set_icon(image: &gtk::Image, name: &str, tone: Tone) {
    let updated = SLOTS.with(|slots| {
        let mut slots = slots.borrow_mut();
        for slot in slots.iter_mut() {
            if slot.image.upgrade().as_ref() == Some(image) {
                slot.name = name.to_string();
                slot.tone = tone;
                return true;
            }
        }
        false
    });
    if !updated {
        register(image, name, tone);
    }
    apply(image, name, tone);
}

fn register(image: &gtk::Image, name: &str, tone: Tone) {
    SLOTS.with(|slots| {
        slots.borrow_mut().push(Slot {
            image: image.downgrade(),
            name: name.to_string(),
            tone,
        });
    });
    apply(image, name, tone);
}

pub fn refresh_all() {
    SLOTS.with(|slots| {
        let mut slots = slots.borrow_mut();
        slots.retain(|slot| slot.image.upgrade().is_some());
        for slot in slots.iter() {
            if let Some(image) = slot.image.upgrade() {
                apply(&image, &slot.name, slot.tone);
            }
        }
    });
}

pub fn attach_theme(theme: &Theme) {
    theme.connect_changed(refresh_all);
}

fn apply(image: &gtk::Image, name: &str, tone: Tone) {
    let scale = if image.scale_factor() >= 2 { 2 } else { 1 };
    let resolved = resolve_name(name);
    let Some(bytes) = icon_bytes(&resolved, tone.file_tone(), tone.dark(), scale) else {
        eprintln!("pi-desktop: message icon in package: {resolved}");
        return;
    };
    let bytes = glib::Bytes::from_static(bytes);
    if let Ok(texture) = gtk::gdk::Texture::from_bytes(&bytes) {
        image.set_paintable(Some(&texture));
    }
}

fn resolve_name(name: &str) -> String {
    match name {
        "caret" | "down" => "chevron-down".to_string(),
        "right" | "expand" => "chevron-right".to_string(),
        "add" => "plus".to_string(),
        "close" => "cross".to_string(),
        "delete" | "remove" => "trash".to_string(),
        "edit" => "pencil".to_string(),
        "gear" | "preferences" => "settings".to_string(),
        "loading" | "reload" => "refresh".to_string(),
        other => other.to_string(),
    }
}


