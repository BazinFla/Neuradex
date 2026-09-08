use crate::core::hardware::HardwareSnapshot;
use crate::t;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareFitness {
    /// 🟢 Model fits 100% in GPU VRAM (Maximum speed)
    FullGpu,
    /// 🟡 Model overflows to CPU RAM (Hybrid execution with offloading)
    GpuOffload,
    /// 🔴 Total memory (VRAM + RAM) is insufficient for this model
    Insufficient,
}

impl HardwareFitness {
    pub fn badge_label(&self) -> String {
        match self {
            Self::FullGpu => t!("fitness.full_gpu"),
            Self::GpuOffload => t!("fitness.gpu_offload"),
            Self::Insufficient => t!("fitness.insufficient"),
        }
    }

    pub fn tooltip(&self) -> String {
        match self {
            Self::FullGpu => t!("fitness.tooltip_full_gpu"),
            Self::GpuOffload => t!("fitness.tooltip_gpu_offload"),
            Self::Insufficient => t!("fitness.tooltip_insufficient"),
        }
    }
}

fn default_source() -> String {
    "ollama_library".to_string()
}
fn default_true() -> bool {
    true
}
fn default_namespace() -> String {
    "library".to_string()
}
fn default_icon() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCapabilitiesFlags {
    #[serde(default)]
    pub think: bool,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub audio: bool,
    #[serde(default)]
    pub tools: bool,
    #[serde(default)]
    pub code: bool,
    #[serde(default)]
    pub embedding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubModelVariant {
    pub tag: String,
    #[serde(default)]
    pub name: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub size_formatted: String,
    #[serde(default)]
    pub parameter_size: String,
    #[serde(default)]
    pub quantization: String,
    #[serde(default)]
    pub context_length: u32,
    #[serde(default)]
    pub context_formatted: String,
    #[serde(default)]
    pub input_type: String,
    #[serde(default)]
    pub digest: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub is_cloud: bool,
}

impl HubModelVariant {
    pub fn new(
        tag: impl Into<String>,
        size_bytes: u64,
        parameter_size: impl Into<String>,
        quantization: impl Into<String>,
        context_length: u32,
    ) -> Self {
        Self {
            tag: tag.into(),
            name: String::new(),
            size_bytes,
            size_formatted: String::new(),
            parameter_size: parameter_size.into(),
            quantization: quantization.into(),
            context_length,
            context_formatted: String::new(),
            input_type: "Text".to_string(),
            digest: String::new(),
            updated_at: String::new(),
            is_cloud: false,
        }
    }
}

impl Default for HubModelVariant {
    fn default() -> Self {
        Self::new("", 0, "", "", 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortCriterion {
    Popularity,
    #[default]
    Newest,
    NameAsc,
    NameDesc,
    SizeAsc,
    SizeDesc,
}

impl SortCriterion {
    pub fn all_labels() -> Vec<String> {
        vec![
            t!("hub.sort.popularity"),
            t!("hub.sort.newest"),
            t!("hub.sort.name_asc"),
            t!("hub.sort.name_desc"),
            t!("hub.sort.size_asc"),
            t!("hub.sort.size_desc"),
        ]
    }

    pub fn from_index(idx: u32) -> Self {
        match idx {
            1 => Self::Newest,
            2 => Self::NameAsc,
            3 => Self::NameDesc,
            4 => Self::SizeAsc,
            5 => Self::SizeDesc,
            _ => Self::Popularity,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubModelInfo {
    pub id: String,
    pub name: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_true")]
    pub is_official: bool,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    pub category: String,
    pub description: String,
    pub default_tag: String,
    #[serde(default)]
    pub badges: Vec<String>,
    #[serde(default)]
    pub capabilities: Option<ModelCapabilitiesFlags>,
    pub variants: Vec<HubModelVariant>,
    #[serde(default = "default_icon")]
    pub icon_name: String,
    #[serde(default)]
    pub is_cloud: bool,
    #[serde(default)]
    pub pulls_count: u64,
    #[serde(default)]
    pub tags_count: u32,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub updated_full_date: Option<String>,
    #[serde(default)]
    pub updated_date_key: u64,
    #[serde(default)]
    pub page_url: String,
    #[serde(default)]
    pub tags_page_url: String,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub is_deprecated: bool,
    #[serde(default)]
    pub deprecated_at: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

impl Default for HubModelInfo {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            source: default_source(),
            is_official: default_true(),
            namespace: default_namespace(),
            category: String::new(),
            description: String::new(),
            default_tag: String::new(),
            badges: Vec::new(),
            capabilities: None,
            variants: Vec::new(),
            icon_name: default_icon(),
            is_cloud: false,
            pulls_count: 0,
            tags_count: 0,
            updated_at: None,
            updated_full_date: None,
            updated_date_key: 0,
            page_url: String::new(),
            tags_page_url: String::new(),
            creator: None,
            publisher: None,
            family: None,
            is_deprecated: false,
            deprecated_at: None,
            status: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCatalogResponse {
    pub version: String,
    #[serde(default)]
    pub scraped_at: String,
    #[serde(default)]
    pub total_models: usize,
    pub models: Vec<HubModelInfo>,
}

pub struct HubManager;

impl HubManager {
    /// Sorts a list of models according to the specified criterion
    pub fn sort_models(models: &mut [HubModelInfo], criterion: SortCriterion) {
        match criterion {
            SortCriterion::Popularity => {
                models.sort_by_key(|a| std::cmp::Reverse(a.pulls_count));
            }
            SortCriterion::Newest => {
                models.sort_by_key(|a| std::cmp::Reverse(a.updated_date_key));
            }
            SortCriterion::NameAsc => {
                models.sort_by_key(|a| a.name.to_lowercase());
            }
            SortCriterion::NameDesc => {
                models.sort_by_key(|a| std::cmp::Reverse(a.name.to_lowercase()));
            }
            SortCriterion::SizeAsc => {
                models.sort_by_key(|a| a.variants.first().map(|v| v.size_bytes).unwrap_or(0));
            }
            SortCriterion::SizeDesc => {
                models.sort_by_key(|a| std::cmp::Reverse(a.variants.first().map(|v| v.size_bytes).unwrap_or(0)));
            }
        }
    }

    /// Evaluates hardware fitness for a model size relative to current hardware
    pub fn evaluate_fitness(model_size_bytes: u64, snap: &HardwareSnapshot) -> HardwareFitness {
        // 20% safety margin for KV-Cache and attention buffers
        let needed_vram = (model_size_bytes as f64 * 1.20) as u64;

        let available_gpu_vram = if let Some(ref gpu) = snap.gpu {
            gpu.vram_free
        } else {
            0
        };

        let available_sys_ram = snap.cpu.ram_total.saturating_sub(snap.cpu.ram_used);
        let total_available = available_gpu_vram + available_sys_ram;

        if available_gpu_vram >= needed_vram {
            HardwareFitness::FullGpu
        } else if total_available >= needed_vram {
            HardwareFitness::GpuOffload
        } else {
            HardwareFitness::Insufficient
        }
    }

    /// Determines if a model is a Cloud / API / Remote model
    pub fn is_cloud_model(id: &str, name: &str, _description: &str, category: &str, variants: &[HubModelVariant]) -> bool {
        // 1. Does it have an explicit Cloud variant?
        if variants.iter().any(|v| v.tag.contains("cloud") || v.size_bytes == 0) {
            return true;
        }

        let id_lower = id.to_lowercase();
        let name_lower = name.to_lowercase();
        let cat_lower = category.to_lowercase();

        // 2. Tag, ID, or name explicitly Cloud
        if id_lower.ends_with(":cloud")
            || id_lower.contains("-cloud")
            || name_lower.contains("cloud")
            || cat_lower.contains("cloud")
            || cat_lower.contains("distant")
            || cat_lower.contains("remote")
        {
            return true;
        }

        false
    }

    pub fn cache_path() -> std::path::PathBuf {
        let config_dir = std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join(".config/neuradex"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        config_dir.join("catalog_cache.json")
    }

    /// Loads the catalog from local cache if available, otherwise returns the default catalog
    pub fn load_catalog() -> Vec<HubModelInfo> {
        let p = Self::cache_path();
        if p.exists() {
            if let Ok(data) = std::fs::read_to_string(&p) {
                if let Ok(mut cached) = serde_json::from_str::<Vec<HubModelInfo>>(&data) {
                    if !cached.is_empty() {
                        // Filter out deprecated models
                        cached.retain(|m| !m.is_deprecated && m.status.as_deref() != Some("deprecated"));

                        for m in &mut cached {
                            if Self::is_cloud_model(&m.id, &m.name, &m.description, &m.category, &m.variants) {
                                m.is_cloud = true;
                                if m.category == "Général & Chat" || m.category == "general" {
                                    m.category = "cloud".to_string();
                                }
                            }
                        }
                        return cached;
                    }
                }
            }
        }
        vec![]
    }

    /// Saves models into the local cache
    pub fn save_cache(models: &[HubModelInfo]) {
        let p = Self::cache_path();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(models) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
}

