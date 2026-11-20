
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
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub appimage: Option<String>,
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
    let mut state: UpdateState = state_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    // A cache made by an older installed build must not offer that same
    // release again after upgrading.
    if state.current != current_version() {
        state.checked_at = 0;
    }
    state.current = current_version();
    state
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
    // /releases/latest excludes prereleases and returns 404 for this project
    // when it only has preview builds. Use the public release list instead.
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=100");
    let output = Command::new("curl")
        .args([
            "-sS",
            "--fail-with-body",
            "--max-filesize",
            "4000000",
            "--max-time",
            "10",
            "-H",
            "User-Agent: FrameT-Pi-Desktop",
            "-H",
            "Accept: application/vnd.github+json",
            &url,
        ])
        .stdin(Stdio::null())
        .output();
    let mut state = UpdateState {
        checked_at: now(),
        current,
        ..UpdateState::default()
    };
    let output = match output {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            state.error = serde_json::from_slice::<serde_json::Value>(&output.stdout)
                .ok().and_then(|value| value.get("message")?.as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("GitHub request failed ({})", output.status));
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
    if let Err(error) = apply_releases(&mut state, &value) { state.error = error.into(); }
    state
}

fn apply_releases(state: &mut UpdateState, value: &serde_json::Value) -> Result<(), &'static str> {
    let releases = value.as_array().ok_or("GitHub did not return a release list.")?;
    let Some((_, tag, release)) = releases.iter().filter_map(|release| {
        if release.get("draft").and_then(|draft| draft.as_bool()) != Some(false) { return None; }
        let tag = release.get("tag_name")?.as_str()?;
        Some((version(tag)?, tag, release))
    }).max_by_key(|(version, _, release)| (*version, release["prerelease"].as_bool() == Some(false))) else {
        return Ok(());
    };
    state.latest = tag.strip_prefix('v').unwrap_or(tag).to_owned();
    state.url = format!("{RELEASES_URL}/tag/{tag}");
    state.prerelease = release["prerelease"].as_bool().unwrap_or(false);
    state.notes = release["body"].as_str().unwrap_or("").to_string();
    let expected = format!("Pi-Desktop-{}-{}.AppImage", state.latest, std::env::consts::ARCH);
    if release["assets"].as_array().is_some_and(|assets| assets.iter().any(|asset|
        asset["name"].as_str() == Some(expected.as_str()) && asset["state"].as_str() == Some("uploaded")
    )) {
        state.appimage = Some(format!("{RELEASES_URL}/download/{tag}/{expected}"));
    }
    Ok(())
}

fn version(tag: &str) -> Option<[u64; 3]> {
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    if !tag.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.') { return None; }
    let mut parts = tag.split('.');
    let version = [parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?];
    parts.next().is_none().then_some(version)
}

pub fn version_gt(left: &str, right: &str) -> bool {
    matches!((version(left), version(right)), (Some(left), Some(right)) if left > right)
}

pub fn open(url: &str) {
    let _ = gtk::gio::AppInfo::launch_default_for_uri(url, gtk::gio::AppLaunchContext::NONE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_highest_public_version_even_when_only_prereleases_exist() {
        let name = format!("Pi-Desktop-1.10.0-{}.AppImage", std::env::consts::ARCH);
        let mut state = UpdateState { current: "1.9.0".into(), ..Default::default() };
        apply_releases(&mut state, &json!([
            {"tag_name":"v2.0.0","draft":true},
            {"tag_name":"v1.9.8","draft":false,"prerelease":true},
            {"tag_name":"v1.10.0","draft":false,"prerelease":true,"assets":[{"name":name,"state":"uploaded"}]},
            {"tag_name":"2.0.0/other","draft":false},
        ])).unwrap();
        assert!(state.has_update() && state.prerelease);
        assert_eq!(state.latest, "1.10.0");
        assert_eq!(state.url, format!("{RELEASES_URL}/tag/v1.10.0"));
        assert!(state.appimage.unwrap().ends_with(&name));
    }

    #[test]
    #[ignore = "requests the public GitHub API"]
    fn official_repository_returns_published_release() {
        let state = check_blocking();
        assert!(state.error.is_empty(), "{}", state.error);
        assert!(!state.latest.is_empty());
        assert!(state.url.starts_with(RELEASES_URL));
        eprintln!("Latest public release: v{} (prerelease: {})", state.latest, state.prerelease);
    }
}

