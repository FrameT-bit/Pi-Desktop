
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;

use serde_json::Value;

use crate::config;
use crate::state::{path_with_pi, PluginItem, Transport};

pub const SETTINGS_CMD: &str = "cat ~/.pi/agent/settings.json";

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

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}


