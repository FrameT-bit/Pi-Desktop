//! Model families and the Best / Balanced / Cheap tiers used to pick a
//! sub-agent model inside the same family as the parent session.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::config::SubagentTier;
use crate::state::ModelItem;

pub const ROLES: &[&str] = &[
    "scout",
    "worker",
    "reviewer",
    "oracle",
    "researcher",
    "delegate",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TieredModels {
    pub family: String,
    pub label: String,
    /// provider/id, ordered from cheapest to most capable.
    pub models: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FamilyChoice {
    pub best: String,
    pub balanced: String,
    pub cheap: String,
}

impl FamilyChoice {
    pub fn get(&self, tier: SubagentTier) -> &str {
        match tier {
            SubagentTier::Best => &self.best,
            SubagentTier::Balanced => &self.balanced,
            SubagentTier::Cheap => &self.cheap,
            SubagentTier::Off => "",
        }
    }

    fn set(&mut self, tier: SubagentTier, value: String) {
        match tier {
            SubagentTier::Best => self.best = value,
            SubagentTier::Balanced => self.balanced = value,
            SubagentTier::Cheap => self.cheap = value,
            SubagentTier::Off => {}
        }
    }
}

/// Vendor family of a provider/model pair. Providers that resell models
/// (openrouter, local servers) are grouped by the id prefix when it is known.
pub fn detect_family(provider: &str, id: &str) -> String {
    let provider = provider.trim().to_ascii_lowercase();
    let id = id.trim().to_ascii_lowercase();
    let id_prefix = id.split('/').next().unwrap_or("");

    for (key, family) in [
        ("openai", "openai"),
        ("chatgpt", "openai"),
        ("codex", "openai"),
        ("anthropic", "anthropic"),
        ("claude", "anthropic"),
        ("google", "google"),
        ("gemini", "google"),
        ("vertex", "google"),
        ("deepseek", "deepseek"),
        ("xai", "xai"),
        ("grok", "xai"),
        ("meta", "meta"),
        ("llama", "meta"),
        ("mistral", "mistral"),
        ("qwen", "qwen"),
        ("alibaba", "qwen"),
        ("moonshot", "moonshot"),
        ("kimi", "moonshot"),
        ("nvidia", "nvidia"),
        ("cohere", "cohere"),
    ] {
        if provider.contains(key) || id_prefix.contains(key) {
            return family.to_string();
        }
    }
    if provider.is_empty() {
        if id_prefix.is_empty() {
            "unknown".to_string()
        } else {
            id_prefix.to_string()
        }
    } else {
        provider
    }
}

pub fn family_label(family: &str) -> String {
    match family {
        "openai" => "OpenAI / ChatGPT".to_string(),
        "anthropic" => "Anthropic / Claude".to_string(),
        "google" => "Google / Gemini".to_string(),
        "deepseek" => "DeepSeek".to_string(),
        "xai" => "xAI / Grok".to_string(),
        "meta" => "Meta / Llama".to_string(),
        "mistral" => "Mistral".to_string(),
        "qwen" => "Qwen".to_string(),
        "moonshot" => "Moonshot / Kimi".to_string(),
        "nvidia" => "NVIDIA".to_string(),
        "cohere" => "Cohere".to_string(),
        other => other.to_string(),
    }
}

/// Relative capability/cost. Lower is cheaper; words that vendors use for
/// small models dominate so a reseller's odd naming still sorts sensibly.
pub fn capability_score(id: &str) -> i32 {
    let id = id.to_ascii_lowercase();
    let mut score = 50;
    for (token, weight) in [
        ("nano", -40),
        ("mini", -30),
        ("lite", -28),
        ("flash", -24),
        ("haiku", -22),
        ("tiny", -30),
        ("small", -20),
        ("instant", -18),
        ("spark", -25),
        ("luna", -20),
        ("fast", -16),
        ("turbo", -10),
        ("scout", -10),
        ("sonnet", 5),
        ("medium", 8),
        ("plus", 6),
        ("standard", 0),
        ("large", 22),
        ("pro", 24),
        ("ultra", 34),
        ("max", 30),
        ("opus", 34),
        ("reason", 20),
        ("reasoner", 22),
        ("sol", 26),
        ("120b", 22),
        ("70b", 16),
        ("32b", 8),
        ("14b", 2),
        ("9b", -6),
        ("8b", -8),
        ("7b", -8),
        ("4b", -14),
        ("2b", -18),
        ("1b", -22),
    ] {
        if id.contains(token) {
            score += weight;
        }
    }
    // Long context / date suffixes should not decide cost.
    score
}

/// Group models per family, cheapest first.
pub fn families(models: &[ModelItem]) -> Vec<TieredModels> {
    let mut groups: BTreeMap<String, Vec<(i32, String)>> = BTreeMap::new();
    for model in models {
        if model.provider.trim().is_empty() || model.id.trim().is_empty() {
            continue;
        }
        let family = detect_family(&model.provider, &model.id);
        let token = format!("{}/{}", model.provider, model.id);
        groups
            .entry(family)
            .or_default()
            .push((capability_score(&model.id), token));
    }
    let preferred_order = [
        "openai", "anthropic", "google", "deepseek", "xai", "meta", "mistral", "qwen",
    ];
    let mut out: Vec<TieredModels> = groups
        .into_iter()
        .map(|(family, mut entries)| {
            entries.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
            entries.dedup_by(|left, right| left.1 == right.1);
            TieredModels {
                label: family_label(&family),
                family,
                models: entries.into_iter().map(|(_, token)| token).collect(),
            }
        })
        .collect();
    out.sort_by_key(|family| {
        preferred_order
            .iter()
            .position(|name| *name == family.family)
            .unwrap_or(preferred_order.len())
    });
    out
}

/// Pick a distinct model per tier when possible: best = most capable,
/// cheap = least capable, balanced = the middle of the list.
pub fn suggest(family: &TieredModels, tier: SubagentTier) -> String {
    if family.models.is_empty() {
        return String::new();
    }
    let cheap = family.models.first().cloned().unwrap_or_default();
    let best = family.models.last().cloned().unwrap_or_default();
    match tier {
        SubagentTier::Cheap => cheap,
        SubagentTier::Best => best,
        SubagentTier::Balanced => {
            if family.models.len() <= 2 {
                best
            } else {
                family.models[family.models.len() / 2].clone()
            }
        }
        SubagentTier::Off => String::new(),
    }
}

pub fn parse_overrides(raw: &str) -> BTreeMap<String, FamilyChoice> {
    let mut out: BTreeMap<String, FamilyChoice> = BTreeMap::new();
    for entry in raw.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let Some((family, rest)) = entry.split_once('=') else {
            continue;
        };
        let family = family.trim().to_ascii_lowercase();
        if family.is_empty() {
            continue;
        }
        let mut choice = FamilyChoice::default();
        for part in rest.split(',') {
            let Some((tier, value)) = part.split_once(':') else {
                continue;
            };
            let value = value.trim().trim_matches('_').to_string();
            if let Some(tier) = SubagentTier::parse(tier) {
                choice.set(tier, value);
            }
        }
        out.insert(family, choice);
    }
    out
}

pub fn serialize_overrides(choices: &BTreeMap<String, FamilyChoice>) -> String {
    choices
        .iter()
        .map(|(family, choice)| {
            let value = |text: &str| {
                if text.trim().is_empty() {
                    "_".to_string()
                } else {
                    text.trim().to_string()
                }
            };
            format!(
                "{family}=best:{},balanced:{},cheap:{}",
                value(&choice.best),
                value(&choice.balanced),
                value(&choice.cheap)
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Fills missing tier suggestions without overwriting explicit user choices.
pub fn resolved_choices(
    families: &[TieredModels],
    raw: &str,
) -> BTreeMap<String, FamilyChoice> {
    let mut choices = parse_overrides(raw);
    for family in families {
        let entry = choices.entry(family.family.clone()).or_default();
        if entry.best.is_empty() {
            entry.best = suggest(family, SubagentTier::Best);
        }
        if entry.balanced.is_empty() {
            entry.balanced = suggest(family, SubagentTier::Balanced);
        }
        if entry.cheap.is_empty() {
            entry.cheap = suggest(family, SubagentTier::Cheap);
        }
    }
    choices
}

pub fn current_family(provider: &str, id: &str) -> String {
    detect_family(provider, id)
}

/// Models of one family split per provider, cheapest first.
fn family_by_provider(models: &[ModelItem], family: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<(i32, String)>> = BTreeMap::new();
    for model in models {
        if detect_family(&model.provider, &model.id) != family {
            continue;
        }
        out.entry(model.provider.clone())
            .or_default()
            .push((capability_score(&model.id), model.id.clone()));
    }
    out.into_iter()
        .map(|(provider, mut ids)| {
            ids.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
            ids.dedup_by(|left, right| left.1 == right.1);
            (
                provider,
                ids.into_iter().map(|(_, id)| id).collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn suggest_within(ids: &[String], tier: SubagentTier) -> String {
    match (tier, ids.len()) {
        (_, 0) => String::new(),
        (SubagentTier::Cheap, _) => ids.first().cloned().unwrap_or_default(),
        (SubagentTier::Best, _) => ids.last().cloned().unwrap_or_default(),
        (SubagentTier::Balanced, 1) | (SubagentTier::Balanced, 2) => {
            ids.last().cloned().unwrap_or_default()
        }
        (SubagentTier::Balanced, _) => ids[ids.len() / 2].clone(),
        (SubagentTier::Off, _) => String::new(),
    }
}

/// Writes the pi-subagents defaults so the extension resolves the chosen tier
/// without the desktop app being in the loop. Only the `subagents` keys are
/// touched; everything else in settings.json is preserved.
pub fn apply_to_settings(
    agent_dir: &std::path::Path,
    tier: SubagentTier,
    models: &[ModelItem],
    current_provider: &str,
    current_id: &str,
    raw_overrides: &str,
) -> Result<Option<String>, String> {
    let path = agent_dir.join("settings.json");
    let mut root: Value = match std::fs::read_to_string(&path) {
        Ok(text) if !text.trim().is_empty() => serde_json::from_str(&text)
            .map_err(|error| format!("settings.json is not valid JSON: {error}"))?,
        _ => json!({}),
    };
    if !root.is_object() {
        return Err("settings.json is not a JSON object".to_string());
    }
    if tier == SubagentTier::Off {
        // Do not guess which keys the user owns: leaving settings.json exactly
        // as it is keeps a hand-written subagents block intact.
        return Ok(None);
    }

    let groups = families(models);
    let choices = resolved_choices(&groups, raw_overrides);
    let family = current_family(current_provider, current_id);
    let by_provider = family_by_provider(models, &family);
    let chosen_token = choices
        .get(&family)
        .map(|choice| choice.get(tier).to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            groups
                .iter()
                .find(|group| group.family == family)
                .map(|group| suggest(group, tier))
        })
        .unwrap_or_default();
    let chosen_id = chosen_token
        .split_once('/')
        .map(|(_, id)| id.to_string())
        .unwrap_or_else(|| chosen_token.clone());
    // Prefer a model from the active provider so the child does not need a
    // cross-provider lookup; fall back to the family pick.
    let current_ids = by_provider.get(current_provider).or_else(|| {
        by_provider
            .iter()
            .find(|(provider, _)| provider.contains(current_provider) || current_provider.contains(provider.as_str()))
            .map(|(_, ids)| ids)
    });
    let picked_id = current_ids
        .and_then(|ids| {
            if ids.iter().any(|id| *id == chosen_id) {
                Some(chosen_id.clone())
            } else {
                Some(suggest_within(ids, tier))
            }
        })
        .filter(|value| !value.is_empty())
        .unwrap_or(chosen_id);
    if picked_id.trim().is_empty() {
        return Ok(None);
    }
    let picked = format!("{current_provider}/{picked_id}");
    let provider = current_provider.to_string();
    let id = picked_id;

    let subagents = root
        .as_object_mut()
        .expect("checked object")
        .entry("subagents")
        .or_insert_with(|| json!({}));
    if !subagents.is_object() {
        *subagents = json!({});
    }
    let subagents = subagents.as_object_mut().expect("object");
    subagents.insert("defaultModel".into(), json!(id));
    subagents.insert("defaultProvider".into(), json!(provider));

    // Per-family role overrides make the choice survive switching the parent
    // model to another family without re-saving the app settings.
    let by_provider = subagents
        .entry("agentOverridesByProvider")
        .or_insert_with(|| json!({}));
    if !by_provider.is_object() {
        *by_provider = json!({});
    }
    let by_provider = by_provider.as_object_mut().expect("object");
    for group in &groups {
        let Some(choice) = choices.get(&group.family) else {
            continue;
        };
        let family_choice = choice.get(tier);
        let providers = family_by_provider(models, &group.family);
        for (provider, ids) in providers {
            let id = match family_choice.split_once('/') {
                Some((chosen_provider, chosen)) if chosen_provider == provider => {
                    chosen.to_string()
                }
                _ => suggest_within(&ids, tier),
            };
            if id.is_empty() {
                continue;
            }
            let provider_entry = by_provider
                .entry(provider)
                .or_insert_with(|| json!({}));
        if !provider_entry.is_object() {
            *provider_entry = json!({});
        }
            let provider_entry = provider_entry.as_object_mut().expect("object");
            for role in ROLES {
                let role_entry = provider_entry
                    .entry((*role).to_string())
                    .or_insert_with(|| json!({}));
                if !role_entry.is_object() {
                    *role_entry = json!({});
                }
                role_entry
                    .as_object_mut()
                    .expect("object")
                    .insert("model".into(), json!(id.clone()));
            }
        }
    }

    write_atomic(&path, &root)?;
    Ok(Some(picked))
}

fn write_atomic(path: &std::path::Path, root: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(root)
        .map_err(|error| format!("cannot encode settings.json: {error}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, format!("{text}\n")).map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(provider: &str, id: &str) -> ModelItem {
        ModelItem {
            provider: provider.into(),
            id: id.into(),
            name: id.into(),
        }
    }

    #[test]
    fn vendors_are_grouped_even_behind_a_reseller() {
        assert_eq!(detect_family("openrouter", "openai/gpt-5"), "openai");
        assert_eq!(detect_family("openai-codex", "gpt-5.6-sol"), "openai");
        assert_eq!(detect_family("anthropic", "claude-opus-4"), "anthropic");
        assert_eq!(detect_family("google", "gemini-3-pro"), "google");
        assert_eq!(detect_family("local", "gemma-26b"), "local");
    }

    #[test]
    fn tiers_pick_cheap_middle_and_best_inside_a_family() {
        let list = [
            model("openai", "gpt-5-nano"),
            model("openai", "gpt-5-mini"),
            model("openai", "gpt-5"),
            model("openai", "gpt-5-pro"),
        ];
        let groups = families(&list);
        let openai = groups.iter().find(|group| group.family == "openai").unwrap();
        assert_eq!(suggest(openai, SubagentTier::Cheap), "openai/gpt-5-nano");
        assert_eq!(suggest(openai, SubagentTier::Best), "openai/gpt-5-pro");
        assert_eq!(suggest(openai, SubagentTier::Balanced), "openai/gpt-5");
    }

    #[test]
    fn overrides_round_trip_and_keep_explicit_values() {
        let raw = "openai=best:openai/gpt-5-pro,balanced:_,cheap:openai/gpt-5-nano";
        let choices = parse_overrides(raw);
        assert_eq!(choices["openai"].best, "openai/gpt-5-pro");
        assert_eq!(choices["openai"].cheap, "openai/gpt-5-nano");
        let again = serialize_overrides(&choices);
        assert_eq!(parse_overrides(&again), choices);
    }

    #[test]
    fn writes_subagent_defaults_without_touching_other_keys() {
        let dir = std::env::temp_dir().join(format!(
            "pi-desktop-families-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, r#"{"theme":"dark","subagents":{"defaultThinking":"high"}}"#).unwrap();
        let list = [model("openai", "gpt-5-mini"), model("openai", "gpt-5-pro")];
        let picked = apply_to_settings(
            &dir,
            SubagentTier::Cheap,
            &list,
            "openai",
            "gpt-5-pro",
            "",
        )
        .unwrap();
        assert_eq!(picked.as_deref(), Some("openai/gpt-5-mini"));
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["theme"], "dark");
        assert_eq!(saved["subagents"]["defaultThinking"], "high");
        assert_eq!(saved["subagents"]["defaultModel"], "gpt-5-mini");
        assert_eq!(saved["subagents"]["defaultProvider"], "openai");
        assert_eq!(
            saved["subagents"]["agentOverridesByProvider"]["openai"]["worker"]["model"],
            "gpt-5-mini"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn picks_the_active_providers_model_inside_the_family() {
        let dir = std::env::temp_dir().join(format!(
            "pi-desktop-families-provider-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let list = [
            model("openai", "gpt-5-mini"),
            model("openai", "gpt-5-pro"),
            model("openai-codex", "gpt-5.6-luna"),
            model("openai-codex", "gpt-5.6-sol"),
        ];
        let picked = apply_to_settings(
            &dir,
            SubagentTier::Cheap,
            &list,
            "openai-codex",
            "gpt-5.6-sol",
            "",
        )
        .unwrap();
        assert_eq!(picked.as_deref(), Some("openai-codex/gpt-5.6-luna"));
        let saved: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(saved["subagents"]["defaultProvider"], "openai-codex");
        assert_eq!(saved["subagents"]["defaultModel"], "gpt-5.6-luna");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn off_leaves_the_file_untouched() {
        let dir = std::env::temp_dir().join(format!(
            "pi-desktop-families-off-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let original = r#"{"subagents":{"defaultModel":"keep-me"}}"#;
        std::fs::write(&path, original).unwrap();
        let list = [model("openai", "gpt-5-mini")];
        let picked =
            apply_to_settings(&dir, SubagentTier::Off, &list, "openai", "gpt-5-mini", "").unwrap();
        assert!(picked.is_none());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cheap_words_beat_a_bigger_number_in_the_name() {
        assert!(capability_score("gpt-5-mini") < capability_score("gpt-5"));
        assert!(capability_score("claude-haiku-4") < capability_score("claude-opus-4"));
    }
}
