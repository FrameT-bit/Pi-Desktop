
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitFile {
    pub path: String,
    pub old_path: Option<String>,
    pub index: char,
    pub worktree: char,
}

impl GitFile {
    pub fn conflicted(&self) -> bool {
        matches!((self.index, self.worktree),
            ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U'))
    }
    pub fn untracked(&self) -> bool {
        self.index == '?' || self.worktree == '?'
    }

    pub fn staged(&self) -> bool {
        !self.untracked() && !self.conflicted() && self.index != ' '
    }

    pub fn unstaged(&self) -> bool {
        !self.untracked() && self.worktree != ' '
    }

    pub fn label(&self) -> &'static str {
        if self.conflicted() { return "conflict"; }
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
        "changed"
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
    if text.contains('\0') { return parse_status_z(text); }
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
        let rename = (index == 'R' || index == 'C' || worktree == 'R' || worktree == 'C')
            .then(|| rename_separator(&raw_path)).flatten();
        let (old_path, path) = match rename {
            Some(at) => (Some(unquote_path(&raw_path[..at])), unquote_path(&raw_path[at + 4..])),
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
    for prefix in ["No commits yet on ", "Initial commit on ", "Don't commit yet on "] {
        if let Some(branch) = header.strip_prefix(prefix) {
            status.branch = branch.trim().to_string();
            return;
        }
    }
    if header.starts_with("HEAD (not branch)") || header.starts_with("HEAD (no branch)") {
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
    vec!["status", "--porcelain=v1", "-b", "-z", "--untracked-files=all"]
}

pub fn diff_args(file: &GitFile, staged: bool) -> Vec<String> {
    let mut args = vec!["diff".to_string(), "--no-ext-diff".to_string(), "--no-color".to_string()];
    if file.conflicted() { args.push("--ours".into()); }
    if file.untracked() {
        args.extend(["--no-index".into(), "--".into(), "/dev/null".into(), file.path.clone()]);
        return args;
    }
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

pub fn stage_files(repo: &Path, files: &[GitFile]) -> Result<String, String> {
    if files.is_empty() { return Err("No files selected.".into()); }
    let mut args = vec!["add".into(), "--".into()];
    args.extend(files.iter().map(|file| file.path.clone()));
    run(repo, &args)
}

pub fn unstage_files(repo: &Path, files: &[GitFile]) -> Result<String, String> {
    if files.is_empty() { return Err("No files selected.".into()); }
    let has_head = run(repo, &["rev-parse".into(), "--verify".into(), "HEAD".into()]).is_ok();
    let mut args: Vec<String> = if has_head { vec!["restore".into(), "--staged".into(), "--".into()] }
        else { vec!["rm".into(), "--cached".into(), "-r".into(), "-f".into(), "--".into()] };
    let mut paths = std::collections::BTreeSet::new();
    for file in files {
        paths.insert(file.path.clone());
        if let Some(old) = &file.old_path { paths.insert(old.clone()); }
    }
    args.extend(paths);
    run(repo, &args)
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
        .arg("--literal-pathspecs")
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() || (args.iter().any(|arg| arg == "--no-index") && output.status.code() == Some(1)) {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        format!("git {} went out with {}", args.join(" "), output.status)
    } else {
        stderr
    })
}

pub fn repository_root(folder: &Path) -> Result<std::path::PathBuf, String> {
    if !folder.is_dir() { return Err("Choose an existing local repository folder.".into()); }
    let inside = run(folder, &["rev-parse".into(), "--is-inside-work-tree".into()])?;
    if inside.trim() != "true" {
        return Err("Choose a working-tree repository, not a bare Git directory.".into());
    }
    let path = run(folder, &["rev-parse".into(), "--show-toplevel".into()])?;
    let path = path.strip_suffix('\n').unwrap_or(&path);
    if path.is_empty() { return Err("Git did not report a working-tree folder.".into()); }
    Ok(std::path::PathBuf::from(path))
}

fn rename_separator(raw: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;
    for (at, ch) in raw.char_indices() {
        if escaped { escaped = false; continue; }
        if ch == '\\' && quoted { escaped = true; continue; }
        if ch == '"' { quoted = !quoted; }
        if !quoted && raw[at..].starts_with(" -> ") { return Some(at); }
    }
    None
}

fn parse_status_z(text: &str) -> GitStatus {
    let mut status = GitStatus::default();
    let mut fields = text.split('\0');
    while let Some(field) = fields.next() {
        if let Some(header) = field.strip_prefix("## ") {
            parse_header(header, &mut status);
        } else if field.len() >= 4 {
            let index = field.as_bytes()[0] as char;
            let worktree = field.as_bytes()[1] as char;
            let old_path = if matches!(index, 'R' | 'C') || matches!(worktree, 'R' | 'C') {
                fields.next().map(str::to_string)
            } else { None };
            status.files.push(GitFile { path: field[3..].to_string(), old_path, index, worktree });
        }
    }
    status
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub status: GitStatus,
    pub branches: Vec<String>,
    pub operation: Option<&'static str>,
    pub has_head: bool,
}

pub fn snapshot(repo: &Path) -> Result<Snapshot, String> {
    let status = parse_status(&run(repo, &status_args().into_iter().map(str::to_string).collect::<Vec<_>>())?);
    let branches = run(repo, &["for-each-ref".into(), "--format=%(refname:short)".into(), "refs/heads/".into()])?
        .lines().map(str::to_string).collect();
    let git_path = |name: &str| -> bool {
        run(repo, &["rev-parse".into(), "--git-path".into(), name.into()]).ok()
            .is_some_and(|path| repo.join(path.trim()).exists())
    };
    let operation = if git_path("rebase-merge") || git_path("rebase-apply") { Some("rebase") }
        else if git_path("MERGE_HEAD") { Some("merge") } else { None };
    let has_head = run(repo, &["rev-parse".into(), "--verify".into(), "HEAD".into()]).is_ok();
    Ok(Snapshot { status, branches, operation, has_head })
}

pub fn unstage(repo: &Path, file: Option<&GitFile>) -> Result<String, String> {
    let has_head = run(repo, &["rev-parse".into(), "--verify".into(), "HEAD".into()]).is_ok();
    let mut args: Vec<String> = if has_head {
        vec!["restore".into(), "--staged".into(), "--".into()]
    } else {
        vec!["rm".into(), "--cached".into(), "-r".into(), "-f".into(), "--".into()]
    };
    if let Some(file) = file {
        args.push(file.path.clone());
        if let Some(old) = &file.old_path { args.push(old.clone()); }
    } else { args.push(".".into()); }
    run(repo, &args)
}

pub fn stash_discard(repo: &Path, file: &GitFile) -> Result<String, String> {
    if file.conflicted() { return Err("Resolve the conflict before discarding changes.".into()); }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| err.to_string())?.as_millis();
    let name = format!("discard/{}/{stamp}", file.path);
    let before = run(repo, &["rev-parse".into(), "--verify".into(), "refs/stash".into()]).ok();
    let mut args = vec!["stash".into(), "push".into(), "--include-untracked".into()];
    if !file.untracked() { args.push("--keep-index".into()); }
    args.extend(["-m".into(), name, "--".into(), file.path.clone()]);
    run(repo, &args)?;
    let after = run(repo, &["rev-parse".into(), "--verify".into(), "refs/stash".into()])?;
    if before.as_deref() == Some(after.as_str()) { return Err("No changes saved; file left untouched.".into()); }
    Ok("Saved to stash@{0}. Recover with git stash apply stash@{0}.".into())
}

pub fn switch_args(branch: &str, create: bool) -> Result<Vec<String>, String> {
    if branch.is_empty() || branch.starts_with('-') { return Err("Choose a valid branch name.".into()); }
    let mut args = vec!["switch".into()];
    if create { args.push("-c".into()); }
    args.push(branch.into());
    Ok(args)
}

pub fn publish_args(branch: &str) -> Vec<String> {
    vec!["push".into(), "-u".into(), "origin".into(), format!("refs/heads/{branch}:refs/heads/{branch}")]
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
