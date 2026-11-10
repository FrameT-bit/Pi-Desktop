
use std::process::{Command, Stdio};
use std::thread;

use serde_json::Value;

use crate::state::{CatalogItem, PluginItem, Transport};

const SEARCH_BASE: &str = "https://registry.npmjs.org/-/v1/search";
pub const PAGE_SIZE: usize = 100;

#[derive(Debug)]
pub enum CatalogEvent {
    Search(Result<(CatalogPage, bool), String>),
    Installed { name: String, result: Result<(), String> },
}

pub fn encode_query(query: &str) -> String {
    let mut out = String::new();
    for byte in query.trim().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn search_url(query: &str, from: usize) -> String {
    let encoded = encode_query(query);
    if encoded.is_empty() {
        format!("{SEARCH_BASE}?text=keywords:pi-package&size={PAGE_SIZE}&from={from}")
    } else {
        format!("{SEARCH_BASE}?text=keywords:pi-package%20{encoded}&size={PAGE_SIZE}&from={from}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogPage {
    pub items: Vec<CatalogItem>,
    pub has_more: bool,
}

pub fn parse_page(text: &str) -> Result<CatalogPage, String> {
    let parsed: Value =
        serde_json::from_str(text).map_err(|err| format!("invalid npm response: {err}"))?;
    let objects = parsed
        .get("objects")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_more = objects.len() >= PAGE_SIZE;
    let mut items = Vec::new();
    for object in objects {
        let package = object.get("package").unwrap_or(&object);
        let keywords = package.get("keywords").and_then(Value::as_array);
        let tagged = keywords.is_some_and(|keywords| {
            keywords.iter().any(|keyword| {
                keyword
                    .as_str()
                    .is_some_and(|keyword| keyword.eq_ignore_ascii_case("pi-package"))
            })
        });
        if !tagged {
            continue;
        }
        if let Some(item) = catalog_item(package, &object) {
            items.push(item);
        }
    }
    Ok(CatalogPage { items, has_more })
}

pub fn parse_search(text: &str) -> Result<Vec<CatalogItem>, String> {
    let parsed: Value =
        serde_json::from_str(text).map_err(|err| format!("invalid npm response: {err}"))?;
    let objects = parsed
        .get("objects")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(objects
        .iter()
        .filter_map(|object| catalog_item(object.get("package").unwrap_or(object), object))
        .collect())
}

fn catalog_item(package: &Value, object: &Value) -> Option<CatalogItem> {
    let name = package.get("name").and_then(Value::as_str)?.trim();
    if name.is_empty() {
        return None;
    }
    let field = |key: &str| {
        package
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let number = |key: &str| {
        object
            .get("downloads")
            .and_then(|downloads| downloads.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let keywords: Vec<String> = package
        .get("keywords")
        .and_then(Value::as_array)
        .map(|keywords| {
            keywords
                .iter()
                .filter_map(Value::as_str)
                .map(|keyword| keyword.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let kind = ["extension", "skill", "theme", "prompt"]
        .iter()
        .find(|kind| keywords.iter().any(|keyword| keyword == *kind))
        .copied()
        .unwrap_or("package")
        .to_string();
    let links = package.get("links");
    let link = |key: &str| {
        links
            .and_then(|links| links.get(key))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let npm_url = link("npm");
    let npm_url = if npm_url.is_empty() {
        format!("https://www.npmjs.com/package/{name}")
    } else {
        npm_url
    };
    let repo_url = normalize_repo(&link("repository"));
    let thumb = repo_thumb(&repo_url);
    let author = package
        .get("publisher")
        .and_then(|publisher| publisher.get("username"))
        .and_then(Value::as_str)
        .or_else(|| {
            package
                .get("maintainers")
                .and_then(Value::as_array)
                .and_then(|maintainers| maintainers.first())
                .and_then(|maintainer| maintainer.get("username"))
                .and_then(Value::as_str)
        })
        .unwrap_or("")
        .to_string();
    Some(CatalogItem {
        name: name.to_string(),
        version: field("version"),
        description: field("description"),
        date: field("date"),
        downloads: number("monthly"),
        downloads_weekly: number("weekly"),
        kind,
        author,
        npm_url,
        repo_url,
        homepage: link("homepage"),
        thumb,
    })
}

fn normalize_repo(value: &str) -> String {
    let mut url = value.trim();
    for prefix in ["git+", "git://", "ssh://git", "git"] {
        if let Some(rest) = url.strip_prefix(prefix) {
            url = rest;
            break;
        }
    }
    if url.starts_with("github.com/") {
        url = &url["github.com/".len()..];
    }
    if url.starts_with("git@github.com:") {
        url = &url["git@github.com:".len()..];
    }
    let url = url.trim_end_matches(".git");
    if url.contains("://") {
        url.to_string()
    } else if url.chars().filter(|ch| *ch == '/').count() == 1 {
        format!("https://github.com/{url}")
    } else {
        String::new()
    }
}

fn repo_thumb(repo: &str) -> String {
    let owner = repo
        .trim_end_matches('/')
        .split('/')
        .nth(3)
        .unwrap_or("");
    if owner.is_empty() {
        String::new()
    } else {
        format!("https://github.com/{owner}.png?size=64")
    }
}

pub fn preview(text: &str, max_chars: usize) -> String {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= max_chars {
        return clean;
    }
    let mut cut: String = clean.chars().take(max_chars.saturating_sub(1)).collect();
    if clean.chars().nth(max_chars.saturating_sub(1)) != Some(' ') {
        if let Some(space) = cut.rfind(' ') {
            if space >= max_chars / 2 {
                cut.truncate(space);
            }
        }
    }
    while cut.ends_with([' ', ',', ';', ':']) {
        cut.pop();
    }
    cut.push('…');
    cut
}

pub fn is_valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@/._-".contains(c))
}

pub fn normalize_package(name: &str) -> String {
    let name = name.trim();
    let name = name.strip_prefix("npm:").unwrap_or(name);
    name.to_ascii_lowercase()
}

pub fn is_installed(name: &str, installed: &[PluginItem]) -> bool {
    let target = normalize_package(name);
    !target.is_empty()
        && installed
            .iter()
            .any(|item| normalize_package(&item.spec) == target)
}

pub fn fetch_readme(name: &str) -> Result<String, String> {
    if !is_valid_package_name(name) {
        return Err("invalid package name".to_string());
    }
    let url = format!("https://registry.npmjs.org/{}", encode_query(name));
    let output = Command::new("curl")
        .args(["- It's all right.", "-m", "20", &url])
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("curl: {err}"))?;
    if !output.status.success() {
        return Err(format!("Curl came out with {}", output.status));
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("Invalid README: {err}"))?;
    let readme = value
        .get("readme")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .or_else(|| value.get("description").and_then(Value::as_str))
        .unwrap_or("");
    if readme.is_empty() {
        return Err("package without README".to_string());
    }
    Ok(readme.chars().take(200_000).collect())
}

fn search_blocking(query: &str, from: usize) -> Result<CatalogPage, String> {
    let url = search_url(query, from);
    let output = Command::new("curl")
        .args(["-s", "-m", "15", &url])
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("curl: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(format!(
            "curl: {}",
            if detail.is_empty() {
                format!("output {}", output.status)
            } else {
                detail.to_string()
            }
        ));
    }
    let body = String::from_utf8_lossy(&output.stdout);
    if body.trim().is_empty() {
        return Err("curl: empty answer".to_string());
    }
    parse_page(&body)
}

pub fn spawn_search(query: String, from: usize, tx: async_channel::Sender<CatalogEvent>) {
    thread::Builder::new()
        .name("pi-catalog-search".into())
        .spawn(move || {
            let _ = tx.send_blocking(CatalogEvent::Search(
                search_blocking(&query, from).map(|page| (page, from > 0)),
            ));
        })
        .ok();
}

pub fn spawn_install(
    transport: Transport,
    name: String,
    tx: async_channel::Sender<CatalogEvent>,
) {
    thread::Builder::new()
        .name("pi-catalog-install".into())
        .spawn(move || {
            if !is_valid_package_name(&name) {
                let _ = tx.send_blocking(CatalogEvent::Installed {
                    name,
                    result: Err("invalid package name".to_string()),
                });
                return;
            }
            let args = vec!["install".to_string(), format!("npm:{name}")];
            let result = match crate::plugins::run_pi(&transport, &args) {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => Err(crate::plugins::command_error(&output)),
                Err(err) => Err(err),
            };
            let _ = tx.send_blocking(CatalogEvent::Installed { name, result });
        })
        .ok();
}


