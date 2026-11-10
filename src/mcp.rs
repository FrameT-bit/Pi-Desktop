
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpServer {
    pub name: String,
    pub scope: String,
    pub transport: String,
    pub state: String,
    pub enabled: bool,
    pub exposure: String,
    pub tools: Vec<String>,
    pub error: Option<String>,
}

impl McpServer {
    pub fn ok(&self) -> bool {
        self.error.is_none()
            && matches!(
                self.state.to_ascii_lowercase().as_str(),
                "ok" | "ready" | "connected" | "running"
            )
    }

    pub fn status(&self) -> &str {
        if self.state.trim().is_empty() {
            if self.ok() {
                "ok"
            } else {
                "error"
            }
        } else {
            &self.state
        }
    }
}

pub fn parse_list(text: &str) -> Vec<McpServer> {
    let Some(value) = parse_json(text) else {
        return Vec::new();
    };
    let Some(servers) = value.get("servers").and_then(Value::as_array) else {
        return Vec::new();
    };
    servers
        .iter()
        .map(|server| McpServer {
            name: string(server, "name"),
            scope: string(server, "scope"),
            transport: string(server, "transport"),
            state: string(server, "state"),
            enabled: server
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            exposure: string(server, "exposure"),
            tools: server
                .get("tools")
                .and_then(Value::as_array)
                .map(|tools| {
                    tools
                        .iter()
                        .filter_map(|tool| {
                            tool.as_str().map(str::to_string).or_else(|| {
                                tool.get("name")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            error: server
                .get("error")
                .and_then(Value::as_str)
                .filter(|error| !error.trim().is_empty())
                .map(str::to_string),
        })
        .collect()
}

pub fn parse_errors(text: &str) -> Vec<String> {
    let Some(value) = parse_json(text) else {
        return Vec::new();
    };
    value
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(|error| {
                    error.as_str().map(str::to_string).or_else(|| {
                        Some(string(error, "message")).filter(|message| !message.is_empty())
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_json(text: &str) -> Option<Value> {
    let text = text.trim();
    serde_json::from_str::<Value>(text).ok().or_else(|| {
        let start = text.find('{')?;
        let end = text.rfind('}')?;
        serde_json::from_str::<Value>(&text[start..=end]).ok()
    })
}

fn string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}


pub fn add_url_args(
    name: &str,
    url: &str,
    headers: &[(String, String)],
    bearer_env: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "mcp".to_string(),
        "add".to_string(),
        name.trim().to_string(),
        "--url".to_string(),
        url.trim().to_string(),
    ];
    for (key, value) in headers {
        if key.trim().is_empty() {
            continue;
        }
        args.push("--header".to_string());
        args.push(format!("{}={}", key.trim(), value.trim()));
    }
    if let Some(env) = bearer_env.map(str::trim).filter(|env| !env.is_empty()) {
        args.push("--bearer-token-env-var".to_string());
        args.push(env.to_string());
    }
    args
}

pub fn add_stdio_args(name: &str, envs: &[(String, String)], command: &[String]) -> Vec<String> {
    let mut args = vec![
        "mcp".to_string(),
        "add".to_string(),
        name.trim().to_string(),
    ];
    for (key, value) in envs {
        if key.trim().is_empty() {
            continue;
        }
        args.push("--env".to_string());
        args.push(format!("{}={}", key.trim(), value.trim()));
    }
    args.push("--".to_string());
    for item in command {
        if !item.trim().is_empty() {
            args.push(item.clone());
        }
    }
    args
}

pub fn login_args(name: &str) -> Vec<String> {
    vec![
        "mcp".to_string(),
        "login".to_string(),
        name.trim().to_string(),
    ]
}

pub fn remove_args(name: &str) -> Vec<String> {
    vec![
        "mcp".to_string(),
        "remove".to_string(),
        name.trim().to_string(),
    ]
}

pub fn parse_pairs(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| line.trim().split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

pub fn parse_command(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut touched = false;
    for ch in text.chars() {
        match quote {
            Some(active) if ch == active => {
                quote = None;
                touched = true;
            }
            Some(_) => current.push(ch),
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                touched = true;
            }
            None if ch.is_whitespace() => {
                if touched || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                    touched = false;
                }
            }
            None => current.push(ch),
        }
    }
    if touched || !current.is_empty() {
        out.push(current);
    }
    out
}


