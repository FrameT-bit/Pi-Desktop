
use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotifyType {
    Info,
    Warning,
    Error,
}

impl NotifyType {
    pub fn parse(value: Option<&str>) -> Self {
        match value.unwrap_or("info") {
            "warning" => NotifyType::Warning,
            "error" => NotifyType::Error,
            _ => NotifyType::Info,
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            NotifyType::Info => "info",
            NotifyType::Warning => "warning",
            NotifyType::Error => "error",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    Above,
    Below,
}

impl Placement {
    fn parse(value: Option<&str>) -> Self {
        match value.unwrap_or("AboveEditor") {
            "belowEditor" => Placement::Below,
            _ => Placement::Above,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    Select,
    Confirm,
    Input,
    Editor,
    Notify,
    SetStatus,
    SetWidget,
    SetTitle,
    SetEditorText,
    Unknown,
}

impl Method {
    fn parse(value: &str) -> Self {
        match value {
            "select" => Method::Select,
            "confirm" => Method::Confirm,
            "input" => Method::Input,
            "editor" => Method::Editor,
            "notify" => Method::Notify,
            "Status set" => Method::SetStatus,
            "setWidget" => Method::SetWidget,
            "setTitle" => Method::SetTitle,
            "set_editor_text" => Method::SetEditorText,
            _ => Method::Unknown,
        }
    }

    pub fn is_dialog(self) -> bool {
        matches!(
            self,
            Method::Select | Method::Confirm | Method::Input | Method::Editor
        )
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub id: String,
    pub method: Method,
    pub title: String,
    pub message: String,
    pub options: Vec<String>,
    pub placeholder: String,
    pub prefill: String,
    pub timeout: Option<Duration>,
    pub notify: NotifyType,
    pub status_key: String,
    pub status_text: Option<String>,
    pub widget_key: String,
    pub widget_lines: Option<Vec<String>>,
    pub placement: Placement,
    pub text: String,
}

impl Request {
    pub fn parse(record: &Value) -> Option<Self> {
        if record.get("type").and_then(Value::as_str) != Some("extension_ui_request") {
            return None;
        }
        let method = Method::parse(record.get("method").and_then(Value::as_str).unwrap_or(""));
        let id = record
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let text = |key: &str| {
            record
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let options = record
            .get("options")
            .and_then(Value::as_array)
            .map(|options| {
                options
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let timeout = record
            .get("timeout")
            .and_then(Value::as_u64)
            .filter(|millis| *millis > 0)
            .map(Duration::from_millis);
        let status_text = record
            .get("statusText")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|text| !text.is_empty());
        let widget_lines = record.get("widgetLines").and_then(Value::as_array).map(|lines| {
            lines
                .iter()
                .map(|line| line.as_str().unwrap_or("").to_string())
                .collect()
        });

        Some(Self {
            id,
            method,
            title: text("title"),
            message: text("message"),
            options,
            placeholder: text("placeholder"),
            prefill: text("prefill"),
            timeout,
            notify: NotifyType::parse(record.get("notificationType").and_then(Value::as_str)),
            status_key: text("statusKey"),
            status_text,
            widget_key: text("widgetKey"),
            widget_lines,
            placement: Placement::parse(record.get("widgetPlace").and_then(Value::as_str)),
            text: text("text"),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    Value(String),
    Confirmed(bool),
    Cancelled,
}

pub fn response_json(id: &str, response: &Response) -> Value {
    match response {
        Response::Value(value) => json!({
            "type": "extension_ui_response",
            "id": id,
            "value": value,
        }),
        Response::Confirmed(confirmed) => json!({
            "type": "extension_ui_response",
            "id": id,
            "confirmed": confirmed,
        }),
        Response::Cancelled => json!({
            "type": "extension_ui_response",
            "id": id,
            "cancelled": true,
        }),
    }
}

pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for next in chars.by_ref() {
                    if next.is_ascii() && (0x40..=0x7e).contains(&(next as u8)) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            Some(ch) if ('\u{20}'..='\u{2f}').contains(&ch) => {
                chars.next();
            }
            Some(_) => {}
            None => {}
        }
    }
    out
}

#[derive(Default)]
pub struct Statuses {
    map: BTreeMap<String, String>,
}

impl Statuses {
    pub fn set(&mut self, key: &str, text: Option<&str>) {
        if key.is_empty() {
            return;
        }
        match text.map(str::trim).filter(|text| !text.is_empty()) {
            Some(text) => {
                self.map.insert(key.to_string(), text.to_string());
            }
            None => {
                self.map.remove(key);
            }
        }
    }

    pub fn text(&self) -> Option<String> {
        let joined = self
            .map
            .values()
            .map(|text| {
                strip_ansi(text)
                    .replace(['\n', '\r'], " ")
                    .trim()
                    .to_string()
            })
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        if joined.is_empty() {
            None
        } else {
            Some(joined)
        }
    }
}

#[derive(Default)]
pub struct Widgets {
    map: BTreeMap<String, (Placement, Vec<String>)>,
}

impl Widgets {
    pub fn set(&mut self, key: &str, lines: Option<Vec<String>>, placement: Placement) {
        if key.is_empty() {
            return;
        }
        match lines.filter(|lines| !lines.is_empty()) {
            Some(lines) => {
                self.map.insert(key.to_string(), (placement, lines));
            }
            None => {
                self.map.remove(key);
            }
        }
    }

    pub fn render(&self, placement: Placement) -> Option<String> {
        let lines: Vec<String> = self
            .map
            .values()
            .filter(|(at, _)| *at == placement)
            .flat_map(|(_, lines)| lines.iter().map(|line| strip_ansi(line)))
            .collect();
        if lines.is_empty() {
            return None;
        }
        Some(clamp_lines(&lines, MAX_LINES))
    }
}

pub const MAX_LINES: usize = 4;

pub fn clamp_lines(lines: &[String], max: usize) -> String {
    let mut shown: Vec<String> = lines.iter().take(max).cloned().collect();
    if lines.len() > max && max > 0 {
        if let Some(last) = shown.last_mut() {
            if !last.trim_end().ends_with('…') {
                last.push_str(" …");
            }
        }
    }
    shown.join("\n")
}


