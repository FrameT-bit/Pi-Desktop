
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactKind {
    Image,
    File,
    Link,
}

impl ArtifactKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::File => "file",
            Self::Link => "link",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub value: String,
    pub label: String,
    pub session_id: String,
    pub session_title: String,
    pub timestamp: i64,
}

const MAX_FILES: usize = 30;
const MAX_ARTIFACTS: usize = 500;
const MAX_TEXT: usize = 200_000;

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];
const FILE_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "pdf", "txt", "json", "md", "csv", "xls",
    "xlsx", "doc", "docx", "ppt", "pptx", "html", "zip", "tar", "gz", "avi", "flac", "m4a", "mkv",
    "mp3", "ogg", "opus", "wav", "webm", "mp4", "mov", "rs", "py", "js", "ts", "tsx", "jsx", "css",
    "sh", "yaml", "yml", "toml", "sql", "go", "c", "h", "cpp", "hpp", "java", "kt", "rb", "php",
    "log",
];
const STRONG_KEYS: &[&str] = &[
    "artifact",
    "artifacts",
    "artifact_file",
    "artifact_image",
    "artifact_path",
    "artifact_url",
    "file",
    "file_path",
    "files",
    "files_created",
    "files_modified",
    "files_written",
    "generated_file",
    "generated_image",
    "generated_path",
    "generated_url",
    "image",
    "image_path",
    "local_path",
    "media",
    "media_tag",
    "output",
    "output_file",
    "output_path",
    "output_url",
    "path",
    "result_file",
    "result_path",
    "result_url",
    "saved_to",
    "screenshot_path",
    "url",
    "video",
];

pub fn session_root(agent_dir: &Path) -> PathBuf {
    agent_dir.join("sessions")
}

pub fn recent_session_files(root: &Path, limit: usize) -> Vec<PathBuf> {
    let mut files: Vec<(SystemTime, PathBuf)> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0;
    while let Some(dir) = stack.pop() {
        if visited > 2000 {
            break;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                if let Ok(metadata) = entry.metadata() {
                    files.push((metadata.modified().unwrap_or(UNIX_EPOCH), path));
                }
            }
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn image_kind(value: &str) -> bool {
    let lower = value.to_lowercase();
    IMAGE_EXT
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

fn has_extension(value: &str) -> bool {
    let lower = value.to_lowercase();
    FILE_EXT
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

fn clean_value(raw: &str) -> String {
    raw.trim()
        .trim_matches(['`', '"', '\'', '<', '>', '(', ')', '[', ']'])
        .trim_end_matches([',', '.', ';', ':', '!', '?'])
        .to_string()
}

fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("data:image/")
}

fn looks_like_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("~/")
        || value.starts_with("./")
        || value.starts_with("../")
}

fn resolve_path(cwd: &str, raw: &str) -> Option<PathBuf> {
    let expanded = if let Some(rest) = raw.strip_prefix("~/") {
        PathBuf::from(std::env::var_os("HOME")?).join(rest)
    } else if raw.starts_with('/') {
        PathBuf::from(raw)
    } else {
        Path::new(cwd).join(raw)
    };
    if expanded.is_file() {
        Some(expanded)
    } else if expanded.is_dir() && has_extension(raw) {
        Some(expanded)
    } else {
        None
    }
}

fn push_candidate(
    out: &mut Vec<Artifact>,
    seen: &mut HashSet<String>,
    session_id: &str,
    session_title: &str,
    cwd: &str,
    timestamp: i64,
    raw: &str,
) {
    let value = clean_value(raw);
    if value.len() < 2 || value.len() > 4096 {
        return;
    }
    let kind = if looks_like_url(&value) {
        if image_kind(&value) {
            ArtifactKind::Image
        } else {
            ArtifactKind::Link
        }
    } else if has_extension(&value)
        && !value.chars().any(char::is_whitespace)
        && resolve_path(cwd, &value).is_some()
    {
        if image_kind(&value) {
            ArtifactKind::Image
        } else {
            ArtifactKind::File
        }
    } else {
        return;
    };
    let key = format!("{session_id}:{}", if kind == ArtifactKind::Link { value.clone() } else { value.clone() });
    if !seen.insert(key) {
        return;
    }
    let label = if kind == ArtifactKind::Link {
        value
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&value)
            .to_string()
    } else {
        Path::new(&value)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&value)
            .to_string()
    };
    out.push(Artifact {
        kind,
        value: if kind == ArtifactKind::File || kind == ArtifactKind::Image {
            resolve_path(cwd, &value)
                .map(|path| path.display().to_string())
                .unwrap_or(value)
        } else {
            value
        },
        label,
        session_id: session_id.to_string(),
        session_title: session_title.to_string(),
        timestamp,
    });
}

fn scan_text(
    text: &str,
    out: &mut Vec<Artifact>,
    seen: &mut HashSet<String>,
    session_id: &str,
    title: &str,
    cwd: &str,
    timestamp: i64,
) {
    let text: String = text.chars().take(MAX_TEXT).collect();
    for marker in ["]("] {
        let mut cursor = 0;
        while let Some(found) = text[cursor..].find(marker) {
            let at = cursor + found + marker.len();
            let Some(end) = text[at..].find(')') else {
                break;
            };
            let value = &text[at..at + end];
            push_candidate(out, seen, session_id, title, cwd, timestamp, value);
            cursor = at + end + 1;
        }
    }
    let mut cursor = 0;
    while cursor < text.len() {
        let rest = &text[cursor..];
        let Some(index) = rest.find("http") else {
            break;
        };
        let at = cursor + index;
        let end = text[at..]
            .find(|ch: char| ch.is_whitespace() || ['"', '\'', ')', ']', '>', '<'].contains(&ch))
            .map(|offset| at + offset)
            .unwrap_or(text.len());
        push_candidate(out, seen, session_id, title, cwd, timestamp, &text[at..end]);
        cursor = end.max(at + 4);
    }
    for (index, _) in text.match_indices("/").chain(text.match_indices("~/")) {
        let start = text[..index]
            .rfind(|ch: char| ch.is_whitespace() || ['(', '"', '\'', '<', ']', '['].contains(&ch))
            .map(|pos| pos + 1)
            .unwrap_or(0);
        let end = text[index..]
            .find(|ch: char| ch.is_whitespace() || ['"', '\'', ')', ']', '>', '<', ','].contains(&ch))
            .map(|offset| index + offset)
            .unwrap_or(text.len());
        if end > start {
            push_candidate(out, seen, session_id, title, cwd, timestamp, &text[start..end]);
        }
    }
}

fn collect_strings(value: &Value, key: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::String(text) => out.push((key.to_string(), text.clone())),
        Value::Array(items) => {
            for item in items {
                collect_strings(item, key, out);
            }
        }
        Value::Object(map) => {
            for (child_key, child) in map {
                let next = if key.is_empty() {
                    child_key.clone()
                } else {
                    format!("{key}.{child_key}")
                };
                collect_strings(child, &next, out);
            }
        }
        _ => {}
    }
}

fn key_is_artifact(key: &str) -> bool {
    key.rsplit('.')
        .next()
        .map(|last| STRONG_KEYS.iter().any(|wanted| wanted.eq_ignore_ascii_case(last)))
        .unwrap_or(false)
}

fn first_text(message: &Value) -> String {
    let content = message.get("content");
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

pub fn parse_session(path: &Path) -> Vec<Artifact> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let metadata = fs::metadata(path).ok();
    let fallback_time = metadata
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_else(now_ms);

    let mut session_id = String::new();
    let mut cwd = String::new();
    let mut name = String::new();
    let mut first_prompt = String::new();
    let mut entries: Vec<(i64, String, Value)> = Vec::new();
    for line in raw.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match value.get("type").and_then(Value::as_str) {
            Some("session") => {
                session_id = value
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                cwd = value
                    .get("cwd")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
            }
            Some("session_info") => {
                if let Some(value) = value.get("name").and_then(Value::as_str) {
                    if !value.trim().is_empty() {
                        name = value.to_string();
                    }
                }
            }
            Some("message") => {
                let Some(message) = value.get("message") else {
                    continue;
                };
                if first_prompt.is_empty()
                    && message.get("role").and_then(Value::as_str) == Some("user")
                {
                    first_prompt = first_text(message);
                }
                let timestamp = message
                    .get("timestamp")
                    .and_then(Value::as_i64)
                    .unwrap_or(fallback_time);
                entries.push((timestamp, String::new(), message.clone()));
            }
            _ => {}
        }
    }
    if name.is_empty() {
        name = first_prompt
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().chars().take(48).collect())
            .unwrap_or_else(|| "(with title)".to_string());
    }
    if session_id.is_empty() {
        session_id = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Session")
            .to_string();
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (timestamp, _, message) in entries {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        match role {
            "assistant" => {
                if let Some(blocks) = message.get("content").and_then(Value::as_array) {
                    for block in blocks {
                        if block.get("type").and_then(Value::as_str) == Some("text") {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                scan_text(&text, &mut out, &mut seen, &session_id, &name, &cwd, timestamp);
                            }
                        } else if block.get("type").and_then(Value::as_str) == Some("toolCall") {
                            let mut strings = Vec::new();
                            if let Some(args) = block.get("arguments") {
                                collect_strings(args, "", &mut strings);
                            }
                            for (key, value) in strings {
                                if key_is_artifact(&key) {
                                    push_candidate(
                                        &mut out,
                                        &mut seen,
                                        &session_id,
                                        &name,
                                        &cwd,
                                        timestamp,
                                        &value,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            "toolResult" => {
                let text = first_text(&message);
                scan_text(&text, &mut out, &mut seen, &session_id, &name, &cwd, timestamp);
                let mut strings = Vec::new();
                if let Some(content) = message.get("content") {
                    collect_strings(content, "", &mut strings);
                }
                for (key, value) in strings {
                    if key_is_artifact(&key) {
                        push_candidate(&mut out, &mut seen, &session_id, &name, &cwd, timestamp, &value);
                    }
                }
            }
            _ => {}
        }
        if out.len() >= MAX_ARTIFACTS {
            break;
        }
    }
    out
}

pub fn scan(agent_dir: &Path, limit: usize) -> Vec<Artifact> {
    let root = session_root(agent_dir);
    let mut out = Vec::new();
    for path in recent_session_files(&root, limit.max(1).min(MAX_FILES)) {
        out.extend(parse_session(&path));
        if out.len() >= MAX_ARTIFACTS {
            break;
        }
    }
    out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    out.truncate(MAX_ARTIFACTS);
    out
}


