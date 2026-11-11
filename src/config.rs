
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::theme::FontChoice;

pub const DEFAULT_BIN: &str = "~/.local/bin/pi";
pub const DEFAULT_REMOTE_SSH: &str = "";
pub const DEFAULT_WORKSPACE: &str = "~/projects";
pub const DEFAULT_SESSION_ID: &str = "desktop";

pub const DEFAULT_CHAT_WIDTH: i32 = 78;
pub const MIN_CHAT_WIDTH: i32 = 60;
pub const MAX_CHAT_WIDTH: i32 = 120;
pub const DEFAULT_FONT_SCALE: i32 = 100;
pub const MIN_FONT_SCALE: i32 = 90;
pub const MAX_FONT_SCALE: i32 = 130;


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Local,
    Remote,
}

impl Target {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Some(Target::Local),
            "remote" | "ssh" => Some(Target::Remote),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Target::Local => "local",
            Target::Remote => "remote",
        }
    }

    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Target::Local => "local",
            Target::Remote => "remote",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Target::Local => 0,
            Target::Remote => 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    #[default]
    Code,
    Chat,
}

impl ViewMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "code" => Some(ViewMode::Code),
            "chat" | "Talk." => Some(ViewMode::Chat),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ViewMode::Code => "code",
            ViewMode::Chat => "chat",
        }
    }

    pub fn is_chat(self) -> bool {
        self == ViewMode::Chat
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Approve {
    #[default]
    Ask,
    Yes,
    No,
}

impl Approve {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ask" => Some(Approve::Ask),
            "yes" | "approve" => Some(Approve::Yes),
            "no" | "no-approve" => Some(Approve::No),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Approve::Ask => "ask",
            Approve::Yes => "yes",
            Approve::No => "no",
        }
    }

    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Approve::Ask => "ask",
            Approve::Yes => "allow",
            Approve::No => "deny",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Approve::Ask => 0,
            Approve::Yes => 1,
            Approve::No => 2,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Approve::Yes,
            2 => Approve::No,
            _ => Approve::Ask,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Thinking {
    #[default]
    Unset,
    Off,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Thinking {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "default" | "standard" => Some(Thinking::Unset),
            "off" => Some(Thinking::Off),
            "minimal" => Some(Thinking::Minimal),
            "low" => Some(Thinking::Low),
            "medium" => Some(Thinking::Medium),
            "high" => Some(Thinking::High),
            "xhigh" => Some(Thinking::Xhigh),
            "max" => Some(Thinking::Max),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Thinking::Unset => "",
            Thinking::Off => "off",
            Thinking::Minimal => "minimal",
            Thinking::Low => "low",
            Thinking::Medium => "medium",
            Thinking::High => "high",
            Thinking::Xhigh => "xhigh",
            Thinking::Max => "max",
        }
    }

    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Thinking::Unset => "Pi default",
            Thinking::Off => "Off",
            Thinking::Minimal => "minimum",
            Thinking::Low => "low",
            Thinking::Medium => "medium",
            Thinking::High => "high",
            Thinking::Xhigh => "too high",
            Thinking::Max => "maximum",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Thinking::Unset => 0,
            Thinking::Off => 1,
            Thinking::Minimal => 2,
            Thinking::Low => 3,
            Thinking::Medium => 4,
            Thinking::High => 5,
            Thinking::Xhigh => 6,
            Thinking::Max => 7,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Thinking::Off,
            2 => Thinking::Minimal,
            3 => Thinking::Low,
            4 => Thinking::Medium,
            5 => Thinking::High,
            6 => Thinking::Xhigh,
            7 => Thinking::Max,
            _ => Thinking::Unset,
        }
    }
}


#[derive(Clone, Debug, Default, PartialEq)]
pub struct CliArgs {
    pub session: Option<String>,
    pub workspace: Option<PathBuf>,
    pub error: Option<String>,
    pub remaining: Vec<String>,
}

impl CliArgs {
    pub fn wants_instance(&self) -> bool {
        self.session.is_some() || self.workspace.is_some()
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub target: Target,
    pub bin: String,
    pub remote_ssh: String,
    pub remote_cmd: String,
    pub workspace: String,
    pub session_id: String,
    pub session_dir: String,
    pub no_session: bool,
    pub start_last: bool,

    pub name: String,
    pub model: String,
    pub aux_model: String,
    pub models: String,
    pub provider: String,
    pub thinking: Thinking,

    pub tools: String,
    pub exclude_tools: String,
    pub no_tools: bool,
    pub no_builtin_tools: bool,
    pub no_mcp: bool,
    pub approve: Approve,
    pub offline: bool,

    pub skills: String,
    pub no_skills: bool,
    pub extensions: String,
    pub no_extensions: bool,
    pub prompt_templates: String,
    pub no_prompt_templates: bool,
    pub no_context_files: bool,
    pub system_prompt: String,
    pub append_system_prompt: String,

    pub view: ViewMode,
    pub timestamps: bool,
    pub auto_scroll: bool,
    pub chat_width: i32,
    pub font_scale: i32,
    pub font: FontChoice,
    pub show_costs: bool,
    pub notify: bool,

    session_override: Option<String>,
    launch_session_id: Option<String>,
    session_path: Option<String>,
    workspace_override: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target: Target::Local,
            bin: DEFAULT_BIN.to_string(),
            remote_ssh: DEFAULT_REMOTE_SSH.to_string(),
            remote_cmd: String::new(),
            workspace: DEFAULT_WORKSPACE.to_string(),
            session_id: DEFAULT_SESSION_ID.to_string(),
            session_dir: String::new(),
            no_session: false,
            start_last: false,
            name: String::new(),
            model: String::new(),
            aux_model: String::new(),
            models: String::new(),
            provider: String::new(),
            thinking: Thinking::Unset,
            tools: String::new(),
            exclude_tools: String::new(),
            no_tools: false,
            no_builtin_tools: false,
            no_mcp: false,
            approve: Approve::Ask,
            offline: false,
            skills: String::new(),
            no_skills: false,
            extensions: String::new(),
            no_extensions: false,
            prompt_templates: String::new(),
            no_prompt_templates: false,
            no_context_files: false,
            system_prompt: String::new(),
            append_system_prompt: String::new(),
            view: ViewMode::Code,
            timestamps: true,
            auto_scroll: true,
            chat_width: DEFAULT_CHAT_WIDTH,
            font_scale: DEFAULT_FONT_SCALE,
            font: FontChoice::Pixel,
            show_costs: true,
            notify: true,
            session_override: None,
            launch_session_id: None,
            session_path: None,
            workspace_override: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let mut config = match config_path() {
            Some(path) => Self::load_from(&path),
            None => Self::default(),
        };
        config.apply_env();
        config
    }

    pub fn load_from(path: &Path) -> Self {
        let mut config = Self::default();
        if let Ok(text) = std::fs::read_to_string(path) {
            config.merge(&text);
        }
        config
    }

    pub fn from_args(argv: &[String]) -> CliArgs {
        let mut cli = CliArgs::default();
        let mut index = 0;
        while index < argv.len() {
            let raw = argv[index].as_str();
            let (flag, inline) = match raw.split_once('=') {
                Some((flag, value)) if flag.starts_with("--") => (flag, Some(value)),
                _ => (raw, None),
            };
            match flag {
                "--session" | "--workspace" => {
                    let value = match inline {
                        Some(value) => Some(value.to_string()),
                        None => match argv.get(index + 1) {
                            Some(next) if !next.starts_with("--") => {
                                index += 1;
                                Some(next.clone())
                            }
                            _ => None,
                        },
                    };
                    match value {
                        Some(value) if !value.trim().is_empty() => {
                            if flag == "--session" {
                                cli.session = Some(value);
                            } else {
                                cli.workspace = Some(PathBuf::from(value));
                            }
                        }
                        _ => {
                            if cli.error.is_none() {
                                cli.error = Some(format!("{flag} needs a value"));
                            }
                        }
                    }
                }
                _ => cli.remaining.push(raw.to_string()),
            }
            index += 1;
        }
        cli
    }

    pub fn apply_overrides(
        &mut self,
        session: Option<String>,
        workspace: Option<PathBuf>,
    ) {
        if let Some(session) = session {
            let session = session.trim().to_string();
            if session.is_empty() {
                eprintln!("pi-desktop: --empty session ignored");
            } else if !session
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
            {
                eprintln!("pi-desktop: --invalid session (use [a-z0-9-]): {session}");
            } else {
                self.session_override = Some(session);
            }
        }
        if let Some(workspace) = workspace {
            let expanded = expand_home(&workspace.to_string_lossy());
            if expanded.is_dir() {
                self.workspace_override = Some(expanded.to_string_lossy().to_string());
            } else {
                eprintln!(
                    "pi-desktop: --workspace does not exist: {}",
                    workspace.display()
                );
            }
        }
    }

    pub fn effective_session_id(&self) -> &str {
        self.session_override
            .as_deref()
            .or(self.launch_session_id.as_deref())
            .unwrap_or(self.session_id.as_str())
    }

    pub fn launch_session_id(&self) -> Option<&str> {
        self.launch_session_id.as_deref()
    }

    pub fn set_launch_session_id(&mut self, id: Option<String>) {
        self.launch_session_id = id.filter(|id| !id.trim().is_empty());
    }

    pub fn for_session(&self, path: Option<&str>, id: Option<&str>) -> Self {
        let mut config = self.clone();
        config.session_override = None;
        config.launch_session_id = id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string);
        config.session_path = path
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string);
        config.start_last = false;
        config
    }

    pub fn new_launch_session_id(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let base: String = self
            .session_id
            .chars()
            .filter(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || *ch == '-')
            .collect();
        let base = if base.is_empty() { "desktop" } else { base.as_str() };
        format!("{base}-{stamp}")
    }

    pub fn effective_workspace(&self) -> &str {
        self.workspace_override
            .as_deref()
            .unwrap_or(self.workspace.as_str())
    }

    pub fn session_override(&self) -> Option<&str> {
        self.session_override.as_deref()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = config_path() else {
            return Ok(());
        };
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        write_atomic(path, &render_preserving(&existing, &self.known_entries()))
    }

    pub fn merge(&mut self, text: &str) {
        let default = Config::default();
        let mut section = String::new();

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[') {
                section = name
                    .split(']')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();
                continue;
            }
            if !(section.is_empty() || section == "pi" || section == "ui") {
                continue;
            }
            let Some((key, raw_value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let ui = section == "ui";

            let long_value = || unescape_text(raw_value.trim());
            let value = || {
                raw_value
                    .split(['#', ';'])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            };

            match key.as_str() {
                "theme" => {} // do theme.rs
                "view" if ui || section.is_empty() => {
                    if let Some(parsed) = ViewMode::parse(&value()) {
                        self.view = parsed;
                    }
                }
                "timestamps" if ui || section.is_empty() => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.timestamps = parsed;
                    }
                }
                "auto_scroll" if ui || section.is_empty() => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.auto_scroll = parsed;
                    }
                }
                "chat_width" if ui || section.is_empty() => {
                    if let Some(parsed) = parse_int(&value(), MIN_CHAT_WIDTH, MAX_CHAT_WIDTH) {
                        self.chat_width = parsed;
                    } else {
                        self.chat_width = default.chat_width;
                    }
                }
                "font_scale" if ui || section.is_empty() => {
                    if let Some(parsed) = parse_int(&value(), MIN_FONT_SCALE, MAX_FONT_SCALE) {
                        self.font_scale = parsed;
                    } else {
                        self.font_scale = default.font_scale;
                    }
                }
                "font" if ui || section.is_empty() => {
                    self.font = FontChoice::parse(&value()).unwrap_or(default.font);
                }
                "notify" if ui || section.is_empty() => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.notify = parsed;
                    }
                }
                "show_costs" if ui || section.is_empty() => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.show_costs = parsed;
                    }
                }
                "target" if !ui => {
                    if let Some(parsed) = Target::parse(&value()) {
                        self.target = parsed;
                    }
                }
                "bin" if !ui => self.bin = value(),
                "remote_ssh" | "remote" if !ui => self.remote_ssh = value(),
                "remote_cmd" if !ui => self.remote_cmd = value(),
                "workspace" if !ui => {
                    let parsed = value();
                    if parsed.is_empty() {
                        self.workspace = default.workspace.clone();
                    } else {
                        self.workspace = parsed;
                    }
                }
                "session_id" if !ui => self.session_id = value(),
                "session_dir" if !ui => self.session_dir = value(),
                "no_session" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.no_session = parsed;
                    }
                }
                "start_last" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.start_last = parsed;
                    }
                }
                "name" if !ui => self.name = value(),
                "model" if !ui => self.model = value(),
                "aux_model" if !ui => self.aux_model = value(),
                "models" if !ui => self.models = value(),
                "provider" if !ui => self.provider = value(),
                "thinking" if !ui => {
                    if let Some(parsed) = Thinking::parse(&value()) {
                        self.thinking = parsed;
                    }
                }
                "tools" if !ui => self.tools = value(),
                "exclude_tools" if !ui => self.exclude_tools = value(),
                "no_tools" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.no_tools = parsed;
                    }
                }
                "no_builtin_tools" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.no_builtin_tools = parsed;
                    }
                }
                "no_mcp" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.no_mcp = parsed;
                    }
                }
                "approve" if !ui => {
                    if let Some(parsed) = Approve::parse(&value()) {
                        self.approve = parsed;
                    }
                }
                "offline" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.offline = parsed;
                    }
                }
                "skills" if !ui => self.skills = value(),
                "no_skills" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.no_skills = parsed;
                    }
                }
                "extensions" if !ui => self.extensions = value(),
                "no_extensions" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.no_extensions = parsed;
                    }
                }
                "prompt_templates" if !ui => self.prompt_templates = value(),
                "no_prompt_templates" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.no_prompt_templates = parsed;
                    }
                }
                "no_context_files" if !ui => {
                    if let Some(parsed) = parse_bool(&value()) {
                        self.no_context_files = parsed;
                    }
                }
                "system_prompt" if !ui => self.system_prompt = long_value(),
                "append_system_prompt" if !ui => self.append_system_prompt = long_value(),
                _ => {}
            }
        }
    }

    fn apply_env(&mut self) {
        if let Ok(value) = std::env::var("PI_TARGET") {
            if let Some(target) = Target::parse(&value) {
                self.target = target;
            }
        }
        if let Ok(value) = std::env::var("PI_SSH_TARGET") {
            self.remote_ssh = value;
        }
        if let Ok(value) = std::env::var("PI_REMOTE_CMD") {
            self.remote_cmd = value;
        }
        if let Ok(value) = std::env::var("PI_WORKSPACE") {
            self.workspace = value;
        }
        if let Ok(value) = std::env::var("PI_SESSION_ID") {
            self.session_id = value;
        }
        if let Ok(value) = std::env::var("PI_MODEL") {
            self.model = value;
        }
        if let Ok(value) = std::env::var("PI_THINKING") {
            if let Some(level) = Thinking::parse(&value) {
                self.thinking = level;
            }
        }
    }

    pub fn args(&self) -> Vec<String> {
        let mut args: Vec<String> = vec!["--mode".into(), "rpc".into()];

        let session_id = self.effective_session_id().trim();
        if self.no_session {
            args.push("--no-session".into());
        } else if let Some(path) = self.session_path.as_deref().filter(|path| !path.is_empty()) {
            args.push("--session".into());
            args.push(path.to_string());
        } else if self.start_last
            && self.session_override.is_none()
            && self.launch_session_id.is_none()
        {
            args.push("--continue".into());
        } else if !session_id.is_empty() {
            args.push("--session-id".into());
            args.push(session_id.to_string());
        }
        push_value(&mut args, "--session-dir", &self.session_dir);
        push_value(&mut args, "--name", &self.name);

        push_value(&mut args, "--model", &self.model);
        push_value(&mut args, "--models", &self.models);
        push_value(&mut args, "--provider", &self.provider);
        if self.thinking != Thinking::Unset {
            args.push("--thinking".into());
            args.push(self.thinking.as_str().to_string());
        }

        push_value(&mut args, "--tools", &self.tools);
        push_value(&mut args, "--exclude-tools", &self.exclude_tools);
        push_flag(&mut args, self.no_tools, "--no-tools");
        push_flag(&mut args, self.no_builtin_tools, "--no-builtin-tools");
        push_flag(&mut args, self.no_mcp, "--no-mcp");
        match self.approve {
            Approve::Ask => {}
            Approve::Yes => args.push("--approve".into()),
            Approve::No => args.push("--no-approve".into()),
        }
        push_flag(&mut args, self.offline, "--offline");

        push_list(&mut args, "--skill", &self.skills);
        push_flag(&mut args, self.no_skills, "--no-skills");
        push_list(&mut args, "-e", &self.extensions);
        push_flag(&mut args, self.no_extensions, "--no-extensions");
        push_list(&mut args, "--prompt-template", &self.prompt_templates);
        push_flag(&mut args, self.no_prompt_templates, "--no-prompt-templates");
        push_flag(&mut args, self.no_context_files, "--no-context-files");

        push_value(&mut args, "--system-prompt", &self.system_prompt);
        push_value(
            &mut args,
            "--append-system-prompt",
            &self.append_system_prompt,
        );

        args
    }

    pub fn needs_reconnect(&self, other: &Self) -> bool {
        self.target != other.target
            || self.bin != other.bin
            || self.remote_ssh != other.remote_ssh
            || self.remote_cmd != other.remote_cmd
            || self.workspace != other.workspace
            || self.session_id != other.session_id
            || self.session_dir != other.session_dir
            || self.no_session != other.no_session
            || self.start_last != other.start_last
            || self.name != other.name
            || self.model != other.model
            || self.models != other.models
            || self.provider != other.provider
            || self.thinking != other.thinking
            || self.tools != other.tools
            || self.exclude_tools != other.exclude_tools
            || self.no_tools != other.no_tools
            || self.no_builtin_tools != other.no_builtin_tools
            || self.no_mcp != other.no_mcp
            || self.approve != other.approve
            || self.offline != other.offline
            || self.skills != other.skills
            || self.no_skills != other.no_skills
            || self.extensions != other.extensions
            || self.no_extensions != other.no_extensions
            || self.prompt_templates != other.prompt_templates
            || self.no_prompt_templates != other.no_prompt_templates
            || self.no_context_files != other.no_context_files
            || self.system_prompt != other.system_prompt
            || self.append_system_prompt != other.append_system_prompt
    }

    pub fn remote_command(&self) -> String {
        let explicit = self.remote_cmd.trim();
        if !explicit.is_empty() {
            return explicit.to_string();
        }
        let args = self
            .args()
            .iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "cd {} && exec pi {}",
            shell_quote(self.effective_workspace()),
            args
        )
    }

    pub fn transport_label(&self) -> String {
        match self.target {
            Target::Local => format!("LOCAL · {}", self.effective_workspace()),
            Target::Remote => format!("REMOTE · {}", self.remote_ssh),
        }
    }

    fn known_entries(&self) -> Vec<(&'static str, &'static str, String)> {
        let bool_str = |value: bool| value.to_string();
        vec![
            ("pi", "target", self.target.as_str().to_string()),
            ("pi", "bin", self.bin.clone()),
            ("pi", "remote_ssh", self.remote_ssh.clone()),
            ("pi", "remote_cmd", self.remote_cmd.clone()),
            ("pi", "workspace", self.workspace.clone()),
            ("pi", "session_id", self.session_id.clone()),
            ("pi", "session_dir", self.session_dir.clone()),
            ("pi", "no_session", bool_str(self.no_session)),
            ("pi", "start_last", bool_str(self.start_last)),
            ("pi", "name", self.name.clone()),
            ("pi", "model", self.model.clone()),
            ("pi", "aux_model", self.aux_model.clone()),
            ("pi", "models", self.models.clone()),
            ("pi", "provider", self.provider.clone()),
            ("pi", "thinking", self.thinking.as_str().to_string()),
            ("pi", "tools", self.tools.clone()),
            ("pi", "exclude_tools", self.exclude_tools.clone()),
            ("pi", "no_tools", bool_str(self.no_tools)),
            ("pi", "no_builtin_tools", bool_str(self.no_builtin_tools)),
            ("pi", "no_mcp", bool_str(self.no_mcp)),
            ("pi", "approve", self.approve.as_str().to_string()),
            ("pi", "offline", bool_str(self.offline)),
            ("pi", "skills", self.skills.clone()),
            ("pi", "no_skills", bool_str(self.no_skills)),
            ("pi", "extensions", self.extensions.clone()),
            ("pi", "no_extensions", bool_str(self.no_extensions)),
            ("pi", "prompt_templates", self.prompt_templates.clone()),
            ("pi", "no_prompt_templates", bool_str(self.no_prompt_templates)),
            ("pi", "no_context_files", bool_str(self.no_context_files)),
            ("pi", "system_prompt", escape_text(&self.system_prompt)),
            (
                "pi",
                "append_system_prompt",
                escape_text(&self.append_system_prompt),
            ),
            ("ui", "view", self.view.as_str().to_string()),
            ("ui", "timestamps", bool_str(self.timestamps)),
            ("ui", "auto_scroll", bool_str(self.auto_scroll)),
            ("ui", "chat_width", self.chat_width.to_string()),
            ("ui", "font_scale", self.font_scale.to_string()),
            ("ui", "font", self.font.as_str().to_string()),
            ("ui", "show_costs", bool_str(self.show_costs)),
            ("ui", "notify", bool_str(self.notify)),
        ]
    }
}


fn push_value(args: &mut Vec<String>, flag: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        args.push(flag.to_string());
        args.push(value.to_string());
    }
}

fn push_flag(args: &mut Vec<String>, enabled: bool, flag: &str) {
    if enabled {
        args.push(flag.to_string());
    }
}

fn push_list(args: &mut Vec<String>, flag: &str, value: &str) {
    for item in value.split(':').map(str::trim).filter(|item| !item.is_empty()) {
        args.push(flag.to_string());
        args.push(item.to_string());
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_int(value: &str, min: i32, max: i32) -> Option<i32> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|parsed| (*parsed >= min) && (*parsed <= max))
}

fn escape_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n")
}

fn unescape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn render_preserving(text: &str, entries: &[(&'static str, &'static str, String)]) -> String {
    let mut values: HashMap<(&str, &str), &str> = HashMap::new();
    for (section, key, value) in entries {
        values.insert((section, key), value.as_str());
    }

    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut section = String::new();
    let mut present: HashSet<(String, String)> = HashSet::new();
    let mut end: HashMap<String, usize> = HashMap::new();

    for index in 0..lines.len() {
        let line = lines[index].trim().to_string();
        if let Some(name) = line.strip_prefix('[') {
            section = name
                .split(']')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            end.insert(section.clone(), index);
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some((key, _)) = line.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            if let Some(value) = values.get(&(section.as_str(), key.as_str())) {
                lines[index] = format!("{key}={value}");
                present.insert((section.clone(), key));
                if let Some(slot) = end.get_mut(&section) {
                    *slot = index;
                } else {
                    end.insert(section.clone(), index);
                }
            } else if section == "pi" || section == "ui" {
                if let Some(slot) = end.get_mut(&section) {
                    *slot = index;
                }
            }
        }
    }

    let mut missing: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
    for (section, key, value) in entries {
        if present.contains(&(section.to_string(), key.to_string())) {
            continue;
        }
        missing.entry(section).or_default().push((key, value.as_str()));
    }

    let mut inside: Vec<(usize, Vec<(&str, &str)>)> = Vec::new();
    let mut append: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
    for (section, items) in missing {
        match end.get(section) {
            Some(anchor) => inside.push((*anchor, items)),
            None => append.push((section, items)),
        }
    }
    inside.sort_by_key(|(anchor, _)| std::cmp::Reverse(*anchor));
    for (anchor, items) in inside {
        for (offset, (key, value)) in items.into_iter().enumerate() {
            lines.insert(anchor + 1 + offset, format!("{key}={value}"));
        }
    }
    append.sort_by_key(|(section, _)| if *section == "pi" { 0 } else { 1 });
    for (section, items) in append {
        if lines.last().is_some_and(|line| !line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(format!("[{section}]"));
        for (key, value) in items {
            lines.push(format!("{key}={value}"));
        }
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._@/-:+~".contains(c))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PI_DESKTOP_CONFIG") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("pi-desktop").join("config.ini"))
}

pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

pub fn pi_agent_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PI_DESKTOP_AGENT_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(dir) = std::env::var_os("PI_CODING_AGENT_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    home_file(".pi/agent")
}

pub fn expand_home(path: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if path == "~" {
        return home.unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

pub fn home_file(relative: &str) -> PathBuf {
    expand_home(&format!("~/{}", relative.trim_start_matches('/')))
}


