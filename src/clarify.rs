//! The `pi-clarify` convention: the model ends a turn with a fenced JSON
//! block instead of guessing. The desktop client renders it as a question card
//! with multiple-choice answers and an always-available free-text field.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clarify {
    pub question: String,
    pub options: Vec<String>,
    pub multiselect: bool,
}

pub const FENCE: &str = "pi-clarify";

/// Removes the first clarify block from the prose and returns it.
pub fn extract(text: &str) -> (String, Option<Clarify>) {
    let mut out: Vec<&str> = Vec::new();
    let mut found: Option<Clarify> = None;
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if let Some(tag) = trimmed.strip_prefix("```") {
            if tag.trim().eq_ignore_ascii_case(FENCE) {
                let mut payload = String::new();
                for inner in lines.by_ref() {
                    if inner.trim_start().starts_with("```") {
                        break;
                    }
                    payload.push_str(inner);
                    payload.push('\n');
                }
                if found.is_none() {
                    found = parse(&payload);
                }
                continue;
            }
        }
        out.push(line);
    }

    (out.join("\n").trim_end().to_string(), found)
}

fn parse(payload: &str) -> Option<Clarify> {
    let value: Value = serde_json::from_str(payload.trim()).ok()?;
    let question = value
        .get("question")
        .or_else(|| value.get("q"))
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if question.is_empty() {
        return None;
    }
    let options = value
        .get("options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(|option| match option {
                    Value::String(text) => Some(text.trim().to_string()),
                    Value::Object(object) => object
                        .get("label")
                        .or_else(|| object.get("text"))
                        .and_then(Value::as_str)
                        .map(|text| text.trim().to_string()),
                    _ => None,
                })
                .filter(|text| !text.is_empty())
                .take(6)
                .collect()
        })
        .unwrap_or_default();
    let multiselect = value
        .get("multiselect")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(Clarify {
        question,
        options,
        multiselect,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_removes_the_question_block() {
        let text = "I need one detail.\n\n```pi-clarify\n{\"question\":\"Which DB?\",\"options\":[\"SQLite\",\"Postgres\"]}\n```\n";
        let (clean, clarify) = extract(text);
        let clarify = clarify.unwrap();
        assert_eq!(clarify.question, "Which DB?");
        assert_eq!(clarify.options, ["SQLite", "Postgres"]);
        assert_eq!(clean, "I need one detail.");
    }

    #[test]
    fn accepts_objects_and_multiselect() {
        let text = "```pi-clarify\n{\"question\":\"Pick\",\"options\":[{\"label\":\"A\"},{\"label\":\"B\"}],\"multiselect\":true}\n```";
        let clarify = extract(text).1.unwrap();
        assert!(clarify.multiselect);
        assert_eq!(clarify.options, ["A", "B"]);
    }

    #[test]
    fn leaves_other_fences_alone() {
        let text = "```json\n{\"a\":1}\n```";
        let (clean, clarify) = extract(text);
        assert!(clarify.is_none());
        assert!(clean.contains("\"a\":1"));
    }
}
