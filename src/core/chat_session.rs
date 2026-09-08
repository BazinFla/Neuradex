use crate::api::types::ChatMessage;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Saved conversation structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub num_ctx: Option<u32>,
    pub messages: Vec<ChatMessage>,
    pub updated_at: u64,
}

impl ChatSession {
    /// Returns the localized default title for a new conversation
    pub fn default_title() -> String {
        crate::t!("chat.default_title")
    }

    /// Checks whether a given title is considered a generic/default conversation title.
    /// Dynamically checks all registered languages so adding a new locale requires no code change.
    pub fn is_default_title(title: &str) -> bool {
        let t = title.trim();
        if t.is_empty() {
            return true;
        }

        // Common legacy / generic default titles
        const COMMON_DEFAULTS: &[&str] = &["new chat", "nouvelle conversation"];
        if COMMON_DEFAULTS.iter().any(|d| t.eq_ignore_ascii_case(d)) {
            return true;
        }

        // Collect all translations of default title keys across every registered language
        let default_titles: Vec<String> = ["chat.default_title", "chat.sidebar.new_chat"]
            .iter()
            .flat_map(|key| crate::core::i18n::t_all_langs(key))
            .collect();

        default_titles.iter().any(|dt| t.eq_ignore_ascii_case(dt))
    }

    /// Automatically generates a concise, readable conversation title from the first user message.
    /// Extracts 5-6 words (or ~36 characters), stripping markdown and dangling prepositions.
    pub fn generate_title(first_message: &str) -> String {
        let raw = first_message.trim();
        if raw.is_empty() {
            return Self::default_title();
        }

        // 1. Clean markdown headers, quotes, bullets, and code fences
        let mut clean_lines = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            // Skip code fences like ```rust or ```
            if trimmed.starts_with("```") {
                continue;
            }
            // Strip markdown headers (#, ##), quotes (>), and bullet points (-, *, +, 1.)
            let stripped = trimmed
                .trim_start_matches('#')
                .trim_start_matches('>')
                .trim_start_matches(|c: char| c == '-' || c == '*' || c == '+' || c.is_ascii_digit() || c == '.')
                .trim();
            if !stripped.is_empty() {
                clean_lines.push(stripped);
            }
        }

        // If all lines were code fences or empty, fallback to raw lines
        let text = if clean_lines.is_empty() {
            raw.lines()
                .map(|l| l.trim().trim_matches('`').trim())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            clean_lines.join(" ")
        };

        // Remove markdown formatting characters like backticks, asterisks, tildes
        let text = text.replace(['`', '*', '_', '~'], "");

        // Split into words
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return Self::default_title();
        }

        // Take up to 6 words, but keep length <= 36 characters if possible
        let mut selected = Vec::new();
        let mut current_chars = 0;

        for (i, &word) in words.iter().enumerate() {
            if i >= 6 {
                break;
            }
            let word_char_count = word.chars().count();
            let new_total = if selected.is_empty() {
                word_char_count
            } else {
                current_chars + 1 + word_char_count
            };

            // Allow at least 2 words if possible, but stop if exceeding 36 characters
            if new_total > 36 && selected.len() >= 2 {
                break;
            }

            selected.push(word);
            current_chars = new_total;
        }

        // If we truncated the sentence, drop dangling trailing prepositions / conjunctions / articles
        const TRAILING_STOP_WORDS: &[&str] = &[
            "a", "an", "the", "in", "on", "at", "to", "for", "of", "with", "and", "or", "by", "from", "about",
            "le", "la", "les", "un", "une", "des", "de", "du", "en", "à", "au", "aux", "pour", "par", "avec", "et", "ou", "sur", "dans",
        ];

        if selected.len() < words.len() {
            while selected.len() > 2 {
                let last = selected.last().unwrap().to_lowercase();
                let last_trimmed = last.trim_matches(|c: char| !c.is_alphabetic());
                if TRAILING_STOP_WORDS.contains(&last_trimmed) {
                    selected.pop();
                } else {
                    break;
                }
            }
        }

        let mut title = selected.join(" ");

        // Trim trailing punctuation (commas, colons, semicolons, dashes, periods) while preserving ? or !
        title = title
            .trim_end_matches([',', ';', ':', '-', '.', '…', '—'])
            .trim()
            .to_string();

        // Capitalize the first character if it's lowercase
        if let Some(first_char) = title.chars().next() {
            if first_char.is_lowercase() {
                let mut chars = title.chars();
                let upper = chars.next().unwrap().to_uppercase().to_string();
                title = format!("{}{}", upper, chars.as_str());
            }
        }

        // Cap maximum length to 40 characters with ellipsis if needed
        if title.chars().count() > 40 {
            let mut truncated: String = title.chars().take(37).collect();
            truncated = truncated.trim_end().to_string();
            title = format!("{}...", truncated);
        }

        if title.is_empty() {
            Self::default_title()
        } else {
            title
        }
    }

    pub fn new_empty(model: Option<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let id = format!("chat_{}", now);
        Self {
            id,
            title: Self::default_title(),
            model,
            system_prompt: None,
            temperature: Some(0.70),
            num_ctx: None,
            messages: Vec::new(),
            updated_at: (now / 1000) as u64,
        }
    }

    pub fn storage_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push("neuradex");
        let _ = fs::create_dir_all(&path);
        path.push("chat_sessions.json");
        path
    }

    pub fn load_all() -> Vec<ChatSession> {
        let path = Self::storage_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut sessions) = serde_json::from_str::<Vec<ChatSession>>(&content) {
                    if !sessions.is_empty() {
                        sessions.sort_by_key(|a| std::cmp::Reverse(a.updated_at));
                        // Auto-upgrade legacy sessions that have a default title but already have user messages
                        let mut updated = false;
                        for s in &mut sessions {
                            if Self::is_default_title(&s.title) {
                                if let Some(first_user_msg) = s.messages.iter().find(|m| m.role == "user") {
                                    s.title = Self::generate_title(&first_user_msg.content);
                                    updated = true;
                                }
                            }
                        }
                        if updated {
                            Self::save_all(&sessions);
                        }
                        return sessions;
                    }
                }
            }
        }
        vec![Self::new_empty(None)]
    }

    pub fn save_all(sessions: &[ChatSession]) {
        let path = Self::storage_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(sessions) {
            let tmp_path = path.with_extension("tmp");
            if fs::write(&tmp_path, json).is_ok() {
                let _ = fs::rename(&tmp_path, &path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_default_title() {
        assert!(ChatSession::is_default_title("Nouvelle discussion"));
        assert!(ChatSession::is_default_title("nouvelle discussion"));
        assert!(ChatSession::is_default_title("Nouvelle Discussion"));
        assert!(ChatSession::is_default_title("New conversation"));
        assert!(ChatSession::is_default_title("new conversation"));
        assert!(ChatSession::is_default_title("New chat"));
        assert!(ChatSession::is_default_title(""));
        assert!(!ChatSession::is_default_title("Explain Rust lifetimes"));
    }

    #[test]
    fn test_generate_title_basic() {
        assert_eq!(
            ChatSession::generate_title("Explain how LLM KV-cache works in simple terms."),
            "Explain how LLM KV-cache works"
        );
        assert_eq!(
            ChatSession::generate_title("How to optimize VRAM usage for local inference?"),
            "How to optimize VRAM usage"
        );
        assert_eq!(
            ChatSession::generate_title("hello world"),
            "Hello world"
        );
    }

    #[test]
    fn test_generate_title_markdown_and_code() {
        assert_eq!(
            ChatSession::generate_title("# Rust Async\n\nHow does tokio work?"),
            "Rust Async How does tokio work?"
        );
        assert_eq!(
            ChatSession::generate_title("```rust\nfn main() {}\n```"),
            "Fn main() {}"
        );
        assert_eq!(
            ChatSession::generate_title("   **what** is `memory mapping`?   "),
            "What is memory mapping?"
        );
    }

    #[test]
    fn test_generate_title_empty_or_whitespace() {
        assert!(!ChatSession::generate_title("   ").is_empty());
    }
}
