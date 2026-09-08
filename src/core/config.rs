use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CustomModelSettings {
    #[serde(default)]
    pub num_ctx: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default)]
    pub min_p: Option<f64>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub num_predict: Option<i32>,
    #[serde(default)]
    pub num_thread: Option<u32>,
    #[serde(default)]
    pub repeat_penalty: Option<f64>,
    #[serde(default)]
    pub repeat_last_n: Option<i32>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    #[serde(default)]
    pub frequency_penalty: Option<f64>,
    #[serde(default)]
    pub num_gpu: Option<i32>,
    #[serde(default)]
    pub use_mlock: Option<bool>,
    #[serde(default)]
    pub main_gpu: Option<u32>,
    #[serde(default)]
    pub keep_alive: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

impl CustomModelSettings {
    pub fn to_options_map(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        if let Some(ctx) = self.num_ctx {
            map.insert("num_ctx".to_string(), serde_json::Value::Number(ctx.into()));
        }
        if let Some(temp) = self.temperature {
            if let Some(n) = serde_json::Number::from_f64(temp) {
                map.insert("temperature".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(tp) = self.top_p {
            if let Some(n) = serde_json::Number::from_f64(tp) {
                map.insert("top_p".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(tk) = self.top_k {
            map.insert("top_k".to_string(), serde_json::Value::Number(tk.into()));
        }
        if let Some(mp) = self.min_p {
            if let Some(n) = serde_json::Number::from_f64(mp) {
                map.insert("min_p".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(s) = self.seed {
            map.insert("seed".to_string(), serde_json::Value::Number(s.into()));
        }
        if let Some(np) = self.num_predict {
            map.insert("num_predict".to_string(), serde_json::Value::Number(np.into()));
        }
        if let Some(nth) = self.num_thread {
            if nth > 0 {
                map.insert("num_thread".to_string(), serde_json::Value::Number(nth.into()));
            }
        }
        if let Some(rp) = self.repeat_penalty {
            if let Some(n) = serde_json::Number::from_f64(rp) {
                map.insert("repeat_penalty".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(rln) = self.repeat_last_n {
            map.insert("repeat_last_n".to_string(), serde_json::Value::Number(rln.into()));
        }
        if let Some(pp) = self.presence_penalty {
            if let Some(n) = serde_json::Number::from_f64(pp) {
                map.insert("presence_penalty".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(fp) = self.frequency_penalty {
            if let Some(n) = serde_json::Number::from_f64(fp) {
                map.insert("frequency_penalty".to_string(), serde_json::Value::Number(n));
            }
        }
        if let Some(gpu) = self.num_gpu {
            if gpu >= 0 {
                map.insert("num_gpu".to_string(), serde_json::Value::Number(gpu.into()));
            }
        }
        if let Some(mlock) = self.use_mlock {
            map.insert("use_mlock".to_string(), serde_json::Value::Bool(mlock));
        }
        if let Some(mgpu) = self.main_gpu {
            map.insert("main_gpu".to_string(), serde_json::Value::Number(mgpu.into()));
        }
        serde_json::Value::Object(map)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DaemonMode {
    #[default]
    SystemdService,
    SupervisedProcess,
    RemoteServer,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ExposureMode {
    #[default]
    LocalHost,
    Lan,
    Exposed,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiProviderType {
    OllamaCloud,
    OllamaSsh,
    HuggingFace,
    OllamaRemote,
    CustomCloud,
}

impl ApiProviderType {
    /// Indicates whether this profile represents an active Ollama daemon identity (SSH key or Ollama Cloud / Remote key)
    pub fn is_ollama_identity(&self) -> bool {
        matches!(
            self,
            Self::OllamaCloud | Self::OllamaSsh | Self::OllamaRemote | Self::CustomCloud
        )
    }

    pub fn label(&self) -> String {
        match self {
            Self::OllamaCloud => crate::t!("settings.provider_ollama_cloud"),
            Self::OllamaSsh => crate::t!("settings.provider_ollama_ssh"),
            Self::HuggingFace => crate::t!("settings.provider_hugging_face"),
            Self::OllamaRemote => crate::t!("settings.provider_ollama_remote"),
            Self::CustomCloud => crate::t!("settings.provider_custom_cloud"),
        }
    }

    pub fn icon_name(&self) -> &str {
        match self {
            Self::OllamaCloud => "weather-overcast-symbolic",
            Self::OllamaSsh => "dialog-password-symbolic",
            Self::HuggingFace => "network-workgroup-symbolic",
            Self::OllamaRemote => "network-server-symbolic",
            Self::CustomCloud => "network-wireless-symbolic",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProfile {
    pub id: String,
    pub name: String,
    pub provider_type: ApiProviderType,
    pub token: String,
    pub endpoint: Option<String>,
    pub is_valid: Option<bool>,
    pub last_checked: Option<String>,
    #[serde(default)]
    pub token_in_keyring: bool,
}

impl ApiProfile {
    pub fn new(name: String, provider_type: ApiProviderType, token: String, endpoint: Option<String>) -> Self {
        let id = format!("{}-{}", name.to_lowercase().replace(' ', "-"), chrono::Utc::now().timestamp());
        Self {
            id,
            name,
            provider_type,
            token,
            endpoint,
            is_valid: None,
            last_checked: None,
            token_in_keyring: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub daemon_mode: DaemonMode,
    #[serde(default)]
    pub exposure_mode: ExposureMode,
    #[serde(default)]
    pub models_directory: Option<String>,
    #[serde(default = "default_host")]
    pub ollama_host: String,
    #[serde(default)]
    pub ollama_origins: Option<String>,
    #[serde(default)]
    pub num_parallel: Option<u32>,
    #[serde(default)]
    pub flash_attention: bool,
    #[serde(default)]
    pub keep_alive: Option<String>,
    #[serde(default)]
    pub active_profile_id: Option<String>,
    #[serde(default)]
    pub profiles: Vec<ApiProfile>,
    #[serde(default)]
    pub model_settings: HashMap<String, CustomModelSettings>,
    #[serde(default)]
    pub model_original_modelfiles: HashMap<String, String>,
    #[serde(default)]
    pub model_original_templates: HashMap<String, String>,
}

fn default_language() -> String {
    "auto".to_string()
}

fn default_host() -> String {
    if let Ok(env_host) = std::env::var("OLLAMA_HOST") {
        let trimmed = env_host.trim();
        if !trimmed.is_empty() {
            let clean = trimmed
                .trim_start_matches("http://")
                .trim_start_matches("https://");
            return clean.to_string();
        }
    }
    "127.0.0.1:11434".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            daemon_mode: DaemonMode::SystemdService,
            exposure_mode: ExposureMode::LocalHost,
            models_directory: None,
            ollama_host: default_host(),
            ollama_origins: None,
            num_parallel: None,
            flash_attention: false,
            keep_alive: None,
            active_profile_id: None,
            profiles: Vec::new(),
            model_settings: HashMap::new(),
            model_original_modelfiles: HashMap::new(),
            model_original_templates: HashMap::new(),
        }
    }
}

impl AppConfig {
    /// Returns all normalized key variants for a model name, used for HashMap lookups.
    /// Handles Ollama registry prefixes (`registry.ollama.ai/library/`) and the `:latest` tag.
    fn normalize_model_keys(model: &str) -> Vec<String> {
        let mut keys = vec![model.to_string()];
        let clean = model
            .trim_start_matches("registry.ollama.ai/")
            .trim_start_matches("library/");
        if clean != model {
            keys.push(clean.to_string());
        }
        let base = clean.trim_end_matches(":latest");
        if base != clean {
            keys.push(base.to_string());
        }
        let with_latest = format!("{}:latest", base);
        if !keys.contains(&with_latest) {
            keys.push(with_latest);
        }
        keys
    }

    pub fn get_model_settings(&self, model: &str) -> Option<&CustomModelSettings> {
        Self::normalize_model_keys(model)
            .iter()
            .find_map(|key| self.model_settings.get(key.as_str()))
    }

    pub fn set_model_settings(&mut self, model: String, settings: CustomModelSettings) {
        self.model_settings.insert(model, settings);
    }

    pub fn get_original_modelfile(&self, model: &str) -> Option<&str> {
        Self::normalize_model_keys(model)
            .iter()
            .find_map(|key| self.model_original_modelfiles.get(key.as_str()))
            .map(|s| s.as_str())
    }

    pub fn set_original_modelfile(&mut self, model: String, modelfile: String) {
        self.model_original_modelfiles.insert(model, modelfile);
    }

    pub fn get_original_template(&self, model: &str) -> Option<&str> {
        Self::normalize_model_keys(model)
            .iter()
            .find_map(|key| self.model_original_templates.get(key.as_str()))
            .map(|s| s.as_str())
    }

    pub fn set_original_template(&mut self, model: String, template: String) {
        self.model_original_templates.insert(model, template);
    }

    pub fn remove_model_settings(&mut self, model: &str) {
        for key in Self::normalize_model_keys(model) {
            self.model_settings.remove(&key);
        }
    }

    pub fn config_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push("neuradex");
        path.push("config.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut cfg = if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                serde_json::from_str::<AppConfig>(&content).unwrap_or_default()
            } else {
                Self::default()
            }
        } else {
            Self::default()
        };

        if let Ok(env_host) = std::env::var("OLLAMA_HOST") {
            let trimmed = env_host.trim();
            if !trimmed.is_empty() {
                let clean = trimmed
                    .trim_start_matches("http://")
                    .trim_start_matches("https://");
                cfg.ollama_host = clean.to_string();
            }
        }

        // Ensure active_profile_id only points to an Ollama identity profile (never HF or other vault keys)
        if let Some(ref act_id) = cfg.active_profile_id {
            let is_valid_ollama = cfg
                .profiles
                .iter()
                .any(|p| &p.id == act_id && p.provider_type.is_ollama_identity());
            if !is_valid_ollama {
                cfg.active_profile_id = cfg
                    .profiles
                    .iter()
                    .find(|p| p.provider_type.is_ollama_identity())
                    .map(|p| p.id.clone());
            }
        }

        crate::core::i18n::set_language(&cfg.language);
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        // Sanitize profiles stored in native keyring so secrets are never written to disk in JSON
        let mut sanitized = self.clone();
        for profile in &mut sanitized.profiles {
            if profile.token_in_keyring {
                profile.token.clear();
            }
        }

        let json = serde_json::to_string_pretty(&sanitized)
            .map_err(|e| format!("JSON serialization error: {}", e))?;

        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, json).map_err(|e| format!("Error writing to {:?}: {}", tmp_path, e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("Error atomically renaming {:?} to {:?}: {}", tmp_path, path, e))?;

        // Restrict config file to owner-only (rw-------) since it contains API tokens in plaintext
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }


    pub fn active_profile(&self) -> Option<&ApiProfile> {
        if let Some(ref id) = self.active_profile_id {
            self.profiles
                .iter()
                .find(|p| &p.id == id && p.provider_type.is_ollama_identity())
        } else {
            self.profiles
                .iter()
                .find(|p| p.provider_type.is_ollama_identity())
        }
    }

    pub fn set_active_profile(&mut self, id: Option<String>) {
        if let Some(ref target_id) = id {
            if let Some(p) = self.profiles.iter().find(|p| &p.id == target_id) {
                if p.provider_type.is_ollama_identity() {
                    self.active_profile_id = id;
                    return;
                }
            }
            return;
        }
        self.active_profile_id = None;
    }

    pub fn add_or_update_profile(&mut self, profile: ApiProfile) {
        if let Some(pos) = self.profiles.iter().position(|p| p.id == profile.id) {
            self.profiles[pos] = profile;
        } else {
            if profile.provider_type.is_ollama_identity() && self.active_profile_id.is_none() {
                self.active_profile_id = Some(profile.id.clone());
            }
            self.profiles.push(profile);
        }
    }

    pub fn delete_profile(&mut self, id: &str) {
        self.profiles.retain(|p| p.id != id);
        if self.active_profile_id.as_deref() == Some(id) {
            self.active_profile_id = self
                .profiles
                .iter()
                .find(|p| p.provider_type.is_ollama_identity())
                .map(|p| p.id.clone());
        }
    }
}
