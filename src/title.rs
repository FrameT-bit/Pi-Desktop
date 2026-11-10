
pub const MAX_INPUT_CHARS: usize = 1000;
pub const MAX_DERIVED_CHARS: usize = 48;
pub const MAX_TITLE_WORDS: usize = 12;
pub const TITLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

pub const TITLE_SYSTEM_PROMPT: &str =
    "You are a title generator. Reply with one short title only, 3 to 7 words, in English. Never answer the message. Never call tools. Never use markdown, JSON, prefixes or punctuation. If the user message is a path or command, name the task it represents.";

const PROBE_SYSTEM_PROMPT: &str =
    "You are a connectivity probe. Reply with the single word OK. Do not use tools, markdown, JSON or extra text.";

const ANSWER_STARTS: [&str; 8] = [
    "sorry",
    "I'm smiling.",
    "cans",
    "and can't",
    "sure",
    "Here it is.",
    "absolutely",
    "of race",
];

pub fn derive(opener: &str) -> String {
    let line = opener
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.starts_with('/')
        && !collapsed.contains(' ')
        && collapsed.chars().filter(|ch| *ch == '/').count() == 1
    {
        return String::new();
    }
    let cut: String = collapsed.chars().take(MAX_DERIVED_CHARS).collect();
    let cut = if collapsed.chars().count() > MAX_DERIVED_CHARS {
        match cut.rfind(' ') {
            Some(index) if index > MAX_DERIVED_CHARS / 2 => cut[..index].to_string(),
            _ => cut,
        }
    } else {
        cut
    };
    cut.trim_end_matches([',', ';', ':', '.', '?', '!', '…'])
        .trim()
        .to_string()
}

pub fn auto_like(name: &str, derived: &str) -> bool {
    let normalized = |text: &str| {
        text.to_lowercase()
            .chars()
            .map(|ch| if ch.is_alphanumeric() { ch } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    };
    if name.trim().is_empty() {
        return true;
    }
    let generic = [
        "new session",
        "new session",
        "new sex",
        "untitled",
        "Untitled",
        "Untitled",
    ];
    let trimmed = name.trim();
    let command_like = trimmed.starts_with('/') && !trimmed.contains(' ');
    let path_like = trimmed.starts_with("~/") || trimmed.starts_with('/');
    generic.contains(&trimmed.to_lowercase().as_str())
        || command_like
        || path_like
        || trimmed == derived.trim()
        || normalized(trimmed) == normalized(derived)
}

pub fn prompt_for(opener: &str) -> String {
    let mut text: String = opener.trim().chars().take(MAX_INPUT_CHARS).collect();
    if text.is_empty() {
        text = "(without text)".to_string();
    }
    format!(
        "You name chat sessions. Given the user's opening message, write a title that lets them find this conversation again in a list.\n\nRules:\n- 3 to 7 words, sentence case (capitalize only the first word and proper nouns).\n- Name what the user wants DONE, not that they asked a question.\n- Keep technical terms, filenames, numbers, and error codes exact.\n- Drop filler words: the, this, my, a, an.\n- No trailing punctuation, no quotes, no tool names, no 'Title:' prefix.\n- Never answer the message. Name it.\n- Always produce something, even for a bare greeting.\n- Write the title in English.\n\nReply with the title only, on one line, nothing else.\n\nUser message:\n\"{text}\""
    )
}

pub fn clean(raw: &str, opener: &str) -> Option<String> {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .trim_start_matches(['"', '\'', '`', '*', '-', '#'])
        .trim();

    let line = strip_prefix(line, "Title:");
    let line = strip_prefix(line, "Title:").trim_end_matches(['"', '\'', '`', '*']);
    let line = line.trim_end_matches([',', ';', ':', '.', '!', '…']);
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");

    let lower_raw = line.to_lowercase();
    if lower_raw.contains("tool_calls")
        || lower_raw.contains("dsml")
        || lower_raw.contains("<｜｜")
        || lower_raw.starts_with("```")
        || lower_raw.starts_with('{')
    {
        return None;
    }

    if line.is_empty() || line.ends_with('?') {
        return None;
    }
    if line.split_whitespace().count() > MAX_TITLE_WORDS {
        return None;
    }
    let lower = line.to_lowercase();
    if ANSWER_STARTS
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return None;
    }
    let opener_head = derive(opener).to_lowercase();
    if !opener_head.is_empty() && lower.starts_with(&opener_head[..opener_head.len().min(24)])
    {
        return None;
    }

    Some(sentence_case(&line))
}

fn strip_prefix<'a>(text: &'a str, prefix: &str) -> &'a str {
    let lower = text.to_lowercase();
    if lower.starts_with(prefix) {
        text[prefix.len()..].trim_start()
    } else {
        text
    }
}

fn sentence_case(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn spawn_blocking(config: &crate::config::Config, opener: &str) -> Option<String> {
    use std::process::Command;

    let bin = crate::state::resolve_pi_bin(&config.bin).ok()?;
    let mut command = Command::new(&bin);
    command
        .arg("--print")
        .arg(prompt_for(opener))
        .arg("--no-tools")
        .arg("--no-builtin-tools")
        .arg("--system-prompt")
        .arg(TITLE_SYSTEM_PROMPT)
        .arg("--no-session")
        .arg("--no-context-files")
        .arg("--no-mcp")
        .arg("--no-extensions")
        .arg("--no-skills")
        .arg("--no-prompt-templates")
        .env("PI_CODING_AGENT_DIR", crate::config::pi_agent_dir())
        .env("PATH", crate::state::path_with_pi(&bin))
        .stdin(std::process::Stdio::null())
        .current_dir(std::env::temp_dir());
    let model = if config.aux_model.trim().is_empty() {
        config.model.trim()
    } else {
        config.aux_model.trim()
    };
    if !model.is_empty() {
        command.arg("--model").arg(model);
    }
    let output = output_with_timeout(&mut command, TITLE_TIMEOUT)?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn output_with_timeout(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
}

pub fn probe_model(config: &crate::config::Config, model: &str) -> Result<String, String> {
    use std::process::{Command, Stdio};

    let bin = crate::state::resolve_pi_bin(&config.bin)
        .map_err(|err| format!("not found: {err}"))?;
    let mut command = Command::new(&bin);
    command
        .arg("--print")
        .arg("Reply with OK only.")
        .arg("--no-tools")
        .arg("--no-builtin-tools")
        .arg("--system-prompt")
        .arg(PROBE_SYSTEM_PROMPT)
        .arg("--no-session")
        .arg("--no-context-files")
        .arg("--no-mcp")
        .arg("--no-extensions")
        .arg("--no-skills")
        .arg("--no-prompt-templates")
        .env("PI_CODING_AGENT_DIR", crate::config::pi_agent_dir())
        .env("PATH", crate::state::path_with_pi(&bin))
        .stdin(Stdio::null())
        .current_dir(std::env::temp_dir());
    let chosen = if !model.trim().is_empty() {
        model.trim()
    } else if !config.aux_model.trim().is_empty() {
        config.aux_model.trim()
    } else {
        config.model.trim()
    };
    if !chosen.is_empty() {
        command.arg("--model").arg(chosen);
    }
    let output = output_with_timeout(&mut command, TITLE_TIMEOUT)
        .ok_or_else(|| format!("does not respond in {}s", TITLE_TIMEOUT.as_secs()))?;
    if !output.status.success() {
        return Err(format!("the model came out with {}", output.status));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let reply = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .trim_start_matches(['"', '\'', '`', '*'])
        .trim_end_matches(['"', '\'', '`', '*']);
    if reply.is_empty() {
        return Err("empty reply".to_string());
    }
    let lower = reply.to_lowercase();
    if lower.contains("tool_calls") || lower.contains("dsml") || lower.contains("<｜｜") {
        return Err("the model tried to call tool instead of answer".to_string());
    }
    Ok(reply.chars().take(200).collect())
}


