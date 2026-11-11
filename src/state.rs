
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::config::{self, Config, Target};

pub const WORKSPACE: &str = "~/projects";
pub const PI_VERSION: &str = "1.5.0";
pub const DEFAULT_MODEL: &str = "—";
pub const DEFAULT_MODEL_ID: &str = "";
pub const DEFAULT_MODEL_PROVIDER: &str = "";
pub const DEFAULT_SESSION: &str = "desktop";

#[derive(Clone, Debug)]
pub struct LocalTransport {
    pub bin: Option<PathBuf>,
    pub workspace: PathBuf,
    pub args: Vec<String>,
}

impl LocalTransport {
    pub fn args(&self) -> Vec<String> {
        self.args.clone()
    }
}

#[derive(Clone, Debug)]
pub enum Transport {
    Local(LocalTransport),
    Remote { target: String, command: String },
    Missing { expected: String },
}

impl Transport {
    pub fn from_config(config: &Config) -> Self {
        match config.target {
            Target::Local => local_transport(config),
            Target::Remote if !config.remote_ssh.trim().is_empty() => Transport::Remote {
                target: config.remote_ssh.clone(),
                command: config.remote_command(),
            },
            Target::Remote => local_transport(config),
        }
    }
}

fn local_transport(config: &Config) -> Transport {
    match resolve_pi_bin(&config.bin) {
        Ok(bin) => Transport::Local(LocalTransport {
            bin: Some(bin),
            workspace: config::expand_home(config.effective_workspace()),
            args: config.args(),
        }),
        Err(expected) => Transport::Missing { expected },
    }
}

pub fn resolve_pi_bin(configured: &str) -> Result<PathBuf, String> {
    let expected = if configured.trim().is_empty() {
        config::home_file(config::DEFAULT_BIN)
    } else {
        config::expand_home(configured.trim())
    };

    if let Some(value) = std::env::var_os("PI_BIN") {
        let path = config::expand_home(&value.to_string_lossy());
        if is_executable(&path) {
            return Ok(path);
        }
        return Err(path.display().to_string());
    }

    for candidate in [expected.clone(), config::home_file(".local/bin/pi")] {
        if is_executable(&candidate) {
            return Ok(candidate);
        }
    }

    if let Some(found) = find_in_path("pi") {
        return Ok(found);
    }

    Err(expected.display().to_string())
}

pub fn path_with_pi(bin: &Path) -> OsString {
    let mut parts: Vec<PathBuf> = Vec::new();
    if let Some(parent) = bin.parent() {
        parts.push(parent.to_path_buf());
    }

    // Pi is a Node CLI. A desktop launcher often has a much smaller PATH than
    // an interactive shell, so include the common per-user Node locations too.
    let home = config::home_file("");
    for relative in [
        ".local/node22/bin",
        ".local/node/bin",
        ".local/bin",
        ".volta/bin",
        ".bun/bin",
        ".local/share/pnpm",
        ".fnm/aliases/default/bin",
    ] {
        push_unique(&mut parts, home.join(relative));
    }
    if let Ok(entries) = std::fs::read_dir(home.join(".local")) {
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("node"))
            })
            .filter(|path| path.join("node").is_file())
            .map(|path| path.join("bin"))
            .collect();
        dirs.sort();
        for dir in dirs {
            push_unique(&mut parts, dir);
        }
    }
    if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("bin"))
            .collect();
        dirs.sort();
        for dir in dirs {
            push_unique(&mut parts, dir);
        }
    }
    for extra in ["/usr/local/bin", "/usr/bin", "/bin"] {
        push_unique(&mut parts, PathBuf::from(extra));
    }
    if let Some(existing) = std::env::var_os("PATH") {
        for part in std::env::split_paths(&existing) {
            push_unique(&mut parts, part);
        }
    }
    std::env::join_paths(parts).unwrap_or_else(|_| OsString::from("/usr/bin:/bin"))
}

fn push_unique(parts: &mut Vec<PathBuf>, value: PathBuf) {
    if !parts.contains(&value) {
        parts.push(value);
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TurnPhase {
    Sent,
    Thinking,
    Tools,
    Writing,
    Done,
    Error,
}

impl Default for TurnPhase {
    fn default() -> Self {
        Self::Sent
    }
}

impl TurnPhase {
    pub fn label(self) -> &'static str {
        match self {
            TurnPhase::Sent => "SENT",
            TurnPhase::Thinking => "THINKING",
            TurnPhase::Tools => "TOOLS",
            TurnPhase::Writing => "WRITING",
            TurnPhase::Done => "DONE",
            TurnPhase::Error => "ERROR",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Ask,
    Smart,
    Off,
}

impl Permission {
    pub fn all() -> [Self; 3] {
        [Permission::Ask, Permission::Smart, Permission::Off]
    }

    pub fn label(self) -> &'static str {
        match self {
            Permission::Ask => "ASK",
            Permission::Smart => "SMART",
            Permission::Off => "OFF",
        }
    }

    pub fn from_approve(approve: crate::config::Approve) -> Self {
        match approve {
            crate::config::Approve::No => Permission::Ask,
            crate::config::Approve::Ask => Permission::Smart,
            crate::config::Approve::Yes => Permission::Off,
        }
    }

    pub fn to_approve(self) -> crate::config::Approve {
        match self {
            Permission::Ask => crate::config::Approve::No,
            Permission::Smart => crate::config::Approve::Ask,
            Permission::Off => crate::config::Approve::Yes,
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Permission::Ask => "ask before privileged tools run",
            Permission::Smart => "Pi decides; non-dangerous tools pass",
            Permission::Off => "tools run without permission prompts",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Connection {
    Connecting,
    Connected,
    Disconnected(String),
}

impl Connection {
    pub fn label(&self) -> &'static str {
        match self {
            Connection::Connecting => "connecting",
            Connection::Connected => "Connected",
            Connection::Disconnected(_) => "disconnected",
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, Connection::Connected)
    }
}

#[derive(Clone)]
pub struct CommandItem {
    pub name: String,
    pub description: String,
    pub source: String,
}

#[derive(Clone)]
pub struct ModelItem {
    pub provider: String,
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct PluginItem {
    pub spec: String,
    pub origin: String,
}

impl PluginItem {
    pub fn name(&self) -> String {
        let value = self
            .spec
            .split_once(':')
            .map(|(_, rest)| rest)
            .unwrap_or(&self.spec)
            .trim_end_matches('/');
        value
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(value)
            .to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogItem {
    pub name: String,
    pub version: String,
    pub description: String,
    pub date: String,
    pub downloads: u64,
    pub downloads_weekly: u64,
    pub kind: String,
    pub author: String,
    pub npm_url: String,
    pub repo_url: String,
    pub homepage: String,
    pub thumb: String,
}

pub fn model_display(model: &str, models: &[ModelItem]) -> String {
    let raw = model.trim();
    if raw.is_empty() {
        return "—".to_string();
    }
    let (provider, id) = match raw.split_once('/') {
        Some((provider, id)) => (Some(provider), id),
        None => (None, raw),
    };
    let matches = |item: &ModelItem| {
        item.id == raw
            || (item.id == id && provider.map_or(true, |provider| provider == item.provider))
    };
    if let Some(item) = models.iter().find(|item| matches(item)) {
        return item.name.clone();
    }
    if let Some(item) = models.iter().find(|item| item.id == id) {
        return item.name.clone();
    }
    if let Some(name) = builtin_model_name(raw).or_else(|| builtin_model_name(id)) {
        return name.to_string();
    }
    id.to_string()
}

fn builtin_model_name(id: &str) -> Option<&'static str> {
    match id {
        "deepseek-flash" => Some("DeepSeek V4.1 Flash"),
        "deepseek-v4-pro" => Some("DeepSeek V4 Pro"),
        "openai/gpt-oss-120b" | "gpt-oss-120b" => Some("GPT OSS 120B"),
        "openai/gpt-oss-20b" | "gpt-oss-20b" => Some("GPT OSS 20B"),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct SessionActivity {
    pub running: bool,
    pub phase: String,
    pub started: Option<Instant>,
    pub unread: bool,
}

impl SessionActivity {
    pub fn elapsed_label(&self) -> String {
        self.started
            .map(|start| format_elapsed(start.elapsed()))
            .unwrap_or_else(|| "00:00".to_string())
    }
}

#[derive(Clone)]
pub struct RemoteSession {
    pub path: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub preview: Option<String>,
    pub session_id: String,
    pub mtime: i64,
}

impl RemoteSession {
    pub fn title(&self) -> String {
        if let Some(name) = self.name.as_deref() {
            if !name.is_empty() {
                return name.to_string();
            }
        }
        if !self.session_id.is_empty() && self.session_id.chars().count() <= 24 {
            return self.session_id.clone();
        }
        if let Some(preview) = self.preview.as_deref() {
            let preview = preview.trim();
            if !preview.is_empty() {
                return truncate(preview, 40);
            }
        }
        if !self.session_id.is_empty() {
            return truncate(&self.session_id, 24);
        }
        self.path
            .rsplit('/')
            .next()
            .unwrap_or("Session")
            .to_string()
    }

    pub fn model_label(&self, models: &[ModelItem]) -> String {
        match self.model.as_deref() {
            Some(model) if !model.is_empty() => model_display(model, models),
            _ => "—".to_string(),
        }
    }

    pub fn age(&self) -> String {
        relative_time(self.mtime)
    }
}

pub struct AppState {
    pub session_id: String,
    pub session_name: Option<String>,
    pub session_file: Option<String>,
    pub model: String,
    pub model_id: String,
    pub model_provider: String,
    pub workspace: String,
    pub transport_label: String,
    pub permission: Permission,
    pub connection: Connection,
    pub conn_detail: String,
    pub running: bool,
    pub phase: TurnPhase,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub turn_finished_tokens: u64,
    pub turn_tokens: u64,
    pub cost: f64,
    pub context_percent: Option<f64>,
    pub cache_read: u64,
    pub cache_write: u64,
    pub context_tokens: Option<u64>,
    pub context_window: Option<u64>,
    pub streamed_chars: u64,
    pub run_started: Option<Instant>,
    pub run_elapsed: Duration,
    pub session_started: Instant,
    pub thinking_level: String,
    pub thinking_levels: Vec<String>,
    pub pending_messages: usize,
    pub commands: Vec<CommandItem>,
    pub models: Vec<ModelItem>,
    pub sessions: Vec<RemoteSession>,
    pub active_session: Option<usize>,
    pub plugins: Vec<PluginItem>,
    pub artifacts: usize,
    pub jobs: usize,
    pub first_prompt: Option<String>,
    pub auto_named: bool,
    pub title_requested: bool,
    pub title_attempts: u8,
    pub missing_model_noted: bool,
    pub state_synced: bool,
    pub messages_loaded: bool,
    pub history_loaded: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            session_id: DEFAULT_SESSION.to_string(),
            session_name: None,
            session_file: None,
            model: DEFAULT_MODEL.to_string(),
            model_id: DEFAULT_MODEL_ID.to_string(),
            model_provider: DEFAULT_MODEL_PROVIDER.to_string(),
            workspace: WORKSPACE.to_string(),
            transport_label: format!("LOCAL · {WORKSPACE}"),
            permission: Permission::Smart,
            connection: Connection::Connecting,
            conn_detail: String::new(),
            running: false,
            phase: TurnPhase::Sent,
            tokens_in: 0,
            tokens_out: 0,
            turn_finished_tokens: 0,
            turn_tokens: 0,
            cost: 0.0,
            context_percent: None,
            cache_read: 0,
            cache_write: 0,
            context_tokens: None,
            context_window: None,
            streamed_chars: 0,
            run_started: None,
            run_elapsed: Duration::ZERO,
            session_started: Instant::now(),
            thinking_level: "medium".to_string(),
            thinking_levels: Vec::new(),
            pending_messages: 0,
            commands: Vec::new(),
            models: Vec::new(),
            sessions: Vec::new(),
            active_session: None,
            plugins: Vec::new(),
            artifacts: 0,
            jobs: 0,
            first_prompt: None,
            auto_named: false,
            title_requested: false,
            title_attempts: 0,
            missing_model_noted: false,
            state_synced: false,
            messages_loaded: false,
            history_loaded: false,
        }
    }
}

impl AppState {
    pub fn apply_config(&mut self, config: &Config) {
        self.workspace = config.effective_workspace().to_string();
        self.transport_label = config.transport_label();
        self.session_id = config.effective_session_id().to_string();
        if !config.model.trim().is_empty() {
            self.model = model_display(&config.model, &[]);
            self.model_id = config.model.trim().to_string();
        }
        if config.thinking != crate::config::Thinking::Unset {
            self.thinking_level = config.thinking.as_str().to_string();
        }
        self.permission = Permission::from_approve(config.approve);
    }

    pub fn display_name(&self) -> String {
        match self.session_name.as_deref() {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => self.session_id.clone(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        match self.run_started {
            Some(start) if self.running => start.elapsed(),
            _ => self.run_elapsed,
        }
    }

    pub fn session_elapsed(&self) -> Duration {
        self.session_started.elapsed()
    }

    pub fn tokens_per_sec(&self) -> f64 {
        let secs = self.elapsed().as_secs_f64();
        if secs <= 0.5 {
            return 0.0;
        }
        let tokens = if self.turn_tokens > 0 {
            self.turn_tokens
        } else {
            self.streamed_chars / 4
        };
        tokens as f64 / secs
    }

    pub fn reset_turn(&mut self) {
        self.phase = TurnPhase::Sent;
        self.run_started = Some(Instant::now());
        self.run_elapsed = Duration::ZERO;
        self.streamed_chars = 0;
        self.turn_finished_tokens = 0;
        self.turn_tokens = 0;
    }

    pub fn reset_session_counters(&mut self) {
        self.tokens_in = 0;
        self.tokens_out = 0;
        self.turn_finished_tokens = 0;
        self.turn_tokens = 0;
        self.cost = 0.0;
        self.context_percent = None;
        self.streamed_chars = 0;
        self.run_elapsed = Duration::ZERO;
        self.run_started = None;
        self.session_started = Instant::now();
        self.first_prompt = None;
        self.auto_named = false;
        self.title_requested = false;
        self.title_attempts = 0;
        self.missing_model_noted = false;
        self.state_synced = false;
        self.session_file = None;
        self.pending_messages = 0;
    }

    pub fn active_index_for(&self, path: &str) -> Option<usize> {
        self.sessions.iter().position(|session| session.path == path)
    }

    pub fn resolve_active_session(&mut self) {
        if let Some(file) = self.session_file.as_deref() {
            if let Some(index) = self.active_index_for(file) {
                self.active_session = Some(index);
                return;
            }
            let base = file.rsplit('/').next().unwrap_or(file);
            if let Some(index) = self
                .sessions
                .iter()
                .position(|session| session.path.rsplit('/').next() == Some(base))
            {
                self.active_session = Some(index);
                return;
            }
        }

        if !self.session_id.is_empty() {
            let by_id: Vec<usize> = self
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, session)| session.session_id == self.session_id)
                .map(|(index, _)| index)
                .collect();
            if by_id.len() == 1 {
                self.active_session = Some(by_id[0]);
                return;
            }
        }

        if let Some(name) = self.session_name.as_deref() {
            if !name.is_empty() {
                let by_name: Vec<usize> = self
                    .sessions
                    .iter()
                    .enumerate()
                    .filter(|(_, session)| session.name.as_deref() == Some(name))
                    .map(|(index, _)| index)
                    .collect();
                if by_name.len() == 1 {
                    self.active_session = Some(by_name[0]);
                    return;
                }
            }
        }

        self.active_session = None;
    }
}

pub fn thinking_fallback(levels: &[String], current: &str) -> Option<String> {
    if levels.is_empty() || levels.iter().any(|level| level == current) {
        None
    } else {
        Some(levels[0].clone())
    }
}

pub fn format_tokens(n: u64) -> String {
    if n >= 100_000 {
        format!("{:.0}k", n as f64 / 1000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

pub fn format_cost(cost: f64) -> String {
    if cost <= 0.0 {
        "$0".to_string()
    } else if cost < 1.0 {
        format!("${cost:.4}")
    } else {
        format!("${cost:.2}")
    }
}

pub fn format_elapsed(d: Duration) -> String {
    let total = d.as_secs();
    if total >= 3600 {
        format!(
            "{:02}:{:02}:{:02}",
            total / 3600,
            (total % 3600) / 60,
            total % 60
        )
    } else {
        format!("{:02}:{:02}", total / 60, total % 60)
    }
}

pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

pub fn relative_time(mtime: i64) -> String {
    if mtime <= 0 {
        return String::new();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = now - mtime;
    if diff < 60 {
        "Now".to_string()
    } else if diff < 3_600 {
        format!("there is {} min", diff / 60)
    } else if diff < 86_400 {
        format!("there is {} h", diff / 3_600)
    } else {
        format!("there is {} d", diff / 86_400)
    }
}

fn truncate(text: &str, max: usize) -> String {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= max {
        return clean;
    }
    let mut out: String = clean.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub fn slug_from_prompt(prompt: &str) -> String {
    let cleaned = prompt
        .trim_start_matches('/')
        .replace(['\n', '\r', '\t'], " ");
    let words: Vec<&str> = cleaned.split_whitespace().take(5).collect();
    let joined = words.join("-").to_lowercase();
    let mut slug: String = joined
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == '-' || c == '_' {
                '-'
            } else {
                ' '
            }
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').trim().replace(' ', "-");
    slug.truncate(48);
    if slug.is_empty() {
        "session".to_string()
    } else {
        slug
    }
}


