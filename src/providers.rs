
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::config;

pub const BUILTINS: &[&str] = &[
    "amazon-bedrock",
    "anthropic",
    "ant-ling",
    "azure",
    "baseten",
    "cerebras",
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "deepseek",
    "fireworks",
    "github-copilot",
    "google",
    "google-vertex",
    "groq",
    "huggingface",
    "kimi-coding",
    "llamacpp",
    "local",
    "meta",
    "minimax",
    "minimax-cn",
    "mistral",
    "moonshotai",
    "moonshotai-cn",
    "nvidia",
    "openai",
    "openai-codex",
    "opencode",
    "opencode-go",
    "openrouter",
    "qwen-token-plan",
    "qwen-token-plan-cn",
    "qwen-token-plan-individual",
    "radius",
    "together",
    "typesafe",
    "vercel-ai-gateway",
    "xai",
    "xiaomi",
    "xiaomi-token-plan-ams",
    "xiaomi-token-plan-cn",
    "xiaomi-token-plan-sgp",
    "zai",
    "zai-coding-cn",
];

pub const APIS: &[&str] = &[
    "openai-completions",
    "openai-responses",
    "anthropic-messages",
    "google-generative-ai",
];

pub const DEFAULT_CONTEXT: i64 = 65536;
pub const DEFAULT_MAX_TOKENS: i64 = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub reasoning: bool,
    pub context_window: i64,
    pub max_tokens: i64,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            reasoning: false,
            context_window: DEFAULT_CONTEXT,
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Provider {
    pub name: String,
    pub base_url: String,
    pub api: String,
    pub api_key: String,
    pub compat_developer: bool,
    pub compat_reasoning: bool,
    pub models: Vec<Model>,
}

impl Provider {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            api: APIS[0].to_string(),
            models: vec![Model::default()],
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug)]
pub struct Providers {
    doc: Value,
}

impl Default for Providers {
    fn default() -> Self {
        Self {
            doc: json!({ "providers": {} }),
        }
    }
}

impl Providers {
    pub fn path(agent_dir: &Path) -> PathBuf {
        agent_dir.join("models.json")
    }

    pub fn load(agent_dir: &Path) -> Self {
        let text = std::fs::read_to_string(Self::path(agent_dir)).unwrap_or_default();
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Self {
        let doc = serde_json::from_str::<Value>(text.trim())
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({ "providers": {} }));
        let mut this = Self { doc };
        this.ensure_providers();
        this
    }

    fn ensure_providers(&mut self) {
        if !self.doc.get("providers").is_some_and(Value::is_object) {
            if let Some(object) = self.doc.as_object_mut() {
                object.insert("providers".to_string(), json!({}));
            } else {
                self.doc = json!({ "providers": {} });
            }
        }
    }

    fn map(&self) -> &Map<String, Value> {
        self.doc
            .get("providers")
            .and_then(Value::as_object)
            .expect("providers are always object")
    }

    fn map_mut(&mut self) -> &mut Map<String, Value> {
        self.doc
            .get_mut("providers")
            .and_then(Value::as_object_mut)
            .expect("providers are always object")
    }

    pub fn names(&self) -> Vec<String> {
        self.map().keys().cloned().collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.map().contains_key(name)
    }

    pub fn provider(&self, name: &str) -> Option<Provider> {
        let value = self.map().get(name)?;
        Some(parse_provider(name, value))
    }

    pub fn all(&self) -> Vec<Provider> {
        self.map()
            .iter()
            .map(|(name, value)| parse_provider(name, value))
            .collect()
    }

    pub fn upsert(&mut self, provider: &Provider) {
        let value = provider_json(provider);
        match self.map_mut().get_mut(&provider.name) {
            Some(Value::Object(object)) => {
                for (key, new) in value.as_object().expect("object").iter() {
                    if key == "compat" {
                        match object.get_mut("compat").and_then(Value::as_object_mut) {
                            Some(compat) => {
                                for (ck, cv) in new.as_object().expect("object").iter() {
                                    compat.insert(ck.clone(), cv.clone());
                                }
                            }
                            None => {
                                object.insert(key.clone(), new.clone());
                            }
                        }
                    } else {
                        object.insert(key.clone(), new.clone());
                    }
                }
            }
            _ => {
                self.map_mut().insert(provider.name.clone(), value);
            }
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        self.map_mut().remove(name).is_some()
    }

    pub fn to_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.doc).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn save_atomic(&self, agent_dir: &Path) -> io::Result<()> {
        config::write_atomic(&Self::path(agent_dir), &format!("{}\n", self.to_pretty()))
    }
}

fn parse_provider(name: &str, value: &Value) -> Provider {
    let string = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let compat = value.get("compat");
    let compat_bool = |key: &str, default: bool| {
        compat
            .and_then(|compat| compat.get(key))
            .and_then(Value::as_bool)
            .unwrap_or(default)
    };
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .map(|model| Model {
                    id: model
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    name: model
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    reasoning: model
                        .get("reasoning")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    context_window: model
                        .get("contextWindows")
                        .and_then(Value::as_i64)
                        .unwrap_or(DEFAULT_CONTEXT),
                    max_tokens: model
                        .get("maxTokens")
                        .and_then(Value::as_i64)
                        .unwrap_or(DEFAULT_MAX_TOKENS),
                })
                .collect()
        })
        .unwrap_or_default();
    Provider {
        name: name.to_string(),
        base_url: string("baseUrl"),
        api: {
            let api = string("api");
            if api.is_empty() {
                APIS[0].to_string()
            } else {
                api
            }
        },
        api_key: string("apiKey"),
        compat_developer: compat_bool("SupportsDeveloperRole", false),
        compat_reasoning: compat_bool("SupportsReasoningEffort", false),
        models,
    }
}

fn provider_json(provider: &Provider) -> Value {
    let models: Vec<Value> = provider
        .models
        .iter()
        .map(|model| {
            json!({
                "id": model.id,
                "name": if model.name.trim().is_empty() { model.id.clone() } else { model.name.clone() },
                "reasoning": model.reasoning,
                "contextWindows": model.context_window,
                "maxTokens": model.max_tokens,
            })
        })
        .collect();
    json!({
        "baseUrl": provider.base_url,
        "api": provider.api,
        "apiKey": provider.api_key,
        "compat": {
            "SupportsDeveloperRole": provider.compat_developer,
            "SupportsReasoningEffort": provider.compat_reasoning,
        },
        "models": models,
    })
}


pub fn validate_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("empty provider name".to_string());
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err("provider name: use only [A-Za-z0-9._-]".to_string());
    }
    if BUILTINS.contains(&name) {
        return Err(format!(
            "`{name}` is built-in PI provider; choose another name"
        ));
    }
    Ok(())
}

pub fn validate_url(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("baseUrl empty".to_string());
    }
    if !(value.starts_with("http://") || value.starts_with("https://")) {
        return Err("baseUrl needs to start with http:// or https://".to_string());
    }
    let rest = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .unwrap_or("");
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err("baseUrl without valid host".to_string());
    }
    Ok(())
}

pub fn validate(provider: &Provider) -> Result<(), String> {
    validate_name(&provider.name)?;
    validate_url(&provider.base_url)?;
    if !APIS.contains(&provider.api.as_str()) {
        return Err(format!(
            "unknown provider: `{}` (use {})",
            provider.api,
            APIS.join(", ")
        ));
    }
    if provider.models.is_empty() {
        return Err("the provider needs at least 1 model".to_string());
    }
    let mut ids = BTreeSet::new();
    for model in &provider.models {
        if model.id.trim().is_empty() {
            return Err("every model needs an id".to_string());
        }
        if !ids.insert(model.id.trim().to_string()) {
            return Err(format!("repeated model: {}", model.id));
        }
        if model.context_window <= 0 {
            return Err(format!("{}: contextWindow must be > 0", model.id));
        }
        if model.max_tokens <= 0 {
            return Err(format!("{}: maxTokens must be > 0", model.id));
        }
    }
    Ok(())
}

pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    if chars.len() <= 4 {
        return "••••".to_string();
    }
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("•••{tail}")
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownModel {
    pub provider: String,
    pub id: String,
    pub name: String,
}

pub fn known_models(agent_dir: &Path) -> Vec<KnownModel> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();

    let mut push = |provider: &str, id: &str, name: &str| {
        let id = id.trim();
        if id.is_empty() || !seen.insert(format!("{provider}/{id}")) {
            return;
        }
        out.push(KnownModel {
            provider: provider.to_string(),
            id: id.to_string(),
            name: name.trim().to_string(),
        });
    };

    let store = std::fs::read_to_string(agent_dir.join("models-store.json")).unwrap_or_default();
    if let Ok(Value::Object(providers)) = serde_json::from_str::<Value>(store.trim()) {
        for (provider, value) in providers {
            let models = value
                .get("models")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for model in models {
                let id = model.get("id").and_then(Value::as_str).unwrap_or("");
                let name = model.get("name").and_then(Value::as_str).unwrap_or("");
                push(&provider, id, name);
            }
        }
    }

    for provider in Providers::load(agent_dir).all() {
        for model in &provider.models {
            push(&provider.name, &model.id, &model.name);
        }
    }

    out
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEntry {
    pub name: String,
    pub api_key: String,
}

#[derive(Clone, Debug)]
pub struct ProviderKeys {
    doc: Value,
}

impl Default for ProviderKeys {
    fn default() -> Self {
        Self { doc: json!({}) }
    }
}

impl ProviderKeys {
    pub fn path(agent_dir: &Path) -> PathBuf {
        agent_dir.join("provider-keys.json")
    }

    pub fn load(agent_dir: &Path) -> Self {
        let text = std::fs::read_to_string(Self::path(agent_dir)).unwrap_or_default();
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Self {
        let doc = serde_json::from_str::<Value>(text.trim())
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        Self { doc }
    }

    fn map(&self) -> &Map<String, Value> {
        self.doc.as_object().expect("document is always object")
    }

    fn map_mut(&mut self) -> &mut Map<String, Value> {
        self.doc.as_object_mut().expect("document is always object")
    }

    pub fn providers(&self) -> Vec<String> {
        self.map().keys().cloned().collect()
    }

    pub fn keys(&self, provider: &str) -> Vec<KeyEntry> {
        self.map()
            .get(provider)
            .and_then(|value| value.get("keys"))
            .and_then(Value::as_array)
            .map(|keys| {
                keys.iter()
                    .map(|key| KeyEntry {
                        name: key
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        api_key: key
                            .get("apiKey")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn active(&self, provider: &str) -> Option<String> {
        self.map()
            .get(provider)
            .and_then(|value| value.get("ActiveKey"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    pub fn add(&mut self, provider: &str, name: &str, api_key: &str) -> Result<(), String> {
        if provider.trim().is_empty() {
            return Err("empty provider".to_string());
        }
        if name.trim().is_empty() {
            return Err("empty key name".to_string());
        }
        if api_key.trim().is_empty() {
            return Err("empty key".to_string());
        }
        let entry = json!({ "name": name.trim(), "apiKey": api_key.trim() });
        match self.map_mut().get_mut(provider) {
            Some(Value::Object(object)) => {
                let keys = object
                    .entry("keys")
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .ok_or_else(|| "invalid key field at provider-keys.json".to_string())?;
                if keys.iter().any(|key| {
                    key.get("name").and_then(Value::as_str) == Some(name.trim())
                }) {
                    return Err(format!("a key named `{}` already exists in {provider}", name.trim()));
                }
                keys.push(entry);
                let has_active = object
                    .get("ActiveKey")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty());
                if !has_active {
                    object.insert("ActiveKey".to_string(), json!(name.trim()));
                }
            }
            _ => {
                self.map_mut().insert(
                    provider.to_string(),
                    json!({
                        "ActiveKey": name.trim(),
                        "keys": [entry],
                    }),
                );
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, provider: &str, name: &str) -> bool {
        let Some(Value::Object(object)) = self.map_mut().get_mut(provider) else {
            return false;
        };
        let Some(keys) = object.get_mut("keys").and_then(Value::as_array_mut) else {
            return false;
        };
        let before = keys.len();
        keys.retain(|key| key.get("name").and_then(Value::as_str) != Some(name));
        let removed = keys.len() != before;
        if removed {
            let next = keys
                .first()
                .and_then(|key| key.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let active_is_gone =
                object.get("ActiveKey").and_then(Value::as_str) == Some(name);
            if active_is_gone {
                match next {
                    Some(next) => {
                        object.insert("ActiveKey".to_string(), json!(next));
                    }
                    None => {
                        object.remove("ActiveKey");
                    }
                }
            }
        }
        removed
    }

    pub fn activate(&mut self, provider: &str, name: &str) -> bool {
        let Some(Value::Object(object)) = self.map_mut().get_mut(provider) else {
            return false;
        };
        let exists = object
            .get("keys")
            .and_then(Value::as_array)
            .is_some_and(|keys| {
                keys.iter()
                    .any(|key| key.get("name").and_then(Value::as_str) == Some(name))
            });
        if !exists {
            return false;
        }
        object.insert("ActiveKey".to_string(), json!(name));
        true
    }

    pub fn to_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.doc).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn save_atomic(&self, agent_dir: &Path) -> io::Result<()> {
        config::write_atomic(&Self::path(agent_dir), &format!("{}\n", self.to_pretty()))
    }
}


pub struct KnownProvider {
    pub id: &'static str,
    pub label: &'static str,
    pub env: &'static str,
    pub hint: &'static str,
}

pub const KNOWN: &[KnownProvider] = &[
    KnownProvider { id: "deepseek", label: "DeepSeek", env: "DEEPSEEK_API_KEY", hint: "platform.deepseek.com/api_keys" },
    KnownProvider { id: "openrouter", label: "OpenRouter", env: "OPENROUTER_API_KEY", hint: "openrouter.ai/keys" },
    KnownProvider { id: "groq", label: "Groq", env: "GROQ_API_KEY", hint: "console.groq.com/keys" },
    KnownProvider { id: "mistral", label: "Mistral", env: "MISTRAL_API_KEY", hint: "console.mistral.ai/api-keys" },
    KnownProvider { id: "openai", label: "OpenAI", env: "OPENAI_API_KEY", hint: "platform.openai.com/api-keys" },
    KnownProvider { id: "anthropic", label: "Anthropic", env: "ANTHROPIC_API_KEY", hint: "console.anthropic.com/settings/keys" },
    KnownProvider { id: "google", label: "Google Gemini", env: "GEMINI_API_KEY", hint: "aistudio.google.com/apikey" },
    KnownProvider { id: "xai", label: "xAI", env: "XAI_API_KEY", hint: "console.x.ai" },
    KnownProvider { id: "cerebras", label: "Cerebras", env: "CEREBRAS_API_KEY", hint: "cloud.cerebras.ai" },
    KnownProvider { id: "nvidia", label: "NVIDIA NIM", env: "NVIDIA_API_KEY", hint: "build.nvidia.com" },
    KnownProvider { id: "fireworks", label: "Fireworks", env: "FIREWORKS_API_KEY", hint: "fireworks.ai/account/api-keys" },
    KnownProvider { id: "together", label: "Together AI", env: "TOGETHER_API_KEY", hint: "api.together.ai/settings/api-keys" },
    KnownProvider { id: "baseten", label: "Baseten", env: "BASETEN_API_KEY", hint: "baseten.co" },
    KnownProvider { id: "moonshotai", label: "Moonshot / Kimi (global)", env: "MOONSHOT_API_KEY", hint: "platform.moonshot.ai" },
    KnownProvider { id: "moonshotai-cn", label: "Moonshot / Kimi (China)", env: "MOONSHOT_API_KEY", hint: "platform.moonshot.cn" },
    KnownProvider { id: "kimi-coding", label: "Kimi for Coding", env: "KIMI_API_KEY", hint: "kimi.com" },
    KnownProvider { id: "zai", label: "Z.AI Coding Plan (global)", env: "ZAI_API_KEY", hint: "z.ai" },
    KnownProvider { id: "zai-coding-cn", label: "Z.AI Coding Plan (China)", env: "ZAI_CODING_CN_API_KEY", hint: "z.ai" },
    KnownProvider { id: "minimax", label: "MiniMax", env: "MINIMAX_API_KEY", hint: "platform.minimax.io" },
    KnownProvider { id: "minimax-cn", label: "MiniMax (China)", env: "MINIMAX_CN_API_KEY", hint: "platform.minimaxi.com" },
    KnownProvider { id: "qwen-token-plan", label: "Qwen Token Plan", env: "QWEN_TOKEN_PLAN_API_KEY", hint: "qwen.ai" },
    KnownProvider { id: "qwen-token-plan-cn", label: "Qwen Token Plan (China)", env: "QWEN_TOKEN_PLAN_CN_API_KEY", hint: "qwen.ai" },
    KnownProvider { id: "qwen-token-plan-individual", label: "Qwen Token Individual Plan", env: "QWEN_TOKEN_PLAN_API_KEY", hint: "qwen.ai" },
    KnownProvider { id: "opencode", label: "OpenCode Zen", env: "OPENCODE_API_KEY", hint: "opencode.ai/auth" },
    KnownProvider { id: "opencode-go", label: "OpenCode Go", env: "OPENCODE_API_KEY", hint: "opencode.ai/auth" },
    KnownProvider { id: "vercel-ai-gateway", label: "Vercel AI Gateway", env: "AI_GATEWAY_API_KEY", hint: "vercel.com/ai-gateway" },
    KnownProvider { id: "huggingface", label: "Hugging Face", env: "HF_TOKEN", hint: "huggingface.co/settings/tokens" },
    KnownProvider { id: "github-copilot", label: "GitHub Copilot", env: "COPILOT_GITHUB_TOKEN", hint: "github.com/settings/copilot" },
    KnownProvider { id: "ant-ling", label: "Ant Ling", env: "ANT_LING_API_KEY", hint: "provider console" },
    KnownProvider { id: "meta", label: "Meta", env: "META_API_KEY", hint: "meta.com" },
    KnownProvider { id: "xiaomi", label: "Xiaomi MiMo", env: "XIAOMI_API_KEY", hint: "xiaomi.com" },
    KnownProvider { id: "xiaomi-token-plan-cn", label: "Xiaomi MiMo Token Plan (China)", env: "XIAOMI_TOKEN_PLAN_CN_API_KEY", hint: "xiaomi.com" },
    KnownProvider { id: "xiaomi-token-plan-ams", label: "Xiaomi MiMo Token Plan (Amsterdam)", env: "XIAOMI_TOKEN_PLAN_AMS_API_KEY", hint: "xiaomi.com" },
    KnownProvider { id: "xiaomi-token-plan-sgp", label: "Xiaomi MiMo Token Plan (Singapore)", env: "XIAOMI_TOKEN_PLAN_SGP_API_KEY", hint: "xiaomi.com" },
    KnownProvider { id: "radius", label: "Radius", env: "RADIUS_API_KEY", hint: "radius.pi.dev" },
];

pub fn known(id: &str) -> Option<&'static KnownProvider> {
    KNOWN.iter().find(|provider| provider.id == id)
}


pub struct AuthStore {
    doc: Value,
}

impl Default for AuthStore {
    fn default() -> Self {
        Self { doc: json!({}) }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Credential {
    None,
    ApiKey(String),
    Other(String),
}

impl AuthStore {
    pub fn path(agent_dir: &Path) -> PathBuf {
        agent_dir.join("auth.json")
    }

    pub fn load(agent_dir: &Path) -> Self {
        let path = Self::path(agent_dir);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        if text.trim().is_empty() {
            return Self::default();
        }
        match serde_json::from_str::<Value>(text.trim()) {
            Ok(Value::Object(object)) => Self {
                doc: Value::Object(object),
            },
            _ => {
                let backup = agent_dir.join("auth.json.invalid");
                if !backup.exists() {
                    let _ = std::fs::copy(&path, &backup);
                }
                Self::default()
            }
        }
    }

    pub fn parse(text: &str) -> Self {
        let doc = serde_json::from_str::<Value>(text.trim())
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        Self { doc }
    }

    pub fn credential(&self, provider: &str) -> Credential {
        let Some(entry) = self.doc.get(provider).and_then(Value::as_object) else {
            return Credential::None;
        };
        let kind = entry.get("type").and_then(Value::as_str).unwrap_or("");
        let key = entry
            .get("key")
            .or_else(|| entry.get("apiKey"))
            .and_then(Value::as_str)
            .unwrap_or("");
        match kind {
            "api_key" | "api-key" if !key.is_empty() => Credential::ApiKey(key.to_string()),
            "" if !key.is_empty() => Credential::ApiKey(key.to_string()),
            _ => Credential::Other(if kind.is_empty() { "credential" } else { kind }.to_string()),
        }
    }

    pub fn set_api_key(&mut self, provider: &str, key: &str) {
        let object = self.doc.as_object_mut().expect("auth is always an object");
        let entry = object
            .entry(provider.to_string())
            .or_insert_with(|| json!({}));
        if !entry.is_object() {
            *entry = json!({});
        }
        let entry = entry.as_object_mut().expect("entry is object");
        entry.insert("type".to_string(), json!("api_key"));
        entry.insert("key".to_string(), json!(key.trim()));
    }

    pub fn remove_api_key(&mut self, provider: &str) -> bool {
        let removable = self
            .doc
            .get(provider)
            .and_then(Value::as_object)
            .is_some_and(|entry| {
                let kind = entry.get("type").and_then(Value::as_str).unwrap_or("");
                let looks_oauth = entry.contains_key("access")
                    || entry.contains_key("refresh")
                    || matches!(kind, "oauth" | "token");
                !looks_oauth && matches!(kind, "" | "api_key" | "api-key")
            });
        if !removable {
            return false;
        }
        self.doc
            .as_object_mut()
            .is_some_and(|object| object.remove(provider).is_some())
    }

    pub fn providers(&self) -> Vec<String> {
        self.doc
            .as_object()
            .map(|object| object.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn to_pretty(&self) -> String {
        format!("{}\n", serde_json::to_string_pretty(&self.doc).unwrap_or_default())
    }

    pub fn save_atomic(&self, agent_dir: &Path) -> io::Result<()> {
        let path = Self::path(agent_dir);
        crate::config::write_atomic(&path, &self.to_pretty())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliModel {
    pub provider: String,
    pub id: String,
    pub context: String,
    pub max_out: String,
    pub thinking: bool,
    pub images: bool,
}

pub fn cli_models(bin: &str, provider: &str) -> Result<Vec<CliModel>, String> {
    let path = crate::state::resolve_pi_bin(bin)?;
    let output = std::process::Command::new(&path)
        .args(["--list-models", provider])
        .env("PI_CODING_AGENT_DIR", crate::config::pi_agent_dir())
        .env("PATH", crate::state::path_with_pi(&path))
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("failed to execute Pi: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }
    Ok(parse_cli_models(
        &String::from_utf8_lossy(&output.stdout),
        provider,
    ))
}

pub fn parse_cli_models(text: &str, provider: &str) -> Vec<CliModel> {
    let mut out = Vec::new();
    for line in text.lines().skip_while(|line| !line.starts_with("provider ")) {
        if line.starts_with("provider ") {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 6 {
            continue;
        }
        if fields[0] != provider {
            continue;
        }
        out.push(CliModel {
            provider: fields[0].to_string(),
            id: fields[1].to_string(),
            context: fields[2].to_string(),
            max_out: fields[3].to_string(),
            thinking: fields[4] == "yes",
            images: fields[5] == "yes",
        });
    }
    out
}

pub fn set_default_model(agent_dir: &Path, provider: &str, model: &str) -> io::Result<()> {
    let path = agent_dir.join("settings.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".to_string());
    let mut doc = serde_json::from_str::<Value>(text.trim())
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let object = doc.as_object_mut().expect("settings is object");
    object.insert("defaultProvider".to_string(), json!(provider));
    object.insert("defaultModel".to_string(), json!(model));
    let pretty = format!("{}\n", serde_json::to_string_pretty(&doc).unwrap_or_default());
    crate::config::write_atomic(&path, &pretty)
}

pub fn default_model(agent_dir: &Path) -> (Option<String>, Option<String>) {
    let text = std::fs::read_to_string(agent_dir.join("settings.json")).unwrap_or_default();
    let doc = serde_json::from_str::<Value>(text.trim()).unwrap_or(Value::Null);
    let provider = doc
        .get("defaultProvider")
        .and_then(Value::as_str)
        .map(str::to_string);
    let model = doc
        .get("defaultModel")
        .and_then(Value::as_str)
        .map(str::to_string);
    (provider, model)
}


pub fn repair_models_store(agent_dir: &Path) -> Result<Option<String>, String> {
    let path = agent_dir.join("models-store.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    if serde_json::from_str::<Value>(raw.trim()).is_ok() {
        return Ok(None);
    }
    let fixed = repair_missing_commas(&raw).ok_or_else(|| {
        "invalid models-store.json: I can only repair a missing comma — delete the file so Pi can recreate it".to_string()
    })?;
    if serde_json::from_str::<Value>(fixed.trim()).is_err() {
        return Err("invalid models-store.json: repair did not produce valid JSON".to_string());
    }
    let stamp = glib_stamp();
    let backup = agent_dir.join(format!("models-store.json.bak-{stamp}"));
    std::fs::copy(&path, &backup)
        .map_err(|err| format!("failed to create catalog backup: {err}"))?;
    crate::config::write_atomic(&path, &fixed)
        .map_err(|err| format!("failed to write models-store.json: {err}"))?;
    Ok(Some(format!(
        "models-store.json was invalid (missing comma) — repaired; backup: {}",
        backup.display()
    )))
}

fn repair_missing_commas(text: &str) -> Option<String> {
    let mut fixed = text.to_string();
    for _ in 0..64 {
        match serde_json::from_str::<Value>(&fixed) {
            Ok(_) => return Some(fixed),
            Err(err) if err.to_string().contains("expected,") => {
                let position = offset_position(&fixed, err.line(), err.column());
                let mut position = position.min(fixed.len());
                while position > 0 && !fixed.is_char_boundary(position) {
                    position -= 1;
                }
                fixed.insert(position, ',');
            }
            Err(_) => return None,
        }
    }
    None
}

fn offset_position(text: &str, line: usize, column: usize) -> usize {
    let mut current_line = 1;
    let mut offset = 0;
    for segment in text.split_inclusive('\n') {
        if current_line == line {
            let within = column.saturating_sub(1).min(segment.len());
            return offset + within;
        }
        offset += segment.len();
        current_line += 1;
    }
    text.len()
}

fn glib_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}


pub fn models_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}

pub fn models_url_for(base: &str, api: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if api == "google-generative-ai" {
        if base.ends_with("/v1beta") || base.ends_with("/v1") {
            format!("{base}/models")
        } else {
            format!("{base}/v1beta/models")
        }
    } else {
        models_url(base)
    }
}

pub fn discover_models(base_url: &str, api: &str, api_key: &str) -> Result<Vec<String>, String> {
    validate_url(base_url)?;
    let url = models_url_for(base_url, api);
    let mut command = std::process::Command::new("curl");
    command.args(["-sS", "--max-time", "12"]);
    if api == "anthropic-messages" {
        command.args(["-H", "anthropic-version: 2023-06-01"]);
        if !api_key.trim().is_empty() {
            command.args(["-H", &format!("x-api-key: {}", api_key.trim())]);
        }
    } else if api == "google-generative-ai" {
        if !api_key.trim().is_empty() {
            command.args(["-H", &format!("x-goog-api-key: {}", api_key.trim())]);
        }
    } else if !api_key.trim().is_empty() {
        command.args(["-H", &format!("Authorization: Bearer {}", api_key.trim())]);
    }
    command.args(["-w", "\n%{http_code}", &url]);
    let output = command
        .output()
        .map_err(|err| format!("Curl failure: {err}"))?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let (body, status) = text.rsplit_once('\n').unwrap_or((text.as_str(), ""));
    if !status.starts_with('2') {
        return Err(format!("HTTP {status}: {}", body.trim().chars().take(160).collect::<String>()));
    }
    let models = if api == "google-generative-ai" {
        parse_google_model_ids(body)
    } else {
        parse_model_ids(body)
    };
    if models.is_empty() {
        return Err("the endpoint answered without models".to_string());
    }
    Ok(models)
}

pub fn parse_google_model_ids(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(list) = value.get("models").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out: Vec<String> = list
        .iter()
        .filter(|item| {
            item.get("supportedGenerationMethods")
                .and_then(Value::as_array)
                .is_none_or(|methods| {
                    methods
                        .iter()
                        .any(|method| method.as_str() == Some("generatedContent"))
                })
        })
        .filter_map(|item| {
            let name = item.get("name").and_then(Value::as_str)?;
            Some(name.strip_prefix("models/").unwrap_or(name).to_string())
        })
        .filter(|id| !id.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

pub fn parse_model_ids(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let list = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.get("models").and_then(Value::as_array))
        .or_else(|| value.as_array());
    let Some(list) = list else {
        return Vec::new();
    };
    let mut out: Vec<String> = list
        .iter()
        .filter_map(|item| match item {
            Value::String(id) => Some(id.clone()),
            Value::Object(map) => ["id", "name", "model"]
                .iter()
                .find_map(|key| map.get(*key).and_then(Value::as_str))
                .map(str::to_string),
            _ => None,
        })
        .filter(|id| !id.trim().is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

