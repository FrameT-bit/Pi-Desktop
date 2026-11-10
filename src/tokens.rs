
#![allow(dead_code)]

use std::cell::{Cell, RefCell};

include!(concat!(env!("OUT_DIR"), "/tokens.rs"));

thread_local! {
    static ACTIVE: Cell<&'static str> = const { Cell::new("dark") };
    static NAMED: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub fn set_active(section: &'static str) {
    ACTIVE.with(|active| active.set(section));
    NAMED.with(|named| named.borrow_mut().clear());
}

pub fn set_active_named(section: &'static str, overrides: &[(String, String)]) {
    ACTIVE.with(|active| active.set(section));
    NAMED.with(|named| *named.borrow_mut() = overrides.to_vec());
}

pub fn active_section() -> &'static str {
    ACTIVE.with(|active| active.get())
}

pub fn active(key: &str) -> String {
    if let Some((_, value)) = NAMED.with(|named| {
        named
            .borrow()
            .iter()
            .find(|(name, _)| name == key)
            .cloned()
    }) {
        return value;
    }
    lookup(active_section(), key)
        .unwrap_or("#000000")
        .to_string()
}

pub fn lookup(section: &str, key: &str) -> Option<&'static str> {
    let table = match section {
        "dark" => DARK,
        "light" => LIGHT,
        _ => return None,
    };
    table
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, hex)| *hex)
}

pub fn rgb(hex: &str) -> (f64, f64, f64) {
    let digits = hex.trim_start_matches('#');
    let channel = |index: usize| {
        digits
            .get(index..index + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .map(|value| value as f64 / 255.0)
            .unwrap_or(0.0)
    };
    (channel(0), channel(2), channel(4))
}


