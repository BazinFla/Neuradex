use serde_json::Value;
use std::collections::HashMap;
use std::sync::RwLock;

/// Definition of a supported language in NeuraDex
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageDef {
    pub code: &'static str,
    pub name: &'static str,
    pub flag: &'static str,
    pub json: &'static str,
}

/// Central registry of available languages.
/// To add a new language, simply add a new LanguageDef entry here!
pub const AVAILABLE_LANGUAGES: &[LanguageDef] = &[
    LanguageDef {
        code: "en",
        name: "English",
        flag: "🇬🇧",
        json: include_str!("../../locales/en.json"),
    },
    LanguageDef {
        code: "fr",
        name: "Français",
        flag: "🇫🇷",
        json: include_str!("../../locales/fr.json"),
    },
];

pub fn available_languages() -> &'static [LanguageDef] {
    AVAILABLE_LANGUAGES
}

pub fn get_language_def(code: &str) -> Option<&'static LanguageDef> {
    AVAILABLE_LANGUAGES.iter().find(|l| l.code == code)
}

pub fn get_language_name(code: &str) -> &'static str {
    get_language_def(code).map(|l| l.name).unwrap_or("English")
}

pub fn get_language_flag(code: &str) -> &'static str {
    get_language_def(code).map(|l| l.flag).unwrap_or("🌐")
}

struct I18nEngine {
    languages: HashMap<String, HashMap<String, String>>,
    active_lang: String,
    detected_system_lang: String,
}

impl I18nEngine {
    fn new() -> Self {
        let mut languages = HashMap::new();

        for def in AVAILABLE_LANGUAGES {
            if let Ok(val) = serde_json::from_str::<Value>(def.json) {
                let mut map = HashMap::new();
                flatten_json(&val, String::new(), &mut map);
                languages.insert(def.code.to_string(), map);
            }
        }

        let detected = detect_system_language();

        Self {
            languages,
            active_lang: "auto".to_string(),
            detected_system_lang: detected.to_string(),
        }
    }

    fn resolve_lang(&self) -> &str {
        if self.active_lang == "auto" {
            &self.detected_system_lang
        } else {
            &self.active_lang
        }
    }

    fn get_raw(&self, key: &str) -> Option<&str> {
        let target_lang = self.resolve_lang();

        // 1. Try target language
        if let Some(map) = self.languages.get(target_lang) {
            if let Some(val) = map.get(key) {
                return Some(val.as_str());
            }
        }

        // 2. Fallback to English
        if target_lang != "en" {
            if let Some(map) = self.languages.get("en") {
                if let Some(val) = map.get(key) {
                    return Some(val.as_str());
                }
            }
        }

        None
    }

    fn translate(&self, key: &str) -> String {
        self.get_raw(key).unwrap_or(key).to_string()
    }

    fn translate_with_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.translate(key);
        for &(param, val) in args {
            let target = format!("{{{}}}", param);
            text = text.replace(&target, val);
        }
        text
    }

    /// Returns the translation of a key in every registered language (for language-agnostic comparisons)
    fn all_translations(&self, key: &str) -> Vec<String> {
        let mut results = Vec::new();
        for map in self.languages.values() {
            if let Some(val) = map.get(key) {
                if !results.contains(val) {
                    results.push(val.clone());
                }
            }
        }
        results
    }
}

fn flatten_json(val: &Value, prefix: String, out: &mut HashMap<String, String>) {
    match val {
        Value::Object(map) => {
            for (k, v) in map {
                let new_prefix = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json(v, new_prefix, out);
            }
        }
        Value::String(s) => {
            out.insert(prefix, s.clone());
        }
        Value::Array(arr) => {
            for (idx, item) in arr.iter().enumerate() {
                let new_prefix = format!("{}.{}", prefix, idx);
                flatten_json(item, new_prefix, out);
            }
        }
        Value::Number(n) => {
            out.insert(prefix, n.to_string());
        }
        Value::Bool(b) => {
            out.insert(prefix, b.to_string());
        }
        Value::Null => {}
    }
}

static ENGINE: std::sync::OnceLock<RwLock<I18nEngine>> = std::sync::OnceLock::new();

fn get_engine() -> &'static RwLock<I18nEngine> {
    ENGINE.get_or_init(|| RwLock::new(I18nEngine::new()))
}

/// Detects system language based on standard Linux environment variables
pub fn detect_system_language() -> &'static str {
    let lang = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_lowercase();

    for def in AVAILABLE_LANGUAGES {
        if lang.starts_with(def.code) {
            return def.code;
        }
    }

    "en"
}

/// Sets the active language preference ("auto" or any registered language code like "en", "fr", etc.)
pub fn set_language(lang: &str) {
    if let Ok(mut engine) = get_engine().write() {
        engine.active_lang = lang.to_string();
    }
}

/// Returns the configured language ("auto", "en", "fr", etc.)
#[allow(dead_code)]
pub fn get_language() -> String {
    get_engine()
        .read()
        .map(|e| e.active_lang.clone())
        .unwrap_or_else(|_| "auto".to_string())
}

/// Returns the effective resolved language ("en", "fr", etc.)
#[allow(dead_code)]
pub fn get_effective_language() -> String {
    get_engine()
        .read()
        .map(|e| e.resolve_lang().to_string())
        .unwrap_or_else(|_| "en".to_string())
}

/// Translates a key into the active language
pub fn t(key: &str) -> String {
    get_engine()
        .read()
        .map(|e| e.translate(key))
        .unwrap_or_else(|_| key.to_string())
}

/// Translates a key with variable substitutions
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    get_engine()
        .read()
        .map(|e| e.translate_with_args(key, args))
        .unwrap_or_else(|_| key.to_string())
}

/// Returns translations of a key in all registered languages (for language-agnostic comparisons)
pub fn t_all_langs(key: &str) -> Vec<String> {
    get_engine()
        .read()
        .map(|e| e.all_translations(key))
        .unwrap_or_default()
}

/// Shorthand macro for translation lookups
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::core::i18n::t($key)
    };
    ($key:expr, $($name:ident = $val:expr),* $(,)?) => {
        $crate::core::i18n::t_args($key, &[
            $((stringify!($name), &$val.to_string())),*
        ])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i18n_translation_and_fallback() {
        for l in AVAILABLE_LANGUAGES {
            set_language(l.code);
            assert_eq!(get_language(), l.code);
            assert_eq!(get_effective_language(), l.code);
            assert_eq!(t("app.title"), "NeuraDex");
            assert_ne!(t!("instances.unload_all"), "instances.unload_all");
        }

        set_language("fr");
        let formatted = t!("instances.installed_count", count = 3);
        assert_eq!(formatted, "3 modèle(s)");

        set_language("en");
        let formatted_en = t!("instances.installed_count", count = 3);
        assert_eq!(formatted_en, "3 model(s)");

        // Ensure critical view keys exist and are translated
        for lang_def in AVAILABLE_LANGUAGES {
            set_language(lang_def.code);
            assert_ne!(t!("chat.default_title"), "chat.default_title");
            assert_ne!(t!("chat.sidebar_title"), "chat.sidebar_title");
            assert_ne!(t!("chat.user_label"), "chat.user_label");
            assert_ne!(t!("chat.input_hint"), "chat.input_hint");
            assert_ne!(t!("chat.cloud_remote_model"), "chat.cloud_remote_model");
            assert_ne!(t!("chat.generating_status"), "chat.generating_status");
            assert_ne!(t!("chat.welcome_title"), "chat.welcome_title");
            assert_ne!(t!("chat.badge_vram"), "chat.badge_vram");
            assert_ne!(t!("model_settings.est_vram"), "model_settings.est_vram");
            assert_ne!(t!("logs.title"), "logs.title");
            assert_ne!(t!("nav.instances"), "nav.instances");
        }
    }
}

