
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
        "openrouter" => "openrouter",
        "xai" | "x-ai" => "xai",
        "cerebras" => "cerebras",
        "fireworks" | "fireworks-ai" => "fireworks",
        "together" | "togetherai" => "together",
        "baseten" => "baseten",
        "moonshotai" | "moonshotai-cn" | "moonshot" | "kimi" | "kimi-coding" => "moonshotai",
        "zai" | "z-ai" | "zai-coding-cn" => "zai",
        "minimax" | "minimax-cn" => "minimax",
        "qwen"
        | "qwen-token-plan"
        | "qwen-token-plan-cn"
        | "qwen-token-plan-individual" => "qwen",
        "opencode" | "opencode-go" => "opencode",
        "vercel" | "vercel-ai-gateway" => "vercel",
        "huggingface" | "hf" => "huggingface",
        "github-copilot" | "githubcopilot" | "copilot" => "githubcopilot",
        "ant-ling" | "antling" => "antling",
        "xiaomi" | "xiaomi-token-plan-cn" | "xiaomi-token-plan-ams" | "xiaomi-token-plan-sgp" => "xiaomi",
        "radius" => "radius",
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



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_provider_has_a_logo() {
        for provider in crate::providers::KNOWN {
            let slug = provider_slug(provider.id)
                .unwrap_or_else(|| panic!("no slug for provider {}", provider.id));
            assert!(
                logo_bytes(slug, true).is_some(),
                "missing dark logo for {} ({slug})",
                provider.id
            );
            assert!(
                logo_bytes(slug, false).is_some(),
                "missing light logo for {} ({slug})",
                provider.id
            );
        }
    }
}
