
use std::cell::RefCell;
use std::collections::HashMap;

use gtk::prelude::*;

use crate::theme::Palette;

include!(concat!(env!("OUT_DIR"), "/logos.rs"));

pub const LOGO_SIZE: i32 = 24;

pub fn provider_slug(provider: &str) -> Option<&'static str> {
    let provider = provider.trim().to_ascii_lowercase();
    Some(match provider.as_str() {
        "mistral" | "mistralai" | "mistral-ai" => "mistralai",
        "groq" => "groq",
        "deepseek" => "deepseek",
        "nvidia" => "nvidia",
        "openai" => "openai",
        "anthropic" => "anthropic",
        "google" | "googlegemini" | "google-gemini" | "gemini" => "googlegemini",
        "meta" | "llama" => "meta",
        "ollama" => "ollama",
        _ => return None,
    })
}

pub fn fallback_letter(provider: &str) -> String {
    provider
        .trim()
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().to_string())
        .unwrap_or_else(|| "·".to_string())
}

thread_local! {
    static TEXTURES: RefCell<HashMap<(String, bool), gtk::gdk::Texture>> =
        RefCell::new(HashMap::new());
}

pub fn texture(slug: &str, palette: Palette) -> Option<gtk::gdk::Texture> {
    let dark = matches!(palette, Palette::Dark);
    let key = (slug.to_string(), dark);
    if let Some(texture) = TEXTURES.with(|cache| cache.borrow().get(&key).cloned()) {
        return Some(texture);
    }
    let bytes = logo_bytes(slug, dark)?;
    let texture = gtk::gdk::Texture::from_bytes(&glib::Bytes::from_static(bytes)).ok()?;
    TEXTURES.with(|cache| {
        cache.borrow_mut().insert(key, texture.clone());
    });
    Some(texture)
}

pub fn widget(provider: &str, palette: Palette) -> gtk::Widget {
    let Some(slug) = provider_slug(provider) else {
        return fallback(provider).upcast();
    };
    let Some(texture) = texture(slug, palette) else {
        return fallback(provider).upcast();
    };
    let image = gtk::Image::new();
    image.add_css_class("model-logo");
    let side = texture.width();
    image.set_pixel_size(side);
    image.set_size_request(side, side);
    image.set_valign(gtk::Align::Center);
    image.set_paintable(Some(&texture));
    image.set_tooltip_text(Some(provider));
    image.upcast()
}

fn fallback(provider: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(&fallback_letter(provider)));
    label.add_css_class("model-logo-fallback");
    label.set_size_request(LOGO_SIZE, LOGO_SIZE);
    label.set_valign(gtk::Align::Center);
    label.set_tooltip_text(Some(provider));
    label
}


