
use std::path::{Path, PathBuf};

pub const MAX_FILES: usize = 4_000;
pub const MAX_DEPTH: usize = 8;
pub const MAX_FILE_BYTES: usize = 32 * 1024;
pub const MAX_TOTAL_BYTES: usize = 96 * 1024;
pub const MAX_REFS: usize = 8;

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    ".next",
    ".turbo",
    ".gradle",
    ".idea",
    ".vscode",
];

fn skip_dir(name: &str) -> bool {
    (name.starts_with('.') && name != ".github") || SKIP_DIRS.contains(&name)
}

pub fn list_files(root: &Path, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, root, 0, limit.min(MAX_FILES), &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, depth: usize, limit: usize, out: &mut Vec<String>) {
    if depth > MAX_DEPTH || out.len() >= limit {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= limit {
            return;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if file_type.is_dir() {
            if skip_dir(&name) || name.starts_with('.') && name != ".github" {
                continue;
            }
            walk(root, &entry.path(), depth + 1, limit, out);
        } else if file_type.is_file() {
            if name.starts_with('.') {
                continue;
            }
            if let Ok(relative) = entry.path().strip_prefix(root) {
                out.push(relative.to_string_lossy().to_string());
            }
        }
    }
}

pub fn expand_mentions(text: &str, root: &Path) -> String {
    let mut out = String::with_capacity(text.len() + 256);
    let mut used_bytes = 0usize;
    let mut refs = 0usize;

    let mut rest = text;
    while let Some(at) = rest.find('@') {
        let before = &rest[..at];
        let word_start = before.is_empty()
            || before
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_whitespace() || "([{\"'`".contains(ch));
        out.push_str(before);
        let after = &rest[at + 1..];
        let end = after
            .find(|ch: char| ch.is_whitespace() || ")]},;:\"'`".contains(ch))
            .unwrap_or(after.len());
        let token = after[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        let end = token.len();
        let token = token;
        out.push('@');
        if !word_start || token.is_empty() || refs >= MAX_REFS {
            out.push_str(token);
            rest = &after[end..];
            continue;
        }
        match reference(root, token, used_bytes) {
            Some((block, bytes)) => {
                out.push_str(token);
                out.push('\n');
                out.push_str(&block);
                used_bytes += bytes;
                refs += 1;
            }
            None => out.push_str(token),
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn reference(root: &Path, token: &str, used: usize) -> Option<(String, usize)> {
    let relative = Path::new(token);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    let candidate: PathBuf = root.join(relative);
    if !candidate.is_file() {
        return None;
    }
    let bytes = std::fs::read(&candidate).ok()?;
    let too_big = bytes.len() > MAX_FILE_BYTES || used + bytes.len() > MAX_TOTAL_BYTES;
    if too_big {
        let notice = format!(
            "<file path=\"{token}\" truncated=\"true\">file with {} bytes: use the read tool</file>",
            bytes.len()
        );
        return Some((notice, 0));
    }
    let Ok(content) = String::from_utf8(bytes) else {
        return None;
    };
    let block = format!("<file path=\"{token}\">\n{content}\n</file>");
    Some((block, content.len()))
}


