use std::path::Path;
use crate::git;

pub const PAGE_SIZE: usize = 200;
pub const MAX_COMMITS: usize = 2_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub oid: String,
    pub parents: Vec<String>,
    pub short: String,
    pub refs: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    pub head: Option<String>,
    pub commits: Vec<Commit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: char,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphRow {
    pub column: usize,
    pub incoming: bool,
    pub lanes: usize,
    pub continuations: Vec<(usize, usize)>,
    pub parents: Vec<usize>,
}

pub fn read(repo: &Path, limit: usize) -> Result<History, String> {
    git::run(repo, &["rev-parse".into(), "--git-dir".into()])?;
    let head = git::run(repo, &["rev-parse".into(), "--verify".into(), "HEAD".into()])
        .ok().map(|oid| oid.trim().to_string());
    let refs = git::run(repo, &["rev-parse".into(), "--all".into()])?;
    if head.is_none() && refs.trim().is_empty() { return Ok(History::default()); }
    let mut args = vec!["log".into(), "--all".into(), "--topo-order".into(),
        "--no-color".into(), "--no-show-signature".into(), "-z".into(),
        format!("--max-count={}", limit.clamp(1, MAX_COMMITS)),
        "--format=%H%x00%P%x00%h%x00%D%x00%s%x00%an%x00%aI%x00%B".into()];
    if head.is_some() { args.push("HEAD".into()); }
    let text = git::run(repo, &args)?;
    Ok(History { head, commits: parse(&text)? })
}

pub fn parse(text: &str) -> Result<Vec<Commit>, String> {
    let mut fields: Vec<&str> = text.split('\0').collect();
    if fields.last() == Some(&"") { fields.pop(); }
    if fields.is_empty() { return Ok(Vec::new()); }
    if fields.len() % 8 != 0 { return Err("Incomplete Git history record.".into()); }
    fields.chunks_exact(8).map(|entry| {
        let oid = entry[0].trim();
        validate_oid(oid)?;
        let parents: Vec<String> = entry[1].split_whitespace().map(str::to_string).collect();
        for parent in &parents { validate_oid(parent)?; }
        Ok(Commit { oid: oid.into(), parents, short: entry[2].into(), refs: entry[3].into(),
            subject: entry[4].into(), author: entry[5].into(), date: entry[6].into(),
            message: entry[7].trim_end_matches('\n').into() })
    }).collect()
}

pub fn changes(repo: &Path, oid: &str) -> Result<Vec<Change>, String> {
    validate_oid(oid)?;
    let text = git::run(repo, &["show".into(), "--format=".into(), "--name-status".into(),
        "--no-color".into(), "--no-ext-diff".into(), "--find-renames".into(),
        "--first-parent".into(), "--diff-merges=first-parent".into(), "-z".into(), oid.into(), "--".into()])?;
    parse_changes(&text)
}

pub fn parse_changes(text: &str) -> Result<Vec<Change>, String> {
    let mut fields = text.split('\0').peekable();
    let mut changes = Vec::new();
    while let Some(status) = fields.next() {
        let status = status.trim_start_matches('\n');
        if status.is_empty() { continue; }
        let kind = status.chars().next().ok_or("Missing change status.")?;
        let first = fields.next().filter(|path| !path.is_empty()).ok_or("Missing changed path.")?;
        let (old_path, path) = if matches!(kind, 'R' | 'C') {
            (Some(first.to_string()), fields.next().filter(|path| !path.is_empty()).ok_or("Missing renamed path.")?)
        } else { (None, first) };
        changes.push(Change { path: path.into(), old_path, kind });
    }
    Ok(changes)
}

pub fn diff(repo: &Path, oid: &str, change: &Change) -> Result<String, String> {
    validate_oid(oid)?;
    let mut args = vec!["show".into(), "--format=".into(), "--no-ext-diff".into(),
        "--no-color".into(), "--find-renames".into(), "--first-parent".into(),
        "--diff-merges=first-parent".into(), oid.into(), "--".into(), change.path.clone()];
    if let Some(old) = &change.old_path { args.push(old.clone()); }
    git::run(repo, &args)
}

fn validate_oid(oid: &str) -> Result<(), String> {
    if matches!(oid.len(), 40 | 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit()) { Ok(()) }
    else { Err("Invalid commit object ID.".into()) }
}

pub fn graph(commits: &[Commit]) -> Vec<GraphRow> {
    let mut lanes: Vec<String> = Vec::new();
    commits.iter().map(|commit| {
        let incoming = lanes.contains(&commit.oid);
        let column = lanes.iter().position(|oid| oid == &commit.oid).unwrap_or_else(|| {
            lanes.push(commit.oid.clone()); lanes.len() - 1
        });
        let before = lanes.clone();
        lanes.remove(column);
        if let Some(first) = commit.parents.first() {
            if !lanes.contains(first) { lanes.insert(column.min(lanes.len()), first.clone()); }
        }
        for parent in commit.parents.iter().skip(1) {
            if !lanes.contains(parent) { lanes.push(parent.clone()); }
        }
        let continuations = before.iter().enumerate().filter(|(index, _)| *index != column)
            .filter_map(|(from, oid)| lanes.iter().position(|lane| lane == oid).map(|to| (from, to))).collect();
        let parents = commit.parents.iter().filter_map(|parent| lanes.iter().position(|lane| lane == parent)).collect();
        GraphRow { column, incoming, lanes: before.len().max(lanes.len()), continuations, parents }
    }).collect()
}
