use serde::Deserialize;

pub const WIDGET_PREFIX: &str = "PI_SUBAGENT_ASYNC_JSON:";

#[derive(Debug, Deserialize)]
pub struct Snapshot {
    kind: String,
    version: u32,
    pub runs: Vec<Run>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub label: String,
    pub state: String,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    #[serde(default)]
    pub activity: Activity,
    #[serde(default)]
    pub children: Vec<Run>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub current_tool: Option<String>,
    pub turn_count: Option<u64>,
    pub tool_count: Option<u64>,
}

impl Run {
    pub fn detail(&self) -> String {
        let mut parts = Vec::new();
        if let Some(model) = &self.model { parts.push(model.clone()); }
        if let Some(level) = &self.thinking { parts.push(level.clone()); }
        if let Some(tool) = &self.activity.current_tool { parts.push(tool.clone()); }
        if let Some(turns) = self.activity.turn_count { parts.push(format!("{turns} turns")); }
        if let Some(tools) = self.activity.tool_count { parts.push(format!("{tools} tools")); }
        parts.join(" · ")
    }
}

/// The extension intentionally sends a machine-readable widget in RPC mode.
/// Keep this protocol out of the composer even if its payload is malformed.
pub fn from_widget(lines: &[String]) -> Option<Result<Snapshot, String>> {
    for line in lines {
        let clean = crate::extension_ui::strip_ansi(line);
        let Some(json) = clean.trim().strip_prefix(WIDGET_PREFIX) else { continue };
        return Some(parse(json));
    }
    None
}

fn parse(json: &str) -> Result<Snapshot, String> {
    if json.len() > 65536 { return Err("Sub-agent status exceeds the display limit".into()); }
    let snapshot: Snapshot = serde_json::from_str(json)
        .map_err(|_| "Sub-agent status is unavailable (invalid snapshot)".to_string())?;
    if snapshot.kind != "pi-subagents.async-status-snapshot" || snapshot.version != 1 {
        return Err("Sub-agent status uses an unsupported format".into());
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_decodes_nested_workers_and_activity() {
        let lines = vec![format!("{WIDGET_PREFIX}{}", serde_json::json!({
            "kind":"pi-subagents.async-status-snapshot", "version":1,
            "runs":[{"id":"run-1", "label":"worker", "state":"running", "children":[{
                "id":"step:0", "label":"worker", "state":"running", "model":"openai/gpt-6.1-sol", "thinking":"high",
                "activity":{"turnCount":17,"toolCount":17,"currentTool":"bash"}
            }]}]
        }))];
        let snapshot = from_widget(&lines).unwrap().unwrap();
        let child = &snapshot.runs[0].children[0];
        assert_eq!(child.id, "step:0");
        assert_eq!(child.detail(), "openai/gpt-6.1-sol · high · bash · 17 turns · 17 tools");
    }

    #[test]
    fn invalid_protocol_is_recognized_but_never_rendered_as_prose() {
        assert!(from_widget(&[format!("{WIDGET_PREFIX}{{broken")]).unwrap().is_err());
        assert!(from_widget(&["A regular extension widget".into()]).is_none());
    }
}
