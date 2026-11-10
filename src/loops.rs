
use std::path::PathBuf;

use serde_json::{json, Value};

pub const DEFAULT_INTERVAL: u64 = 300;
pub const COMPLETE_MARKER: &str = "LOOP_COMPLETE";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopState {
    pub task: String,
    pub interval: u64,
    pub iteration: u64,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopCommand {
    Start { interval: u64, task: String },
    Stop,
    Status,
    Invalid(&'static str),
}

pub fn parse_command(argument: &str) -> LoopCommand {
    let text = argument.trim();
    if text.is_empty() {
        return LoopCommand::Status;
    }
    let lowered = text.to_ascii_lowercase();
    if lowered == "stop" || lowered == "off" {
        return LoopCommand::Stop;
    }

    let mut parts = text.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    if let Some(parsed) = parse_interval(first) {
        let Ok(interval) = parsed else {
            return LoopCommand::Invalid("Invalid interval");
        };
        if interval == 0 {
            return LoopCommand::Invalid("interval needs to be greater than zero");
        }
        if rest.is_empty() {
            return LoopCommand::Invalid("inform the task after the interval");
        }
        return LoopCommand::Start {
            interval,
            task: rest.to_string(),
        };
    }

    LoopCommand::Start {
        interval: DEFAULT_INTERVAL,
        task: text.to_string(),
    }
}

fn parse_interval(token: &str) -> Option<Result<u64, ()>> {
    if token.len() < 2 {
        return None;
    }
    let mut chars = token.chars();
    let unit = chars.next_back()?;
    let digits: String = chars.collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let multiplier = match unit {
        's' => 1,
        'm' => 60,
        'h' => 3_600,
        _ => return None,
    };
    let Ok(value) = digits.parse::<u64>() else {
        return Some(Err(()));
    };
    value.checked_mul(multiplier).map(Ok).or(Some(Err(())))
}

pub fn wakeup_text(task: &str, iteration: u64) -> String {
    format!(
        "[/loop wakeup #{iteration}, self-paced]\n\
Recurring task: {task}\n\
\n\
This is an automatic wakeup from the /loop the user set. Perform the task now against the CURRENT state (re-check files, processes, or services fresh — do not assume anything from earlier iterations still holds). Report concisely what you found or did this iteration.\n\
If the task is now complete, no longer applicable, or the thing you were watching has finished, say so and end your reply with LOOP_COMPLETE on its own line — that stops the loop."
    )
}

pub fn has_loop_complete(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.ends_with(COMPLETE_MARKER) {
        return true;
    }
    trimmed
        .lines()
        .any(|line| line.trim() == COMPLETE_MARKER)
}

pub fn format_interval(secs: u64) -> String {
    if secs == 0 {
        return "0s".to_string();
    }
    if secs % 3_600 == 0 {
        return format!("{}h", secs / 3_600);
    }
    if secs >= 60 {
        let minutes = secs / 60;
        let rest = secs % 60;
        if rest == 0 {
            format!("{minutes}m")
        } else {
            format!("{minutes}m{rest}s")
        }
    } else {
        format!("{secs}s")
    }
}

pub fn format_countdown(secs: u64) -> String {
    if secs >= 3_600 {
        format!(
            "{:02}:{:02}:{:02}",
            secs / 3_600,
            (secs % 3_600) / 60,
            secs % 60
        )
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

pub fn loops_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("pi-desktop").join("loops.json"))
}

pub fn load() -> Option<LoopState> {
    let path = loops_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    from_json(&value)
}

fn from_json(value: &Value) -> Option<LoopState> {
    let task = value.get("task").and_then(Value::as_str)?.trim();
    if task.is_empty() {
        return None;
    }
    let interval = value.get("interval").and_then(Value::as_u64)?;
    if interval == 0 {
        return None;
    }
    Some(LoopState {
        task: task.to_string(),
        interval,
        iteration: value.get("iteration").and_then(Value::as_u64).unwrap_or(0),
        active: value.get("active").and_then(Value::as_bool).unwrap_or(false),
    })
}

pub fn save(state: &LoopState) {
    let Some(path) = loops_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = json!({
        "task": state.task,
        "interval": state.interval,
        "iteration": state.iteration,
        "active": state.active,
    });
    if let Ok(text) = serde_json::to_string_pretty(&body) {
        let _ = std::fs::write(path, format!("{text}\n"));
    }
}

pub fn clear() {
    if let Some(path) = loops_path() {
        let _ = std::fs::remove_file(path);
    }
}


