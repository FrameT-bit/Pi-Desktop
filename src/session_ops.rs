
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

pub fn slug_to_path(slug: &str) -> Option<PathBuf> {
    let inner = slug.strip_prefix("--")?.strip_suffix("--")?;
    if inner.is_empty() || inner.starts_with('-') {
        return None;
    }
    if !inner
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return None;
    }
    let decoded = inner.replace('-', "/");
    if decoded.is_empty() {
        return None;
    }
    Some(PathBuf::from("/").join(decoded))
}

pub fn project_label(slug: &str) -> String {
    let Some(path) = slug_to_path(slug) else {
        return if slug.is_empty() {
            "no design".to_string()
        } else {
            slug.to_string()
        };
    };
    let home = crate::config::home_file("");
    if path == home {
        return "~".to_string();
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("~")
        .to_string()
}

pub fn session_slug(path: &str) -> String {
    Path::new(path)
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

pub fn apply_name_to_jsonl(content: &str, name: &str) -> Option<String> {
    let trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    if lines.is_empty() {
        return None;
    }

    let first = serde_json::from_str::<Value>(lines[0].trim()).ok()?;
    if first.get("name").is_some() {
        lines[0] = replace_name(&lines[0], name)?;
        return Some(join_lines(lines, trailing_newline));
    }

    for index in (1..lines.len()).rev() {
        let Ok(record) = serde_json::from_str::<Value>(lines[index].trim()) else {
            continue;
        };
        if record.get("type").and_then(Value::as_str) == Some("session_info") {
            lines[index] = replace_name(&lines[index], name)?;
            return Some(join_lines(lines, trailing_newline));
        }
    }

    None
}

fn replace_name(line: &str, name: &str) -> Option<String> {
    let mut value = serde_json::from_str::<Value>(line.trim()).ok()?;
    let object = value.as_object_mut()?;
    object.insert("name".to_string(), Value::String(name.to_string()));
    serde_json::to_string(&value).ok()
}

fn join_lines(lines: Vec<String>, trailing_newline: bool) -> String {
    let mut joined = lines.join("\n");
    if trailing_newline {
        joined.push('\n');
    }
    joined
}

#[allow(dead_code)]
pub fn name_in_jsonl(content: &str) -> Option<String> {
    let mut name = None;
    for line in content.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if record.get("type").and_then(Value::as_str) == Some("session_info") {
            name = record
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
                .filter(|value| !value.trim().is_empty());
        }
    }
    name
}

pub fn clone_jsonl(content: &str, new_id: &str) -> Option<String> {
    let trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    if lines.is_empty() {
        return None;
    }

    let first = serde_json::from_str::<Value>(lines[0].trim()).ok()?;
    if first.get("type").and_then(Value::as_str) != Some("session") {
        return None;
    }
    let mut first = first;
    let object = first.as_object_mut()?;
    object.insert("id".to_string(), Value::String(new_id.to_string()));
    if let Some(name) = object.get("name").and_then(Value::as_str) {
        object.insert("name".to_string(), Value::String(copy_name(name)));
    }
    lines[0] = serde_json::to_string(&first).ok()?;

    for index in (1..lines.len()).rev() {
        let Ok(record) = serde_json::from_str::<Value>(lines[index].trim()) else {
            continue;
        };
        if record.get("type").and_then(Value::as_str) == Some("session_info") {
            if let Some(name) = record.get("name").and_then(Value::as_str) {
                lines[index] = replace_name(&lines[index], &copy_name(name))?;
                break;
            }
        }
    }

    Some(join_lines(lines, trailing_newline))
}

fn copy_name(name: &str) -> String {
    if name.ends_with("(copying)") {
        name.to_string()
    } else {
        format!("{name} (copia)")
    }
}

pub fn clone_file_name(original: &Path, new_id: &str, now: &str) -> String {
    let stamp = original
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.split('_').next())
        .filter(|stamp| !stamp.is_empty())
        .unwrap_or(now);
    format!("{stamp}_{new_id}.jsonl")
}

pub fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Session.jsonl");
    let tmp = dir.join(format!(".{base}.tmp-{}", std::process::id()));
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)
}

pub fn rename_session_file(path: &Path, name: &str) -> std::io::Result<bool> {
    let content = fs::read_to_string(path)?;
    match apply_name_to_jsonl(&content, name) {
        Some(updated) => {
            write_atomic(path, &updated)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub fn clone_session_file(path: &Path, new_id: &str, now: &str) -> std::io::Result<PathBuf> {
    let content = fs::read_to_string(path)?;
    let cloned = clone_jsonl(&content, new_id)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid session jsonl"))?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let target = dir.join(clone_file_name(path, new_id, now));
    write_atomic(&target, &cloned)?;
    Ok(target)
}

pub fn new_uuid() -> String {
    let mut bytes = [0u8; 16];
    let filled = fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_ok();
    if !filled {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (nanos >> ((index % 16) * 8)) as u8;
        }
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

pub fn trash_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| crate::config::home_file(".local/share"))
        .join("pi-desktop")
        .join("trash")
}

pub fn move_to_trash(path: &Path, stamp: &str) -> std::io::Result<PathBuf> {
    move_to_trash_in(path, stamp, &trash_dir())
}

pub fn move_to_trash_in(path: &Path, stamp: &str, dir: &Path) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Session.jsonl");
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) => (stem, format!(".{ext}")),
        None => (name, String::new()),
    };
    let target = dir.join(format!("{stem}.{stamp}{ext}"));
    match fs::rename(path, &target) {
        Ok(()) => Ok(target),
        Err(_) => {
            fs::copy(path, &target)?;
            fs::remove_file(path)?;
            Ok(target)
        }
    }
}

pub fn downloads_dir() -> PathBuf {
    std::env::var_os("XDG_DOWNLOAD_DIR")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| crate::config::home_file("Downloads"))
}

pub fn export_path(session_slug: &str, stamp: &str) -> PathBuf {
    downloads_dir().join(format!("pi-{session_slug}-{stamp}.html"))
}

pub fn export_stamp(now: &glib::DateTime) -> String {
    now.format("%Y%m%d-%H%M")
        .map(|text| text.to_string())
        .unwrap_or_else(|_| "00000000-0000".to_string())
}

pub fn trash_stamp(now: &glib::DateTime) -> String {
    now.format("%Y%m%d-%H%M%S")
        .map(|text| text.to_string())
        .unwrap_or_else(|_| "00000000-000000".to_string())
}



pub const WORKSPACE_TRASH_DAYS: u64 = 14;

#[derive(Clone)]
pub struct WorkspaceTrashEntry {
    pub path: PathBuf,
    pub original: PathBuf,
    pub label: String,
    pub sessions: usize,
    pub deleted: u64,
}

pub fn workspace_trash_dir() -> PathBuf {
    trash_dir().join("workspaces")
}

pub fn trash_workspace(
    dir: &Path,
    label: &str,
    sessions: usize,
    stamp: &str,
) -> std::io::Result<PathBuf> {
    trash_workspace_in(dir, label, sessions, stamp, &workspace_trash_dir())
}

fn trash_workspace_in(
    dir: &Path,
    label: &str,
    sessions: usize,
    stamp: &str,
    root: &Path,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let slug = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace");
    let mut target = root.join(format!("{slug}.{stamp}"));
    let mut suffix = 1;
    while target.exists() {
        target = root.join(format!("{slug}.{stamp}-{suffix}"));
        suffix += 1;
    }
    move_dir(dir, &target)?;
    let deleted = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let meta = serde_json::json!({
        "original": dir.to_string_lossy(),
        "label": label,
        "sessions": sessions,
        "deleted": deleted,
        "stamp": stamp,
    });
    if let Err(error) = fs::write(target.join("meta.json"), serde_json::to_vec_pretty(&meta)?) {
        let _ = move_dir(&target, dir);
        return Err(error);
    }
    Ok(target)
}

pub fn list_workspace_trash() -> Vec<WorkspaceTrashEntry> {
    list_workspace_trash_in(&workspace_trash_dir())
}

fn list_workspace_trash_in(root: &Path) -> Vec<WorkspaceTrashEntry> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(text) = fs::read_to_string(path.join("meta.json")) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(original) = meta.get("original").and_then(Value::as_str) else {
            continue;
        };
        let deleted = meta.get("deleted").and_then(Value::as_u64).unwrap_or(0);
        if deleted > 0 && now.saturating_sub(deleted) > WORKSPACE_TRASH_DAYS * 86_400 {
            let _ = fs::remove_dir_all(&path);
            continue;
        }
        out.push(WorkspaceTrashEntry {
            path,
            original: PathBuf::from(original),
            label: meta
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("workspace")
                .to_string(),
            sessions: meta
                .get("sessions")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
            deleted,
        });
    }
    out.sort_by(|a, b| b.deleted.cmp(&a.deleted));
    out
}

pub fn restore_workspace(entry: &WorkspaceTrashEntry) -> std::io::Result<()> {
    if entry.original.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} already exists", entry.original.display()),
        ));
    }
    if let Some(parent) = entry.original.parent() {
        fs::create_dir_all(parent)?;
    }
    move_dir(&entry.path, &entry.original)?;
    let _ = fs::remove_file(entry.original.join("meta.json"));
    Ok(())
}

pub fn restore_workspace_path(path: &Path) -> std::io::Result<()> {
    let text = fs::read_to_string(path.join("meta.json"))?;
    let meta = serde_json::from_str::<Value>(&text)?;
    let original = meta
        .get("original")
        .and_then(Value::as_str)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid workspace meta"))?;
    restore_workspace(&WorkspaceTrashEntry {
        path: path.to_path_buf(),
        original: PathBuf::from(original),
        label: meta
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("workspace")
            .to_string(),
        sessions: meta
            .get("sessions")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        deleted: meta.get("deleted").and_then(Value::as_u64).unwrap_or(0),
    })
}

fn move_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_dir_all(from, to)?;
            fs::remove_dir_all(from)
        }
    }
}

fn copy_dir_all(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}


#[cfg(test)]
mod workspace_trash_tests {
    use super::*;

    #[test]
    fn workspace_trash_round_trip() {
        let base = std::env::temp_dir().join(format!(
            "pi-desktop-workspace-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let workspace = base.join("--tmp-workspace-test--");
        let trash = base.join("trash");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("session.jsonl"), "{}").unwrap();

        let target = trash_workspace_in(&workspace, "test workspace", 1, "stamp", &trash).unwrap();
        assert!(!workspace.exists());
        assert!(target.join("meta.json").exists());

        let entries = list_workspace_trash_in(&trash);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sessions, 1);
        restore_workspace(&entries[0]).unwrap();
        assert!(workspace.join("session.jsonl").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn workspace_trash_purges_after_fourteen_days() {
        let base = std::env::temp_dir().join(format!(
            "pi-desktop-workspace-expiry-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let workspace = base.join("--tmp-workspace-expiry--");
        let trash = base.join("trash");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("session.jsonl"), "{}").unwrap();
        let target = trash_workspace_in(&workspace, "expired", 1, "stamp", &trash).unwrap();
        let meta_path = target.join("meta.json");
        let mut meta: Value =
            serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        meta["deleted"] = Value::from(1);
        fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
        assert!(list_workspace_trash_in(&trash).is_empty());
        assert!(!target.exists());
        let _ = fs::remove_dir_all(&base);
    }
}
