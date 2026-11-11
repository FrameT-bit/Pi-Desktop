
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;

use serde_json::Value;

use crate::config;
use crate::state::{path_with_pi, PluginItem, Transport};

pub const SETTINGS_CMD: &str = "cat ~/.pi/agent/settings.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginSettings {
    Tab(&'static str),
    Json {
        title: &'static str,
        path: &'static str,
    },
    Text {
        title: &'static str,
        path: &'static str,
    },
}

pub fn settings_for(spec: &str) -> Option<PluginSettings> {
    let slug = plugin_slug(spec);
    match slug.as_str() {
        "pi-web-access" => Some(PluginSettings::Tab("CONNECTIONS")),
        "pi-personas" => Some(PluginSettings::Tab("SOUL")),
        "pi-memory" => Some(PluginSettings::Tab("MEMORY")),
        "pi-mcp-adapter" => Some(PluginSettings::Tab("MCP")),
        "pi-guard" => Some(PluginSettings::Json {
            title: "pi-guard",
            path: "~/.pi/agent/guard.json",
        }),
        "pi-subagents" => Some(PluginSettings::Json {
            title: "pi-subagents",
            path: "~/.pi/agent/extensions/subagent/config.json",
        }),
        "openwiki" => Some(PluginSettings::Text {
            title: "openwiki",
            path: "~/.openwiki/.env",
        }),
        _ => None,
    }
}

fn plugin_slug(spec: &str) -> String {
    let mut value = spec.trim();
    for prefix in ["npm:", "git:"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest;
        }
    }
    let value = value.trim_end_matches('/');
    let tail = value.rsplit('/').next().unwrap_or(value);
    let tail = tail
        .split(|ch| ch == '@' || ch == '#')
        .next()
        .unwrap_or(tail);
    tail.to_ascii_lowercase()
}

pub fn read_config(transport: &Transport, path: &str) -> Result<String, String> {
    match transport {
        Transport::Local(_) => {
            let file = config::expand_home(path);
            match std::fs::read_to_string(&file) {
                Ok(text) => Ok(text),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
                Err(error) => Err(format!("{}: {error}", file.display())),
            }
        }
        Transport::Remote { target, .. } => {
            let quoted = remote_path(path);
            let output = ssh(
                target,
                &format!("if [ -f {quoted} ]; then cat {quoted}; fi"),
            )?;
            if !output.status.success() {
                return Err(command_error(&output));
            }
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Transport::Missing { expected } => Err(format!("not found: {expected}")),
    }
}

pub fn write_config(transport: &Transport, path: &str, text: &str) -> Result<(), String> {
    match transport {
        Transport::Local(_) => {
            let file = config::expand_home(path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            std::fs::write(&file, text).map_err(|error| format!("{}: {error}", file.display()))
        }
        Transport::Remote { target, .. } => {
            let quoted = remote_path(path);
            let dir = path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or(".");
            let command = format!("mkdir -p {} && cat > {quoted}", remote_path(dir));
            let output = ssh_write(target, &command, text)?;
            if output.status.success() {
                Ok(())
            } else {
                Err(command_error(&output))
            }
        }
        Transport::Missing { expected } => Err(format!("not found: {expected}")),
    }
}

#[derive(Debug)]
pub enum PluginEvent {
    List(Result<Vec<PluginItem>, String>),
    Installed(Result<(), String>),
    Removed(Result<(), String>),
}

pub fn is_valid_spec(spec: &str) -> bool {
    !spec.is_empty()
        && spec.len() <= 200
        && spec
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._@/-:+".contains(c))
}

pub fn normalize_spec(input: &str) -> Option<String> {
    let spec = input.trim();
    if spec.is_empty() {
        return None;
    }
    let spec = if spec.starts_with("npm:") || spec.starts_with("git:") {
        spec.to_string()
    } else {
        format!("npm:{spec}")
    };
    is_valid_spec(&spec).then_some(spec)
}

pub(crate) fn ssh(target: &str, command: &str) -> Result<Output, String> {
    Command::new("ssh")
        .arg("-T")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=8")
        .arg(target)
        .arg(command)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("ssh: {err}"))
}

fn ssh_write(target: &str, command: &str, text: &str) -> Result<Output, String> {
    use std::io::Write;
    let mut child = Command::new("ssh")
        .arg("-T")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=8")
        .arg(target)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("ssh: {err}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|err| format!("ssh: {err}"))?;
    }
    child
        .wait_with_output()
        .map_err(|err| format!("ssh: {err}"))
}

pub(crate) fn run_pi(transport: &Transport, args: &[String]) -> Result<Output, String> {
    match transport {
        Transport::Local(local) => {
            let Some(bin) = local.bin.as_deref() else {
                return Err("not found below".to_string());
            };
            Command::new(bin)
                .args(args)
                .env("PATH", path_with_pi(bin))
                .stdin(Stdio::null())
                .output()
                .map_err(|err| format!("pi: {err}"))
        }
        Transport::Remote { target, .. } => {
            let quoted: Vec<String> = args.iter().map(|arg| shell_quote(arg)).collect();
            let command = format!(
                "export PATH=$HOME/.local/bin:$HOME/.local/bin:$PATH; pi {}",
                quoted.join(" ")
            );
            ssh(target, &command)
        }
        Transport::Missing { expected } => Err(format!("not found: {expected}")),
    }
}

fn settings_path() -> PathBuf {
    config::home_file(".pi/agent/settings.json")
}

fn parse_settings(text: &str) -> Result<Vec<PluginItem>, String> {
    let parsed: Value = serde_json::from_str(text).map_err(|err| format!("settings.json: {err}"))?;
    let packages = parsed
        .get("packages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for package in packages {
        let Some(spec) = package.as_str() else { continue };
        let spec = spec.trim();
        if spec.is_empty() {
            continue;
        }
        let origin = if spec.starts_with("npm:") {
            "npm"
        } else if spec.starts_with("git:") {
            "git"
        } else {
            "local"
        }
        .to_string();
        items.push(PluginItem {
            spec: spec.to_string(),
            origin,
        });
    }
    Ok(items)
}

fn fetch_blocking(transport: &Transport) -> Result<Vec<PluginItem>, String> {
    let text = match transport {
        Transport::Local(_) => {
            let path = settings_path();
            match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(err) => {
                    return Err(format!("{}: {err}", path.display()));
                }
            }
        }
        Transport::Remote { target, .. } => {
            let output = ssh(target, SETTINGS_CMD)?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "ssh: {}",
                    if stderr.trim().is_empty() {
                        format!("output {}", output.status)
                    } else {
                        stderr.trim().to_string()
                    }
                ));
            }
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        Transport::Missing { expected } => {
            return Err(format!("not found: {expected}"));
        }
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    parse_settings(&text)
}

pub(crate) fn command_error(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    if detail.is_empty() {
        format!("command failed ({})", output.status)
    } else {
        detail.lines().last().unwrap_or(detail).to_string()
    }
}

pub fn spawn_fetch(transport: Transport, tx: async_channel::Sender<PluginEvent>) {
    thread::Builder::new()
        .name("pi-plugins-list".into())
        .spawn(move || {
            let _ = tx.send_blocking(PluginEvent::List(fetch_blocking(&transport)));
        })
        .ok();
}

pub fn spawn_install(
    transport: Transport,
    spec: String,
    tx: async_channel::Sender<PluginEvent>,
) {
    thread::Builder::new()
        .name("pi-plugins-install".into())
        .spawn(move || {
            if !is_valid_spec(&spec) {
                let _ = tx.send_blocking(PluginEvent::Installed(Err(
                    "invalid package name".to_string(),
                )));
                return;
            }
            let args = vec!["install".to_string(), spec];
            let result = match run_pi(&transport, &args) {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => Err(command_error(&output)),
                Err(err) => Err(err),
            };
            if result.is_ok() {
                let _ = tx.send_blocking(PluginEvent::List(fetch_blocking(&transport)));
            }
            let _ = tx.send_blocking(PluginEvent::Installed(result));
        })
        .ok();
}

pub fn spawn_remove(
    transport: Transport,
    spec: String,
    tx: async_channel::Sender<PluginEvent>,
) {
    thread::Builder::new()
        .name("pi-plugins-remove".into())
        .spawn(move || {
            if !is_valid_spec(&spec) {
                let _ = tx.send_blocking(PluginEvent::Removed(Err(
                    "invalid package name".to_string(),
                )));
                return;
            }
            let args = vec!["remove".to_string(), spec];
            let result = match run_pi(&transport, &args) {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => Err(command_error(&output)),
                Err(err) => Err(err),
            };
            if result.is_ok() {
                let _ = tx.send_blocking(PluginEvent::List(fetch_blocking(&transport)));
            }
            let _ = tx.send_blocking(PluginEvent::Removed(result));
        })
        .ok();
}

fn remote_path(value: &str) -> String {
    match value.strip_prefix("~/") {
        Some(rest) => format!("$HOME/{}", shell_quote(rest)),
        None => shell_quote(value),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_targets_are_available_for_known_packages() {
        assert_eq!(
            settings_for("npm:pi-web-access@0.47.0"),
            Some(PluginSettings::Tab("CONNECTIONS"))
        );
        assert_eq!(
            settings_for("../packages/pi-memory"),
            Some(PluginSettings::Tab("MEMORY"))
        );
        assert_eq!(
            settings_for("npm:pi-subagents"),
            Some(PluginSettings::Json {
                title: "pi-subagents",
                path: "~/.pi/agent/extensions/subagent/config.json",
            })
        );
        assert_eq!(
            settings_for("npm:openwiki"),
            Some(PluginSettings::Text {
                title: "openwiki",
                path: "~/.openwiki/.env",
            })
        );
        assert_eq!(settings_for("npm:pi-checkpoints"), None);
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use crate::state::LocalTransport;

    #[test]
    fn local_config_round_trip() {
        let transport = Transport::Local(LocalTransport {
            bin: None,
            workspace: std::path::PathBuf::from("."),
            args: Vec::new(),
        });
        let path = std::env::temp_dir().join("pi-desktop-plugin-config-test.json");
        let path_text = path.to_string_lossy().to_string();
        write_config(&transport, &path_text, "{\n  \"ok\": true\n}\n").unwrap();
        let text = read_config(&transport, &path_text).unwrap();
        assert!(text.contains("\"ok\": true"));
        let _ = std::fs::remove_file(&path);
    }
}
