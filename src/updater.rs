
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const REPO: &str = "FrameT-bit/Pi-Desktop";
pub const REPO_URL: &str = "https://github.com/FrameT-bit/Pi-Desktop";
pub const RELEASES_URL: &str = "https://github.com/FrameT-bit/Pi-Desktop/releases";
pub const ISSUES_URL: &str = "https://github.com/FrameT-bit/Pi-Desktop/issues";
const CHECK_INTERVAL_SECS: i64 = 7 * 24 * 60 * 60;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UpdateState {
    pub checked_at: i64,
    pub current: String,
    pub latest: String,
    pub url: String,
    pub notes: String,
    pub error: String,
}

impl UpdateState {
    pub fn has_update(&self) -> bool {
        !self.latest.is_empty() && version_gt(&self.latest, &self.current)
    }

    pub fn is_checked(&self) -> bool {
        self.checked_at > 0
    }
}

pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn state_path() -> Option<std::path::PathBuf> {
    let config = crate::theme::config_path()?;
    Some(config.with_file_name("update.json"))
}

pub fn load() -> UpdateState {
    state_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(state: &UpdateState) {
    let Some(path) = state_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(state) {
        let _ = crate::config::write_atomic(&path, &text);
    }
}

pub fn check_due() -> bool {
    if std::env::var("PI_DESKTOP_NO_UPDATE").is_ok() {
        return false;
    }
    let state = load();
    !state.is_checked() || now().saturating_sub(state.checked_at) >= CHECK_INTERVAL_SECS
}

pub fn spawn_check(tx: async_channel::Sender<UpdateState>) {
    std::thread::Builder::new()
        .name("pi-update-check".into())
        .spawn(move || {
            let state = check_blocking();
            save(&state);
            let _ = tx.send_blocking(state);
        })
        .ok();
}

fn check_blocking() -> UpdateState {
    let current = current_version();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let output = Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "10",
            "-H",
            "User-Agent: FrameT-Pi-Desktop",
            &url,
        ])
        .stdin(Stdio::null())
        .output();
    let mut state = UpdateState {
        checked_at: now(),
        current: current.clone(),
        ..UpdateState::default()
    };
    let output = match output {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            state.error = format!("GitHub answered {}", output.status);
            return state;
        }
        Err(error) => {
            state.error = format!("curl: {error}");
            return state;
        }
    };
    let value: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(value) => value,
        Err(error) => {
            state.error = format!("invalid GitHub response: {error}");
            return state;
        }
    };
    let Some(tag) = value.get("tag_name").and_then(|value| value.as_str()) else {
        let message = value
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("release without tag");
        state.error = if message.eq_ignore_ascii_case("not found") {
            "no public release yet".to_string()
        } else {
            message.to_string()
        };
        return state;
    };
    state.latest = tag.trim_start_matches('v').to_string();
    state.url = value
        .get("html_url")
        .and_then(|value| value.as_str())
        .unwrap_or(RELEASES_URL)
        .to_string();
    state.notes = value
        .get("body")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    state
}

pub fn version_gt(left: &str, right: &str) -> bool {
    let parse = |value: &str| {
        value
            .trim()
            .trim_start_matches('v')
            .split(['.', '-', '+'])
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let left = parse(left);
    let right = parse(right);
    let len = left.len().max(right.len());
    for index in 0..len {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }
    false
}

pub fn open(url: &str) {
    let _ = Command::new("xdg-open").arg(url).stdin(Stdio::null()).spawn();
}


