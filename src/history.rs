
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::config;
use crate::state::{RemoteSession, Transport};

pub const HISTORY_CMD: &str = concat!(
    "ls -1t -- ~/.pi/agent/sessions/*/*.jsonl 2>/dev/null | head -30 | ",
    "while IFS= read -r f; do ",
    "mt=$(stat -c %Y \"$f\" 2>/dev/null || echo 0);",
    "sid=$(sed -n '1s/.*\"id\":\"\\([^\"]*\\)\".*/\\1/p' \"$f\");",
    "printf '%s\\t%s\\t%s\\t%s\\t%s\\t%s\\n' \"$f\" \"$mt\" \"$mi\" \"$nm\" \"$pv\" \"$sid\";",
    "done"
);

pub fn spawn_fetch(transport: Transport, tx: async_channel::Sender<Vec<RemoteSession>>) {
    thread::Builder::new()
        .name("pi-history".into())
        .spawn(move || {
            let sessions = fetch_blocking(&transport);
            let _ = tx.send_blocking(sessions);
        })
        .ok();
}

fn fetch_blocking(transport: &Transport) -> Vec<RemoteSession> {
    match transport {
        Transport::Local(_) => local_sessions(),
        Transport::Remote { target, .. } => remote_sessions(target),
        Transport::Missing { .. } => Vec::new(),
    }
}


fn local_sessions() -> Vec<RemoteSession> {
    let root = config::home_file(".pi/agent/sessions");
    let Ok(dirs) = fs::read_dir(&root) else {
        return Vec::new();
    };

    let mut files: Vec<(PathBuf, i64)> = Vec::new();
    for dir in dirs.flatten() {
        let path = dir.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let file = entry.path();
            if file.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(0);
            files.push((file, mtime));
        }
    }

    files.sort_by(|left, right| right.1.cmp(&left.1));
    files.truncate(30);
    files
        .into_iter()
        .filter_map(|(path, mtime)| read_session_file(&path, mtime))
        .collect()
}

fn read_session_file(path: &Path, mtime: i64) -> Option<RemoteSession> {
    let file = fs::File::open(path).ok()?;
    let mut session = SessionFields::default();
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        session.consume(&record);
    }
    session.finish(path.to_string_lossy().to_string(), mtime)
}

#[derive(Default)]
struct SessionFields {
    session_id: String,
    name: Option<String>,
    model: Option<String>,
    preview: Option<String>,
    has_user: bool,
}

impl SessionFields {
    fn consume(&mut self, record: &Value) {
        match record.get("type").and_then(Value::as_str).unwrap_or("") {
            "session" => {
                if self.session_id.is_empty() {
                    if let Some(id) = record.get("id").and_then(Value::as_str) {
                        self.session_id = id.to_string();
                    }
                }
            }
            "session_info" => {
                if let Some(name) = record.get("name").and_then(Value::as_str) {
                    if !name.trim().is_empty() {
                        self.name = Some(name.to_string());
                    }
                }
            }
            "model_change" => {
                let provider = record.get("provider").and_then(Value::as_str);
                let model_id = record.get("Model").and_then(Value::as_str);
                if let (Some(provider), Some(model_id)) = (provider, model_id) {
                    self.model = Some(format!("{provider}/{model_id}"));
                }
            }
            "message" => {
                if self.preview.is_some() {
                    return;
                }
                let message = record.get("message").unwrap_or(record);
                if message.get("role").and_then(Value::as_str) != Some("user") {
                    return;
                }
                self.has_user = true;
                if let Some(text) = content_text(message.get("content")) {
                    let text = text.trim();
                    if !text.is_empty() {
                        self.preview = Some(text.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    fn finish(self, path: String, mtime: i64) -> Option<RemoteSession> {
        if !self.has_user {
            return None;
        }
        Some(RemoteSession {
            path,
            name: self.name,
            model: self.model,
            preview: self.preview,
            session_id: self.session_id,
            mtime,
        })
    }
}

fn content_text(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => items.iter().find_map(|item| {
            if item.get("type").and_then(Value::as_str) == Some("text") {
                item.get("text").and_then(Value::as_str).map(str::to_string)
            } else {
                None
            }
        }),
        _ => None,
    }
}

pub fn read_messages(path: &Path) -> Vec<Value> {
    let Ok(file) = fs::File::open(path) else {
        return Vec::new();
    };
    let mut messages = Vec::new();
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if record.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        if let Some(message) = record.get("message") {
            messages.push(message.clone());
        }
    }
    messages
}


fn remote_sessions(target: &str) -> Vec<RemoteSession> {
    let output = Command::new("ssh")
        .arg("-T")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=8")
        .arg(target)
        .arg(HISTORY_CMD)
        .stdin(Stdio::null())
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut sessions = Vec::new();
    for line in text.lines() {
        let mut fields = line.split('\t');
        let Some(path) = fields.next() else { continue };
        if path.trim().is_empty() {
            continue;
        }
        let mtime = fields
            .next()
            .and_then(|value| value.trim().parse::<i64>().ok())
            .unwrap_or(0);
        let model = clean(fields.next());
        let name = clean(fields.next());
        let preview = clean(fields.next());
        if preview.is_none() {
            continue;
        }
        let session_id = clean(fields.next()).unwrap_or_default();
        sessions.push(RemoteSession {
            path: path.to_string(),
            name,
            model,
            preview,
            session_id,
            mtime,
        });
    }
    sessions
}

fn clean(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.replace("\\\"", "\"").replace("\\n", " "))
}


