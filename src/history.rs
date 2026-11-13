
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::config;
use crate::state::{RemoteSession, Transport};

// Return only the metadata needed by SessionFields, never full tool output.
pub const HISTORY_CMD: &str = r#"python3 - <<'PY'
import json
from pathlib import Path
files = []
for path in (Path.home() / '.pi/agent/sessions').glob('*/*.jsonl'):
    try:
        files.append((int(path.stat().st_mtime), path))
    except OSError:
        continue
for mtime, path in sorted(files, reverse=True)[:30]:
    records = []
    has_preview = False
    try:
        with path.open() as stream:
            for line in stream:
                try:
                    entry = json.loads(line)
                except ValueError:
                    continue
                if entry.get('type') in ('session', 'session_info', 'model_change'):
                    records.append(entry)
                elif entry.get('type') == 'message':
                    message = entry.get('message', {})
                    if message.get('role') == 'user' and not has_preview:
                        records.append(entry)
                        has_preview = bool(message.get('content'))
                    elif message.get('role') == 'assistant':
                        message = {key: message[key] for key in ('role','provider','model','modelId','timestamp') if key in message}
                        records.append({'type':'message','timestamp':entry.get('timestamp'),'message':message})
    except OSError:
        continue
    print(json.dumps({'path':str(path),'mtime':mtime,'records':records}))
PY"#;

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
    last_model: Option<String>,
    last_response: Option<i64>,
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
                let model_id = record.get("modelId").or_else(|| record.get("Model")).and_then(Value::as_str);
                if let (Some(provider), Some(model_id)) = (provider, model_id) {
                    self.model = Some(format!("{provider}/{model_id}"));
                }
            }
            "message" => {
                let message = record.get("message").unwrap_or(record);
                if message.get("role").and_then(Value::as_str) == Some("assistant") {
                    if let Some(model) = message.get("model").or_else(|| message.get("modelId")).and_then(Value::as_str) {
                        let provider = message.get("provider").and_then(Value::as_str).unwrap_or("");
                        self.last_model = Some(if provider.is_empty() { model.to_string() } else { format!("{provider}/{model}") });
                    }
                    self.last_response = record.get("timestamp").and_then(timestamp_seconds)
                        .or_else(|| message.get("timestamp").and_then(timestamp_seconds))
                        .or(self.last_response);
                    return;
                }
                if message.get("role").and_then(Value::as_str) != Some("user") {
                    return;
                }
                self.has_user = true;
                if self.preview.is_none() {
                  if let Some(text) = content_text(message.get("content")) {
                    let text = text.trim();
                    if !text.is_empty() {
                        self.preview = Some(text.to_string());
                    }
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
            model: self.last_model.or(self.model),
            preview: self.preview,
            session_id: self.session_id,
            mtime,
            last_response: self.last_response,
        })
    }
}

fn timestamp_seconds(value: &Value) -> Option<i64> {
    if let Some(millis) = value.as_i64() {
        return Some(millis / 1000);
    }
    glib::DateTime::from_iso8601(value.as_str()?, None).ok().map(|time| time.to_unix())
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
    text.lines().filter_map(parse_remote_session).collect()
}

fn parse_remote_session(line: &str) -> Option<RemoteSession> {
    let data: Value = serde_json::from_str(line).ok()?;
    let mut session = SessionFields::default();
    for record in data.get("records")?.as_array()? { session.consume(record); }
    session.finish(data.get("path")?.as_str()?.to_string(), data.get("mtime")?.as_i64()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn history_tracks_the_last_response_model_and_time_after_the_preview() {
        let mut session = SessionFields::default();
        for record in [
            json!({"type":"session", "id":"session-1"}),
            json!({"type":"model_change", "provider":"openai", "modelId":"old"}),
            json!({"type":"message", "message":{"role":"user", "content":"First prompt"}}),
            json!({"type":"message", "timestamp":"2026-11-12T10:00:00Z", "message":{"role":"assistant", "provider":"openai", "model":"old"}}),
            json!({"type":"message", "timestamp":"2026-11-12T11:00:00Z", "message":{"role":"assistant", "provider":"anthropic", "model":"latest"}}),
            json!({"type":"model_change", "provider":"google", "modelId":"selected-but-unused"}),
        ] { session.consume(&record); }
        let session = session.finish("/test.jsonl".into(), 1).unwrap();
        assert_eq!(session.model.as_deref(), Some("anthropic/latest"));
        assert_eq!(session.preview.as_deref(), Some("First prompt"));
        assert_eq!(session.last_response, Some(1794481200));
    }

    #[test]
    fn model_id_is_used_when_no_assistant_reply_exists() {
        let mut session = SessionFields::default();
        session.consume(&json!({"type":"model_change", "provider":"openai", "modelId":"gpt-6.1-sol"}));
        session.consume(&json!({"type":"message", "message":{"role":"user", "content":"Hi"}}));
        assert_eq!(session.finish("/test.jsonl".into(), 1).unwrap().model.as_deref(), Some("openai/gpt-6.1-sol"));
    }

    #[test]
    fn remote_history_uses_the_same_metadata_parser() {
        let data = json!({"path":"/remote/session.jsonl", "mtime":12, "records":[
            {"type":"session", "id":"remote"},
            {"type":"message", "message":{"role":"user","content":"A prompt with\ttabs"}},
            {"type":"message", "message":{"role":"assistant","model":"latest","provider":"openai","timestamp":1794481200000_i64}}
        ]});
        let session = parse_remote_session(&data.to_string()).unwrap();
        assert_eq!(session.model.as_deref(), Some("openai/latest"));
        assert_eq!(session.last_response, Some(1794481200));
        assert_eq!(session.preview.as_deref(), Some("A prompt with\ttabs"));
    }
}
