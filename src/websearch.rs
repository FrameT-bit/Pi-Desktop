
use std::io;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::config;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Bool,
    Text,
    Url,
    Choice(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub kind: Kind,
    pub secret: bool,
}

const FIRECRAWL_VERSIONS: &[&str] = &["v1", "v2"];

pub const FIELDS: &[Field] = &[
    Field {
        key: "enabled",
        label: "Web search",
        hint: "link the pi-web-access extension in this session",
        kind: Kind::Bool,
        secret: false,
    },
    Field {
        key: "searxngBaseUrl",
        label: "SearXNG instance",
        hint: "SearXNG instance (JSON API); empty shutdown",
        kind: Kind::Url,
        secret: false,
    },
    Field {
        key: "FirecrawlBaseUrl",
        label: "Firecrawl ( URL base)",
        hint: "Server URL: https://api.firecrawl.dev in the cloud or the URL of the self-hosted. It is not the key API.",
        kind: Kind::Url,
        secret: false,
    },
    Field {
        key: "FirecrawlApiKey",
        label: "Firecrawl (API key)",
        hint: "the only key of the Firecrawl API; the above field is only the address of the server.",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "FirecrawlApiVersion",
        label: "Firecrawl (version)",
        hint: "version of the API used by the scraper",
        kind: Kind::Choice(FIRECRAWL_VERSIONS),
        secret: false,
    },
    Field {
        key: "FirecrawlFreshScrape",
        label: "Firecrawl always fresh",
        hint: "ignore cache and scrape page again",
        kind: Kind::Bool,
        secret: false,
    },
    Field {
        key: "GoodApiKey",
        label: "Good (key)",
        hint: "key to Brave Search API",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "tavilyApiKey",
        label: "Tavily (key)",
        hint: "key to Tavily Search API",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "exaApiKey",
        label: "Exa (key)",
        hint: "key to Exa Search API",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "serverApiKey",
        label: "Serper (key)",
        hint: "Serper key (Google SERP)",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "jinaApiKey",
        label: "Jina (key)",
        hint: "key to Jina Reader",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "kagiApiKey",
        label: "Kagi (key)",
        hint: "key to Kagi Search API",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "perplexityApiKey",
        label: "Perplexity (key)",
        hint: "key of Perplexity (Sound)",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "deepseekApiKey",
        label: "DeepSeek (key)",
        hint: "DeepSeek key used in synthesis",
        kind: Kind::Text,
        secret: true,
    },
    Field {
        key: "mistralApiKey",
        label: "Mistral (key)",
        hint: "Mistral key used in synthesis",
        kind: Kind::Text,
        secret: true,
    },
];

pub fn field(key: &str) -> Option<&'static Field> {
    FIELDS.iter().find(|field| field.key == key)
}

pub fn path() -> PathBuf {
    config::pi_agent_dir().join("web-search.json")
}

fn migrate_legacy(destination: &std::path::Path) {
    if destination.exists() {
        return;
    }
    let mut candidates = vec![config::home_file(".pi/web-search.json")];
    if let Some(base) = std::env::var_os("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(base).join("pi").join("web-search.json"));
    }
    for candidate in candidates {
        if candidate == destination || !candidate.exists() {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&candidate) {
            if let Some(parent) = destination.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut tmp) = destination.as_os_str().to_owned().into_string() {
                tmp.push_str(".tmp");
                let tmp = PathBuf::from(tmp);
                if std::fs::write(&tmp, text).is_ok() {
                    let _ = std::fs::rename(&tmp, destination);
                }
            }
        }
        break;
    }
}

#[derive(Clone, Debug)]
pub struct WebSearch {
    doc: Value,
    path: PathBuf,
}

impl WebSearch {
    pub fn load() -> Self {
        let path = path();
        migrate_legacy(&path);
        Self::load_at(path)
    }

    pub fn load_at(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let doc = serde_json::from_str::<Value>(text.trim())
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        Self { doc, path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        self.doc
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    }

    pub fn get_bool(&self, key: &str) -> bool {
        self.doc
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    

    pub fn set_str(&mut self, key: &str, value: &str) {
        let value = value.trim();
        let Some(object) = self.doc.as_object_mut() else {
            return;
        };
        if value.is_empty() {
            object.remove(key);
        } else {
            object.insert(key.to_string(), json!(value));
        }
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        if let Some(object) = self.doc.as_object_mut() {
            object.insert(key.to_string(), json!(value));
        }
    }

    pub fn remove(&mut self, key: &str) {
        if let Some(object) = self.doc.as_object_mut() {
            object.remove(key);
        }
    }

    pub fn to_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.doc).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn save_atomic(&self) -> io::Result<()> {
        config::write_atomic(&self.path, &format!("{}\n", self.to_pretty()))
    }
}

pub fn validate_url(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(());
    }
    let rest = if let Some(rest) = value.strip_prefix("http://") {
        rest
    } else if let Some(rest) = value.strip_prefix("https://") {
        rest
    } else {
        return Err("you need to start with http:// or https://".to_string());
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err("not valid host".to_string());
    }
    Ok(())
}


