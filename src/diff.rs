
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Diff {
    pub files: Vec<FileDiff>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn adds(&self) -> usize {
        self.files.iter().map(FileDiff::adds).sum()
    }

    pub fn dels(&self) -> usize {
        self.files.iter().map(FileDiff::dels).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    pub fn adds(&self) -> usize {
        self.lines()
            .filter(|line| line.kind == LineKind::Add)
            .count()
    }

    pub fn dels(&self) -> usize {
        self.lines()
            .filter(|line| line.kind == LineKind::Del)
            .count()
    }

    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.hunks.iter().flat_map(|hunk| hunk.lines.iter())
    }

    pub fn name(&self) -> &str {
        self.path
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub header: String,
    pub header_old: String,
    pub header_new: String,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Add,
    Del,
    Context,
    Meta,
}

pub fn parse(patch: &str) -> Diff {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut file: Option<FileDiff> = None;
    let mut hunk: Option<Hunk> = None;
    let mut old_path: Option<String> = None;
    let mut new_path: Option<String> = None;
    let mut pending: Vec<Line> = Vec::new();
    let mut old_cursor = 0u32;
    let mut new_cursor = 0u32;

    for raw in patch.lines() {
        if raw.starts_with("@@") {
            flush_hunk(&mut file, &mut hunk);
            match parse_hunk_header(raw) {
                Some((header_old, header_new, old_start, new_start)) => {
                    if file.is_none() {
                        file = Some(FileDiff {
                            path: pick_path(&old_path, &new_path),
                            hunks: Vec::new(),
                        });
                    }
                    old_cursor = old_start;
                    new_cursor = new_start;
                    hunk = Some(Hunk {
                        header: raw.to_string(),
                        header_old,
                        header_new,
                        lines: std::mem::take(&mut pending),
                    });
                }
                None => push_line(&mut hunk, &mut pending, meta(raw)),
            }
            continue;
        }

        if let Some(rest) = raw.strip_prefix("--- ") {
            flush_hunk(&mut file, &mut hunk);
            flush_file(&mut files, &mut file);
            old_path = Some(clean_path(rest));
            new_path = None;
            pending.push(meta(raw));
            continue;
        }

        if let Some(rest) = raw.strip_prefix("+++ ") {
            new_path = Some(clean_path(rest));
            pending.push(meta(raw));
            continue;
        }

        let Some(current) = hunk.as_mut() else {
            pending.push(meta(raw));
            continue;
        };

        let line = match raw.as_bytes().first() {
            Some(b'+') => {
                let new_no = new_cursor;
                new_cursor += 1;
                Line {
                    kind: LineKind::Add,
                    old_no: None,
                    new_no: Some(new_no),
                    text: raw.to_string(),
                }
            }
            Some(b'-') => {
                let old_no = old_cursor;
                old_cursor += 1;
                Line {
                    kind: LineKind::Del,
                    old_no: Some(old_no),
                    new_no: None,
                    text: raw.to_string(),
                }
            }
            Some(b' ') => {
                let old_no = old_cursor;
                let new_no = new_cursor;
                old_cursor += 1;
                new_cursor += 1;
                Line {
                    kind: LineKind::Context,
                    old_no: Some(old_no),
                    new_no: Some(new_no),
                    text: raw.to_string(),
                }
            }
            _ => meta(raw),
        };
        current.lines.push(line);
    }

    flush_hunk(&mut file, &mut hunk);
    flush_file(&mut files, &mut file);
    Diff { files }
}

fn flush_hunk(file: &mut Option<FileDiff>, hunk: &mut Option<Hunk>) {
    if let (Some(file), Some(hunk)) = (file.as_mut(), hunk.take()) {
        file.hunks.push(hunk);
    }
}

fn flush_file(files: &mut Vec<FileDiff>, file: &mut Option<FileDiff>) {
    if let Some(file) = file.take() {
        files.push(file);
    }
}

fn push_line(hunk: &mut Option<Hunk>, pending: &mut Vec<Line>, line: Line) {
    match hunk.as_mut() {
        Some(hunk) => hunk.lines.push(line),
        None => pending.push(line),
    }
}

fn meta(raw: &str) -> Line {
    Line {
        kind: LineKind::Meta,
        old_no: None,
        new_no: None,
        text: raw.to_string(),
    }
}

fn parse_hunk_header(line: &str) -> Option<(String, String, u32, u32)> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "@@" {
        return None;
    }
    let old = parts.next()?.to_string();
    let new = parts.next()?.to_string();
    let old_start = range_start(old.strip_prefix('-')?)?;
    let new_start = range_start(new.strip_prefix('+')?)?;
    Some((old, new, old_start, new_start))
}

fn range_start(range: &str) -> Option<u32> {
    range.split(',').next()?.parse().ok()
}

fn clean_path(raw: &str) -> String {
    let path = raw.split('\t').next().unwrap_or(raw).trim();
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_string()
}

fn pick_path(old: &Option<String>, new: &Option<String>) -> String {
    if let Some(path) = new.as_deref() {
        if !path.is_empty() && path != "/dev/null" {
            return path.to_string();
        }
    }
    old.clone().unwrap_or_default()
}


