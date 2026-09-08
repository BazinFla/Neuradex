use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelDetails {
    pub parent_model: Option<String>,
    pub format: Option<String>,
    pub family: Option<String>,
    pub families: Option<Vec<String>>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTag {
    pub name: String,
    pub model: Option<String>,
    #[serde(default)]
    pub remote_model: Option<String>,
    #[serde(default)]
    pub remote_host: Option<String>,
    pub modified_at: Option<String>,
    pub size: u64,
    pub digest: Option<String>,
    pub details: Option<ModelDetails>,
}

impl ModelTag {
    pub fn is_cloud(&self) -> bool {
        self.remote_model.is_some()
            || self.remote_host.is_some()
            || self.name.contains("cloud")
            || self.name.ends_with(":cloud")
            || (self.size < 4096 && self.details.as_ref().map(|d| d.format.as_deref() != Some("gguf")).unwrap_or(false))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTagsResponse {
    #[serde(default)]
    pub models: Vec<ModelTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPs {
    pub name: String,
    pub model: Option<String>,
    pub size: u64,
    #[serde(default)]
    pub size_vram: u64,
    pub digest: Option<String>,
    pub details: Option<ModelDetails>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPsResponse {
    #[serde(default)]
    pub models: Vec<ModelPs>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub name: String,
    #[serde(default = "default_true")]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insecure: Option<bool>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PullProgress {
    #[serde(default)]
    pub status: String,
    pub digest: Option<String>,
    pub total: Option<u64>,
    pub completed: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

impl PullProgress {
    pub fn fraction(&self) -> Option<f64> {
        if let (Some(completed), Some(total)) = (self.completed, self.total) {
            if total > 0 {
                return Some((completed as f64 / total as f64).clamp(0.0, 1.0));
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRequest {
    #[serde(alias = "name")]
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modelfile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateProgress {
    #[serde(default)]
    pub status: String,
    pub digest: Option<String>,
    pub total: Option<u64>,
    pub completed: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelShowResponse {
    pub license: Option<String>,
    pub modelfile: Option<String>,
    pub parameters: Option<String>,
    pub template: Option<String>,
    pub system: Option<String>,
    pub details: Option<ModelDetails>,
    pub model_info: Option<std::collections::HashMap<String, serde_json::Value>>,
    pub modified_at: Option<String>,
}

impl ModelShowResponse {
    /// Extracts the native maximum context length of the model from model_info (*.context_length) or parameters (num_ctx)
    pub fn extract_context_length(&self) -> Option<u32> {
        let arch = self.extract_architecture();
        if let Some(ref info) = self.model_info {
            // 1. Exact key for main architecture (e.g. gemma4.context_length, llama.context_length)
            if let Some(ref a) = arch {
                let exact_key = format!("{}.context_length", a);
                if let Some(v) = info.get(&exact_key).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
            }

            // 2. Key without sub-encoder (exclude vision/audio)
            for (k, v) in info {
                if !k.contains(".vision.") && !k.contains(".audio.") && !k.contains(".image.")
                    && (k.ends_with(".context_length") || k == "context_length") {
                        if let Some(n) = v.as_u64() {
                            return Some(n as u32);
                        }
                    }
            }
        }

        if let Some(ref params) = self.parameters {
            for line in params.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0] == "num_ctx" {
                    if let Ok(n) = parts[1].parse::<u32>() {
                        return Some(n);
                    }
                }
            }
        }
        None
    }

    /// Extracts the total number of GPU layers of the main model (block_count / layer_count)
    pub fn extract_layer_count(&self) -> Option<u32> {
        let arch = self.extract_architecture();
        if let Some(ref info) = self.model_info {
            // 1. Exact key for main architecture (e.g. gemma4.block_count, llama.block_count)
            if let Some(ref a) = arch {
                let exact_key = format!("{}.block_count", a);
                if let Some(v) = info.get(&exact_key).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
                let exact_key_layers = format!("{}.layer_count", a);
                if let Some(v) = info.get(&exact_key_layers).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
            }

            // 2. Main key ignoring multimodal sub-towers (vision, audio, image, clip)
            for (k, v) in info {
                if !k.contains(".vision.") && !k.contains(".audio.") && !k.contains(".image.") && !k.contains(".clip.")
                    && (k.ends_with(".block_count") || k.ends_with(".layer_count") || k == "block_count" || k == "layer_count") {
                        if let Some(n) = v.as_u64() {
                            return Some(n as u32);
                        }
                    }
            }
        }
        None
    }

    /// Extracts the number of KV attention heads (head_count_kv)
    pub fn extract_kv_heads(&self) -> Option<u32> {
        let arch = self.extract_architecture();
        if let Some(ref info) = self.model_info {
            if let Some(ref a) = arch {
                let exact_key = format!("{}.attention.head_count_kv", a);
                if let Some(v) = info.get(&exact_key).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
            }

            for (k, v) in info {
                if !k.contains(".vision.") && !k.contains(".audio.")
                    && (k.ends_with(".head_count_kv") || k.ends_with(".attention.head_count_kv") || k == "head_count_kv") {
                        if let Some(n) = v.as_u64() {
                            return Some(n as u32);
                        }
                    }
            }
        }
        None
    }

    /// Extracts the total number of attention heads (head_count)
    pub fn extract_head_count(&self) -> Option<u32> {
        let arch = self.extract_architecture();
        if let Some(ref info) = self.model_info {
            if let Some(ref a) = arch {
                let exact_key = format!("{}.attention.head_count", a);
                if let Some(v) = info.get(&exact_key).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
            }

            for (k, v) in info {
                if !k.contains(".vision.") && !k.contains(".audio.")
                    && (k.ends_with(".head_count") || k.ends_with(".attention.head_count") || k == "head_count") {
                        if let Some(n) = v.as_u64() {
                            return Some(n as u32);
                        }
                    }
            }
        }
        None
    }

    /// Extracts the embedding dimension (embedding_length)
    pub fn extract_embedding_length(&self) -> Option<u32> {
        let arch = self.extract_architecture();
        if let Some(ref info) = self.model_info {
            if let Some(ref a) = arch {
                let exact_key = format!("{}.embedding_length", a);
                if let Some(v) = info.get(&exact_key).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
            }

            for (k, v) in info {
                if !k.contains(".vision.") && !k.contains(".audio.")
                    && (k.ends_with(".embedding_length") || k == "embedding_length") {
                        if let Some(n) = v.as_u64() {
                            return Some(n as u32);
                        }
                    }
            }
        }
        None
    }

    /// Extracts the model architecture (e.g. "gemma", "gemma2", "gemma4", "llama", "qwen2", "mistral")
    pub fn extract_architecture(&self) -> Option<String> {
        if let Some(ref info) = self.model_info {
            for (k, v) in info {
                if k == "general.architecture" || k.ends_with(".architecture") {
                    if let Some(s) = v.as_str() {
                        return Some(s.to_string());
                    }
                }
            }
        }
        None
    }

    /// Extracts the dimension per attention head (key_length / head_dim)
    pub fn extract_head_dim(&self) -> Option<u32> {
        let arch = self.extract_architecture();
        if let Some(ref info) = self.model_info {
            if let Some(ref a) = arch {
                let exact_key = format!("{}.attention.key_length", a);
                if let Some(v) = info.get(&exact_key).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
                let exact_head_dim = format!("{}.attention.head_dim", a);
                if let Some(v) = info.get(&exact_head_dim).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
            }

            for (k, v) in info {
                if !k.contains(".vision.") && !k.contains(".audio.")
                    && (k.ends_with(".attention.key_length")
                        || k.ends_with(".key_length")
                        || k.ends_with(".attention.head_dim")
                        || k.ends_with(".head_dim")
                        || k.ends_with(".attention.value_length"))
                    {
                        if let Some(n) = v.as_u64() {
                            return Some(n as u32);
                        }
                    }
            }
        }

        // Specific case for Gemma / Gemma 2 / Gemma 3 architectures
        if let Some(arch_name) = self.extract_architecture() {
            let arch_low = arch_name.to_lowercase();
            if arch_low.contains("gemma") {
                return Some(256);
            }
        }

        // Fallback: embedding_length / head_count
        if let (Some(embd), Some(heads)) = (self.extract_embedding_length(), self.extract_head_count()) {
            return embd.checked_div(heads);
        }

        None
    }

    /// Extracts the sliding window attention size (sliding_window) if applicable
    pub fn extract_sliding_window(&self) -> Option<u32> {
        let arch = self.extract_architecture();
        if let Some(ref info) = self.model_info {
            if let Some(ref a) = arch {
                let exact_key = format!("{}.attention.sliding_window", a);
                if let Some(v) = info.get(&exact_key).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
                let exact_key_len = format!("{}.attention.sliding_window_length", a);
                if let Some(v) = info.get(&exact_key_len).and_then(|v| v.as_u64()) {
                    return Some(v as u32);
                }
            }

            for (k, v) in info {
                if !k.contains(".vision.") && !k.contains(".audio.")
                    && (k.ends_with(".attention.sliding_window_length")
                        || k.ends_with(".sliding_window_length")
                        || k.ends_with(".attention.sliding_window")
                        || k.ends_with(".sliding_window"))
                    {
                        if let Some(n) = v.as_u64() {
                            return Some(n as u32);
                        }
                    }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            images: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            images: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
            images: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default = "default_true")]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatStreamChunk {
    pub model: Option<String>,
    pub created_at: Option<String>,
    pub message: Option<ChatMessage>,
    #[serde(default)]
    pub done: bool,
    pub total_duration: Option<u64>,
    pub load_duration: Option<u64>,
    pub prompt_eval_count: Option<u64>,
    pub prompt_eval_duration: Option<u64>,
    pub eval_count: Option<u64>,
    pub eval_duration: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ChatMetrics {
    pub total_duration_ms: f64,
    pub ttft_ms: f64,
    pub eval_count: u64,
    pub prompt_eval_count: u64,
    pub tokens_per_sec: f64,
    pub vram_bytes: u64,
}

/// Utilities for normalization, sanitization, and comparison of Ollama model tags
pub struct ModelNameUtils;

impl ModelNameUtils {
    /// Strips registry prefixes (`registry.ollama.ai/`, `library/`) and converts to lowercase
    pub fn clean_tag(tag: &str) -> String {
        let t = tag.trim().to_lowercase();
        t.trim_start_matches("registry.ollama.ai/")
            .trim_start_matches("library/")
            .to_string()
    }

    /// Strips prefixes and removes the default `:latest` suffix
    pub fn base_tag(tag: &str) -> String {
        let cleaned = Self::clean_tag(tag);
        cleaned.trim_end_matches(":latest").to_string()
    }

    /// Checks if a given tag belongs to a collection or set of installed tags
    pub fn is_tag_installed<I, S>(installed_tags: I, target_tag: &str) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let target_clean = Self::clean_tag(target_tag);
        let target_base = Self::base_tag(target_tag);
        let is_target_cloud = target_tag.ends_with(":cloud") || target_tag.ends_with("-cloud");

        for item in installed_tags {
            let item_str = item.as_ref();
            let inst_clean = Self::clean_tag(item_str);
            let inst_base = Self::base_tag(item_str);
            let is_inst_cloud = item_str.ends_with(":cloud") || item_str.ends_with("-cloud");

            // A Cloud tag must never be confused with a physical local variant
            if is_target_cloud != is_inst_cloud {
                continue;
            }

            if inst_clean == target_clean
                || inst_base == target_base
                || (is_target_cloud && (inst_clean == target_clean || inst_clean.starts_with(&target_base)))
            {
                return true;
            }
        }
        false
    }

    /// Detects if a model is currently preloaded in VRAM / RAM
    /// and returns `(is_loaded, vram_used_in_bytes)`
    pub fn check_model_running(model_name: &str, model_size: u64, running: &[ModelPs]) -> (bool, u64) {
        let tag_name = model_name.to_lowercase();
        let tag_clean = Self::clean_tag(model_name);
        let tag_base = Self::base_tag(model_name);

        for r in running {
            let r_name = r.name.to_lowercase();
            let r_clean = Self::clean_tag(&r.name);
            let r_base = Self::base_tag(&r.name);

            let r_model = r.model.as_deref().map(|m| m.to_lowercase()).unwrap_or_default();
            let r_model_clean = Self::clean_tag(&r_model);
            let r_model_base = Self::base_tag(&r_model);

            let matches = tag_name == r_name
                || tag_clean == r_clean
                || tag_base == r_base
                || (!r_model.is_empty() && (tag_name == r_model || tag_clean == r_model_clean || tag_base == r_model_base))
                || (!tag_base.is_empty() && (r_clean.starts_with(&tag_base) || tag_clean.starts_with(&r_base)))
                || (!r_model_base.is_empty() && (r_model_clean.starts_with(&tag_base) || tag_clean.starts_with(&r_model_base)));

            if matches {
                let vram = if r.size_vram > 0 {
                    r.size_vram
                } else if r.size > 0 {
                    r.size
                } else {
                    model_size
                };
                return (true, vram);
            }
        }
        (false, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_name_utils() {
        assert_eq!(ModelNameUtils::clean_tag("registry.ollama.ai/library/llama3.2:latest"), "llama3.2:latest");
        assert_eq!(ModelNameUtils::base_tag("registry.ollama.ai/library/llama3.2:latest"), "llama3.2");
        assert_eq!(ModelNameUtils::base_tag("llama3.2"), "llama3.2");

        let installed = vec![
            "llama3.2:latest".to_string(),
            "qwen2.5-coder:7b".to_string(),
            "gemma4:cloud".to_string(),
        ];
        assert!(ModelNameUtils::is_tag_installed(&installed, "llama3.2"));
        assert!(ModelNameUtils::is_tag_installed(&installed, "llama3.2:latest"));
        assert!(ModelNameUtils::is_tag_installed(&installed, "registry.ollama.ai/library/llama3.2:latest"));
        assert!(ModelNameUtils::is_tag_installed(&installed, "qwen2.5-coder:7b"));
        assert!(!ModelNameUtils::is_tag_installed(&installed, "mistral:latest"));

        // Strict separation between Cloud and Local
        assert!(ModelNameUtils::is_tag_installed(&installed, "gemma4:cloud"));
        assert!(!ModelNameUtils::is_tag_installed(&installed, "gemma4:31b"));
        assert!(!ModelNameUtils::is_tag_installed(&installed, "gemma4:latest"));
        assert!(!ModelNameUtils::is_tag_installed(&installed, "gemma4"));

        let running = vec![
            ModelPs {
                name: "registry.ollama.ai/library/llama3.2:latest".to_string(),
                model: Some("llama3.2:latest".to_string()),
                size: 2_000_000_000,
                size_vram: 2_000_000_000,
                digest: None,
                details: None,
                expires_at: None,
            }
        ];
        let (is_run, vram) = ModelNameUtils::check_model_running("llama3.2", 2_000_000_000, &running);
        assert!(is_run);
        assert_eq!(vram, 2_000_000_000);
    }
}

