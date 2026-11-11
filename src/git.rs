
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitFile {
    pub path: String,
    pub old_path: Option<String>,
    pub index: char,
    pub worktree: char,
}

impl GitFile {
    pub fn untracked(&self) -> bool {
        self.index == '?' || self.worktree == '?'
    }

    pub fn staged(&self) -> bool {
        !self.untracked() && self.index != ' '
    }

    pub fn unstaged(&self) -> bool {
        !self.untracked() && self.worktree != ' '
    }

    pub fn label(&self) -> &'static str {
        if self.untracked() {
            return "new";
        }
        if self.index == 'D' || self.worktree == 'D' {
            return "removed";
        }
        if self.index == 'R' {
            return "renamed";
        }
        if self.staged() && self.unstaged() {
            return "partial";
        }
        if self.staged() {
            return "staged";
        }
        "modificado"
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitStatus {
    pub branch: String,
    pub upstream: String,
    pub ahead: u32,
    pub behind: u32,
    pub detached: bool,
    pub files: Vec<GitFile>,
}

impl GitStatus {
    pub fn clean(&self) -> bool {
        self.files.is_empty()
    }

    pub fn staged_count(&self) -> usize {
        self.files.iter().filter(|file| file.staged()).count()
    }

    pub fn changed_count(&self) -> usize {
        self.files.len()
    }
}

pub fn parse_status(text: &str) -> GitStatus {
    let mut status = GitStatus::default();
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            parse_header(header, &mut status);
            continue;
        }
        if line.len() < 4 {
            continue;
        }
        let mut chars = line.chars();
        let index = chars.next().unwrap_or(' ');
        let worktree = chars.next().unwrap_or(' ');
        if index == '!' && worktree == '!' {
            continue;
        }
        let raw_path = line.chars().skip(3).collect::<String>();
        let (old_path, path) = match raw_path.split_once(" -> ") {
            Some((old, new)) => (Some(unquote_path(old)), unquote_path(new)),
            None => (None, unquote_path(&raw_path)),
        };
        status.files.push(GitFile {
            path,
            old_path,
            index,
            worktree,
        });
    }
    if status.branch.is_empty() {
        status.branch = if status.detached {
            "detached".to_string()
        } else {
            "?".to_string()
        };
    }
    status
}

fn unquote_path(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() < 2 || !raw.starts_with('"') || !raw.ends_with('"') {
        return raw.to_string();
    }
    let mut bytes = Vec::new();
    let inner = &raw[1..raw.len() - 1];
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
            continue;
        }
        match chars.next() {
            Some('n') => bytes.push(b'\n'),
            Some('t') => bytes.push(b'\t'),
            Some('r') => bytes.push(b'\r'),
            Some('"') => bytes.push(b'"'),
            Some('\\') => bytes.push(b'\\'),
            Some(first @ '0'..='7') => {
                let mut value = first.to_digit(8).unwrap_or(0) as u8;
                for _ in 0..2 {
                    match chars.peek().and_then(|next| next.to_digit(8)) {
                        Some(digit) => {
                            chars.next();
                            value = value.wrapping_mul(8).wrapping_add(digit as u8);
                        }
                        None => break,
                    }
                }
                bytes.push(value);
            }
            Some(other) => {
                let mut buffer = [0u8; 4];
                bytes.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
            }
            None => break,
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn parse_header(header: &str, status: &mut GitStatus) {
    let header = header.trim();
    if header.starts_with("Don't commit yet on") {
        status.branch = header
            .trim_start_matches("Don't commit yet on")
            .trim()
            .to_string();
        return;
    }
    if header.starts_with("HEAD (not branch)") {
        status.detached = true;
        status.branch = "detached".to_string();
        return;
    }
    let (left, rest) = match header.split_once("...") {
        Some((left, rest)) => (left, Some(rest)),
        None => (header, None),
    };
    status.branch = left.split_whitespace().next().unwrap_or("?").to_string();
    let Some(rest) = rest else {
        return;
    };
    let rest = rest.trim();
    if let Some((upstream, tail)) = rest.split_once(' ') {
        status.upstream = upstream.to_string();
        let tail = tail.trim_matches(['[', ']']);
        for part in tail.split(',') {
            let part = part.trim();
            if let Some(value) = part.strip_prefix("ahead ") {
                status.ahead = value.parse().unwrap_or(0);
            } else if let Some(value) = part.strip_prefix("behind ") {
                status.behind = value.parse().unwrap_or(0);
            }
        }
    } else {
        status.upstream = rest.to_string();
    }
}


pub fn status_args() -> Vec<&'static str> {
    vec!["status", "--porcelain=v1", "-b"]
}

pub fn diff_args(file: &GitFile, staged: bool) -> Vec<String> {
    let mut args = vec!["diff".to_string(), "--no-ext-diff".to_string()];
    if staged {
        args.push("--cached".to_string());
    }
    args.push("--".to_string());
    args.push(file.path.clone());
    args
}

pub fn add_all_args() -> Vec<String> {
    vec!["add".to_string(), "-A".to_string()]
}

pub fn add_args(file: &GitFile) -> Vec<String> {
    vec!["add".to_string(), "--".to_string(), file.path.clone()]
}

pub fn unstage_args(file: &GitFile) -> Vec<String> {
    vec![
        "restore".to_string(),
        "--staged".to_string(),
        "--".to_string(),
        file.path.clone(),
    ]
}

pub fn discard_args(file: &GitFile) -> Vec<String> {
    if file.untracked() {
        vec![
            "clean".to_string(),
            "-f".to_string(),
            "-d".to_string(),
            "--".to_string(),
            file.path.clone(),
        ]
    } else {
        vec![
            "restore".to_string(),
            "--worktree".to_string(),
            "--".to_string(),
            file.path.clone(),
        ]
    }
}

pub fn commit_args(message: &str) -> Vec<String> {
    vec!["commit".to_string(), "-m".to_string(), message.trim().to_string()]
}

pub fn push_args() -> Vec<String> {
    vec!["push".to_string()]
}

pub fn run(repo: &Path, args: &[String]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        format!("git {} went out with {}", args.join(" "), output.status)
    } else {
        stderr
    })
}


