
use std::collections::{HashMap, HashSet};

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    User,
    Assistant,
    Tool,
    System,
    Model,
    Thinking,
    Compaction,
    SessionInfo,
    Edit,
}

impl Kind {
    pub fn icon(self) -> &'static str {
        match self {
            Kind::User => "message",
            Kind::Assistant => "robot",
            Kind::Tool => "terminal",
            Kind::System => "box",
            Kind::Model => "sliders",
            Kind::Thinking => "moon",
            Kind::Compaction => "compact",
            Kind::SessionInfo => "file",
            Kind::Edit => "pencil",
        }
    }

    fn fallback_label(self) -> &'static str {
        match self {
            Kind::User => "message",
            Kind::Assistant => "response",
            Kind::Tool => "tool",
            Kind::System => "system",
            Kind::Model => "model",
            Kind::Thinking => "reasoning",
            Kind::Compaction => "compaction",
            Kind::SessionInfo => "Session",
            Kind::Edit => "editing context",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub depth: usize,
    pub kind: Kind,
    pub label: String,
    pub text: String,
    pub is_leaf: bool,
    pub is_on_leaf_path: bool,
    pub entry_type: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeView {
    pub nodes: Vec<Node>,
    pub leaf_id: Option<String>,
}

impl TreeView {
    pub fn count(&self) -> usize {
        self.nodes.len()
    }

    pub fn roots(&self) -> usize {
        self.nodes.iter().filter(|node| node.depth == 0).count()
    }
}

pub fn parse(data: &Value) -> TreeView {
    let data = if data.get("tree").is_some() {
        data
    } else {
        data.get("data")
            .filter(|data| data.get("tree").is_some())
            .unwrap_or(data)
    };

    let leaf_id = data
        .get("leafId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string);

    let mut nodes = Vec::new();
    let mut parents: HashMap<String, Option<String>> = HashMap::new();
    if let Some(roots) = data.get("tree").and_then(Value::as_array) {
        for root in roots {
            walk(root, 0, &mut nodes, &mut parents);
        }
    }

    let mut active = HashSet::new();
    let mut cursor = leaf_id.clone();
    while let Some(id) = cursor {
        if !active.insert(id.clone()) {
            break; 
        }
        cursor = parents.get(&id).cloned().flatten();
    }

    for node in &mut nodes {
        node.is_leaf = leaf_id.as_deref() == Some(node.id.as_str());
        node.is_on_leaf_path = active.contains(&node.id);
    }

    TreeView { nodes, leaf_id }
}

fn walk(
    value: &Value,
    depth: usize,
    out: &mut Vec<Node>,
    parents: &mut HashMap<String, Option<String>>,
) {
    let Some(entry) = value.get("entry").filter(|entry| entry.is_object()) else {
        return;
    };
    let Some(id) = entry
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    else {
        return;
    };
    let children: &[Value] = match value.get("children") {
        None => &[],
        Some(Value::Array(children)) => children,
        Some(_) => return,
    };

    parents.insert(
        id.to_string(),
        entry
            .get("parent")
            .and_then(Value::as_str)
            .filter(|parent| !parent.is_empty())
            .map(str::to_string),
    );
    let entry_type = entry
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let kind = kind_of(entry, &entry_type);
    let text = entry_text(entry, &entry_type);
    let label = entry_label(entry, &entry_type, kind, &text);

    out.push(Node {
        id: id.to_string(),
        depth,
        kind,
        label,
        text,
        is_leaf: false,
        is_on_leaf_path: false,
        entry_type,
        timestamp: entry
            .get("timestamp")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    });

    for child in children {
        walk(child, depth + 1, out, parents);
    }
}

fn kind_of(entry: &Value, entry_type: &str) -> Kind {
    match entry_type {
        "model_change" => Kind::Model,
        "thinking_level_change" => Kind::Thinking,
        "compaction_start" | "compaction_end" | "compaction" => Kind::Compaction,
        "session_info" => Kind::SessionInfo,
        "context_edit" | "edit" => Kind::Edit,
        "message" => match entry
            .get("message")
            .and_then(|message| message.get("role"))
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "user" => Kind::User,
            "assistant" => Kind::Assistant,
            "toolResult" | "tool" => Kind::Tool,
            "system" => Kind::System,
            _ => Kind::System,
        },
        _ => Kind::System,
    }
}

fn entry_text(entry: &Value, entry_type: &str) -> String {
    match entry_type {
        "message" => {
            let message = entry.get("message").unwrap_or(entry);
            content_text(message.get("content"))
        }
        "model_change" => {
            let provider = entry.get("provider").and_then(Value::as_str).unwrap_or("");
            let model = entry.get("Model").and_then(Value::as_str).unwrap_or("");
            match (provider.is_empty(), model.is_empty()) {
                (false, false) => format!("{provider}/{model}"),
                (false, true) => provider.to_string(),
                (true, false) => model.to_string(),
                (true, true) => String::new(),
            }
        }
        "thinking_level_change" => entry
            .get("thinkingLevel")
            .or_else(|| entry.get("level"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "compaction_start" | "compaction_end" | "compaction" => "compaction".to_string(),
        "session_info" => entry
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "context_edit" | "edit" => {
            let replacement = entry.get("replacement").and_then(Value::as_str).unwrap_or("");
            let target = entry.get("Target").and_then(Value::as_str).unwrap_or("");
            if replacement.is_empty() {
                target.to_string()
            } else {
                replacement.to_string()
            }
        }
        _ => entry
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

fn content_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn entry_label(entry: &Value, entry_type: &str, kind: Kind, text: &str) -> String {
    let raw = match entry_type {
        "message" => text.to_string(),
        "model_change" | "thinking_level_change" | "compaction_start" | "compaction_end"
        | "compaction" => text.to_string(),
        "session_info" => {
            if text.is_empty() {
                String::new()
            } else {
                format!("session → {text}")
            }
        }
        "context_edit" | "edit" => {
            if text.is_empty() {
                String::new()
            } else {
                format!("editing context · {text}")
            }
        }
        _ => entry
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(text)
            .to_string(),
    };
    let first = raw.lines().next().unwrap_or("").trim();
    let label = truncate_label(first);
    if label.is_empty() {
        kind.fallback_label().to_string()
    } else {
        label
    }
}

fn truncate_label(text: &str) -> String {
    let clean = text.replace('\t', " ");
    if clean.chars().count() <= 80 {
        return clean;
    }
    let mut out: String = clean.chars().take(79).collect();
    out.push('…');
    out
}


