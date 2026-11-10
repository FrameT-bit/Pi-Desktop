
use std::path::PathBuf;

pub const MAX_ENTRIES: usize = 200;
pub const MAX_CHARS: usize = 8_000;

#[derive(Default)]
pub struct PromptHistory {
    entries: Vec<String>,
    index: Option<usize>,
    draft: String,
}

impl PromptHistory {
    pub fn new(entries: Vec<String>) -> Self {
        let mut entries: Vec<String> = entries
            .into_iter()
            .filter(|entry| !entry.trim().is_empty())
            .collect();
        if entries.len() > MAX_ENTRIES {
            entries.drain(..entries.len() - MAX_ENTRIES);
        }
        Self {
            entries,
            index: None,
            draft: String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn navigating(&self) -> bool {
        self.index.is_some()
    }

    pub fn push(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let text: String = text.chars().take(MAX_CHARS).collect();
        if self.entries.last() == Some(&text) {
            self.reset();
            return;
        }
        self.entries.push(text);
        if self.entries.len() > MAX_ENTRIES {
            let excess = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..excess);
        }
        self.reset();
    }

    pub fn previous(&mut self, current: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let next = match self.index {
            None => {
                self.draft = current.to_string();
                self.entries.len() - 1
            }
            Some(0) => 0,
            Some(index) => index - 1,
        };
        self.index = Some(next);
        Some(self.entries[next].clone())
    }

    pub fn next(&mut self) -> Option<String> {
        let index = self.index?;
        if index + 1 < self.entries.len() {
            self.index = Some(index + 1);
            return Some(self.entries[index + 1].clone());
        }
        self.index = None;
        Some(std::mem::take(&mut self.draft))
    }

    pub fn reset(&mut self) {
        self.index = None;
        self.draft.clear();
    }

    pub fn path() -> Option<PathBuf> {
        crate::config::config_path().and_then(|path| path.parent().map(|dir| dir.join("composer-history.json")))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::new(Vec::new());
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::new(Vec::new());
        };
        let entries = serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default();
        Self::new(entries)
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        let Ok(raw) = serde_json::to_string_pretty(&self.entries) else {
            return;
        };
        let _ = crate::config::write_atomic(&path, &raw);
    }
}


