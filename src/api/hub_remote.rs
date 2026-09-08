use crate::api::error::ApiError;
use crate::core::hardware::estimator::format_bytes;
use crate::core::hub::{HubModelInfo, RemoteCatalogResponse};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HfGgufFile {
    pub filename: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub quantization: String,
    pub description: String,
    pub size_bytes: u64,
    pub size_formatted: String,
    pub shard_count: usize,
    pub tag: String,
    pub is_recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HfRepoDetails {
    pub author: String,
    pub model_name: String,
    pub downloads: u64,
    pub page_url: String,
    pub files: Vec<HfGgufFile>,
}

#[derive(Debug, Deserialize)]
struct HfTreeItem {
    pub path: String,
    pub size: Option<u64>,
    pub lfs: Option<HfLfsItem>,
}

#[derive(Debug, Deserialize)]
struct HfLfsItem {
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct HfModelDetailResponse {
    pub author: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    siblings: Vec<HfSiblingItem>,
}

#[derive(Debug, Deserialize)]
struct HfSiblingItem {
    rfilename: String,
}

pub struct OllamaWebClient {
    http_client: Client,
}

impl Default for OllamaWebClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaWebClient {
    pub fn new() -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent(concat!("NeuraDex/", env!("CARGO_PKG_VERSION"), " (Linux; x86_64; GNOME Libadwaita)"))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { http_client }
    }

    /// Downloads the complete official Ollama catalog from remote CDN (jsDelivr / Raw GitHub)
    pub async fn fetch_library_catalog(&self) -> Result<Vec<HubModelInfo>, ApiError> {
        let primary_url = "https://cdn.jsdelivr.net/gh/BazinFla/ai-models-list@main/ollama/ollama-list.json";
        let fallback_url = "https://raw.githubusercontent.com/BazinFla/ai-models-list/main/ollama/ollama-list.json";

        // 1. Try jsDelivr CDN
        match self.fetch_json_catalog(primary_url).await {
            Ok(models) => {
                if !models.is_empty() {
                    Ok(models)
                } else {
                    self.fetch_json_catalog(fallback_url).await
                }
            }
            Err(e) => {
                tracing::warn!("jsDelivr CDN failed ({}), trying Raw GitHub mirror...", e);
                // 2. Fallback to Raw GitHub
                self.fetch_json_catalog(fallback_url).await
            }
        }
    }

    async fn fetch_json_catalog(&self, url: &str) -> Result<Vec<HubModelInfo>, ApiError> {
        let resp = self.http_client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(ApiError::HttpStatus {
                status: resp.status().as_u16(),
                message: format!("HTTP error from {}", url),
            });
        }

        let catalog: RemoteCatalogResponse = resp.json().await?;

        let mut models = catalog.models;
        // Filter out deprecated models
        models.retain(|m| !m.is_deprecated && m.status.as_deref() != Some("deprecated"));

        if models.is_empty() {
            return Err(ApiError::Custom("Remote catalog has no active models".to_string()));
        }

        Ok(models)
    }

    /// Retrieves all GGUF files and quantizations from a Hugging Face repository
    pub async fn fetch_hf_repo_details(
        &self,
        repo_id: &str,
        hf_token: Option<&str>,
    ) -> Result<HfRepoDetails, ApiError> {
        let repo_clean = repo_id.trim().trim_start_matches('/').trim_end_matches('/');

        // 1. Retrieve model metadata
        let meta_url = format!("https://huggingface.co/api/models/{}", repo_clean);
        let mut meta_req = self.http_client.get(&meta_url);
        if let Some(token) = hf_token {
            let t = token.trim();
            if !t.is_empty() {
                meta_req = meta_req.header("Authorization", format!("Bearer {}", t));
            }
        }

        let meta_resp = meta_req.send().await?;

        if !meta_resp.status().is_success() {
            return Err(ApiError::HttpStatus {
                status: meta_resp.status().as_u16(),
                message: format!("Hugging Face model not found or inaccessible: {}", repo_clean),
            });
        }

        let meta: HfModelDetailResponse = meta_resp.json().await?;

        let author = meta.author.unwrap_or_else(|| {
            repo_clean
                .split('/')
                .next()
                .unwrap_or("Hugging Face")
                .to_string()
        });
        let model_name = repo_clean
            .split('/')
            .next_back()
            .unwrap_or(repo_clean)
            .to_string();

        // 2. Retrieve file tree
        let tree_url = format!("https://huggingface.co/api/models/{}/tree/main", repo_clean);
        let mut tree_req = self.http_client.get(&tree_url);
        if let Some(token) = hf_token {
            let t = token.trim();
            if !t.is_empty() {
                tree_req = tree_req.header("Authorization", format!("Bearer {}", t));
            }
        }

        let tree_resp = tree_req.send().await;
        let mut raw_files: Vec<(String, u64)> = Vec::new();

        if let Ok(resp) = tree_resp {
            if resp.status().is_success() {
                if let Ok(tree_items) = resp.json::<Vec<HfTreeItem>>().await {
                    for item in tree_items {
                        if item.path.to_lowercase().ends_with(".gguf") {
                            let size = item
                                .lfs
                                .and_then(|l| l.size)
                                .or(item.size)
                                .unwrap_or(0);
                            raw_files.push((item.path, size));
                        }
                    }
                }
            }
        }

        // Fallback to metadata siblings if tree/main returned no files
        if raw_files.is_empty() {
            for sibling in &meta.siblings {
                if sibling.rfilename.to_lowercase().ends_with(".gguf") {
                    raw_files.push((sibling.rfilename.clone(), 0));
                }
            }
        }

        if raw_files.is_empty() {
            return Err(ApiError::Custom(format!(
                "No GGUF files (.gguf) found in repository '{}'.",
                repo_clean
            )));
        }

        // 3. Aggregate files by quantization and merge shards
        struct QuantGroup {
            base_filename: String,
            files: Vec<String>,
            quantization: String,
            description: String,
            total_size: u64,
            shards: usize,
            tag_suffix: String,
        }

        let mut groups: HashMap<String, QuantGroup> = HashMap::new();

        for (filepath, size) in raw_files {
            let (quant, desc, shard_info, base_stem) = extract_quant_info(&filepath);

            let group = groups.entry(quant.clone()).or_insert_with(|| {
                let tag_suffix = if !quant.is_empty() && quant != "GGUF" {
                    quant.to_string()
                } else {
                    filepath.clone()
                };

                QuantGroup {
                    base_filename: base_stem,
                    files: Vec::new(),
                    quantization: quant,
                    description: desc,
                    total_size: 0,
                    shards: 0,
                    tag_suffix,
                }
            });

            group.files.push(filepath.clone());
            group.total_size += size;
            group.shards = group.shards.max(shard_info.map(|s| s.1).unwrap_or(1));
        }

        let mut files: Vec<HfGgufFile> = groups
            .into_values()
            .map(|mut g| {
                g.files.sort();
                let tag = format!("hf.co/{}:{}", repo_clean, g.tag_suffix);
                let q_upper = g.quantization.to_uppercase();
                let is_recommended = q_upper == "Q4_K_M" || q_upper == "Q4_K";

                HfGgufFile {
                    filename: g.base_filename,
                    files: g.files,
                    quantization: g.quantization,
                    description: g.description,
                    size_bytes: g.total_size,
                    size_formatted: format_bytes(g.total_size),
                    shard_count: g.shards,
                    tag,
                    is_recommended,
                }
            })
            .collect();

        // Smart sorting: Q4_K_M first, then by quantization rank / size
        files.sort_by(|a, b| {
            let rank_a = quant_sort_rank(&a.quantization);
            let rank_b = quant_sort_rank(&b.quantization);
            if rank_a != rank_b {
                rank_a.cmp(&rank_b)
            } else {
                b.size_bytes.cmp(&a.size_bytes)
            }
        });

        // If none is explicitly marked recommended, mark the first available Q4
        if !files.iter().any(|f| f.is_recommended) {
            if let Some(first_q4) = files.iter_mut().find(|f| f.quantization.starts_with("Q4") || f.quantization.starts_with("IQ4")) {
                first_q4.is_recommended = true;
            } else if let Some(first) = files.first_mut() {
                first.is_recommended = true;
            }
        }

        Ok(HfRepoDetails {
            author,
            model_name,
            downloads: meta.downloads,
            page_url: format!("https://huggingface.co/{}", repo_clean),
            files,
        })
    }
}

/// Determines the base sort order for a quantization
fn quant_sort_rank(quant: &str) -> u32 {
    let q = quant.to_uppercase();
    match q.as_str() {
        "Q4_K_M" => 1,
        "Q4_K_S" => 2,
        "Q4_K_L" | "Q4_K" => 3,
        "Q4_0" | "Q4_1" => 4,
        "IQ4_XS" | "IQ4_NL" | "IQ4_S" | "IQ4_M" => 5,
        "Q5_K_M" => 6,
        "Q5_K_S" => 7,
        "Q5_0" | "Q5_1" => 8,
        "Q6_K" | "Q6_0" => 9,
        "Q8_0" | "Q8_K" | "Q8_1" => 10,
        "Q3_K_M" | "Q3_K_L" => 11,
        "Q3_K_S" | "Q3_K_XS" => 12,
        "IQ3_M" | "IQ3_S" | "IQ3_XS" | "IQ3_XXS" => 13,
        "Q2_K" | "Q2_K_L" => 14,
        "IQ2_M" | "IQ2_S" | "IQ2_XS" | "IQ2_XXS" => 15,
        "BF16" | "FP16" | "F16" => 16,
        "FP32" | "F32" => 17,
        _ => 50,
    }
}

/// Parses a string to determine if it is a Hugging Face repository
/// and returns `Some((repo_id, optional_tag))`
pub fn parse_hf_identifier(input: &str) -> Option<(String, Option<String>)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Full URLs (https://huggingface.co/owner/repo or hf.co/owner/repo)
    let cleaned = if let Some(rest) = trimmed.strip_prefix("https://huggingface.co/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://huggingface.co/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("huggingface.co/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("hf.co/") {
        rest
    } else {
        trimmed
    };

    // Strip /tree/main, /blob/main, etc.
    let cleaned = cleaned.trim_end_matches('/');
    let cleaned_no_tree = if let Some(idx) = cleaned.find("/tree/") {
        &cleaned[..idx]
    } else if let Some(idx) = cleaned.find("/blob/") {
        &cleaned[..idx]
    } else {
        cleaned
    };

    // Extract :quantization tag if present
    let (repo_part, tag_opt) = if let Some(idx) = cleaned_no_tree.rfind(':') {
        let (r, t) = cleaned_no_tree.split_at(idx);
        let tag_val = &t[1..];
        (r, if tag_val.is_empty() { None } else { Some(tag_val.to_string()) })
    } else {
        (cleaned_no_tree, None)
    };

    let segments: Vec<&str> = repo_part.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() == 2 {
        let owner = segments[0];
        let repo = segments[1];
        if owner != "library" && owner != "registry.ollama.ai" && !owner.is_empty() && !repo.is_empty() {
            return Some((format!("{}/{}", owner, repo), tag_opt));
        }
    }

    None
}

/// Extracts quantization, description, sharding info, and base name from a GGUF file
fn extract_quant_info(filename: &str) -> (String, String, Option<(usize, usize)>, String) {
    static SHARD_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[-_.](\d{4,5})-of-(\d{4,5})").expect("Invalid shard regex pattern"));

    let clean_name = filename.split('/').next_back().unwrap_or(filename);
    let stem = clean_name.strip_suffix(".gguf").unwrap_or(clean_name);

    // Detect shards (e.g. 00001-of-00003 or part1of2)
    let mut shard_info = None;
    let mut base_stem = stem.to_string();

    if let Some(caps) = SHARD_RE.captures(stem) {
        let current = caps.get(1).and_then(|m| m.as_str().parse::<usize>().ok()).unwrap_or(1);
        let total = caps.get(2).and_then(|m| m.as_str().parse::<usize>().ok()).unwrap_or(1);
        shard_info = Some((current, total));
        base_stem = SHARD_RE.replace_all(&base_stem, "").to_string();
    }

    // Search for standard quantization patterns
    let quant_patterns = [
        ("IQ1_S", "1-bit I-Quant (Ultra-compact)"),
        ("IQ1_M", "1-bit I-Quant (Ultra-compact)"),
        ("IQ2_XXS", "2-bit I-Quant (Extreme compression)"),
        ("IQ2_XS", "2-bit I-Quant (Extreme compression)"),
        ("IQ2_S", "2-bit I-Quant (Extreme compression)"),
        ("IQ2_M", "2-bit I-Quant (Extreme compression)"),
        ("Q2_K_L", "2-bit standard (Maximum compression)"),
        ("Q2_K_S", "2-bit standard (Maximum compression)"),
        ("Q2_K", "2-bit standard (Maximum compression)"),
        ("Q2_0", "2-bit legacy"),
        ("IQ3_XXS", "3-bit I-Quant (Ultra-compact)"),
        ("IQ3_XS", "3-bit I-Quant compact"),
        ("IQ3_S", "3-bit I-Quant compact"),
        ("IQ3_M", "3-bit I-Quant balanced"),
        ("Q3_K_L", "3-bit high precision"),
        ("Q3_K_M", "3-bit standard (Good compression)"),
        ("Q3_K_S", "3-bit compact"),
        ("Q3_K_XS", "3-bit ultra-compact"),
        ("Q3_K", "3-bit standard"),
        ("Q3_0", "3-bit legacy"),
        ("IQ4_NL", "4-bit non-linear I-Quant (Optimized)"),
        ("IQ4_XS", "4-bit compact I-Quant (Very efficient)"),
        ("IQ4_S", "4-bit I-Quant"),
        ("IQ4_M", "4-bit I-Quant"),
        ("Q4_K_M", "Recommended • 4-bit standard (Ideal speed/quality balance)"),
        ("Q4_K_S", "4-bit compact (Low VRAM footprint)"),
        ("Q4_K_L", "4-bit large"),
        ("Q4_K", "4-bit standard"),
        ("Q4_0_4_4", "4-bit optimized ARM/Metal"),
        ("Q4_0_8_8", "4-bit optimized AVX"),
        ("Q4_0", "4-bit standard legacy"),
        ("Q4_1", "4-bit standard legacy"),
        ("IQ5_XS", "5-bit I-Quant compact"),
        ("IQ5_S", "5-bit I-Quant"),
        ("IQ5_M", "5-bit I-Quant"),
        ("Q5_K_M", "5-bit standard (High fidelity)"),
        ("Q5_K_S", "5-bit compact"),
        ("Q5_K_L", "5-bit large"),
        ("Q5_K", "5-bit standard"),
        ("Q5_0", "5-bit standard"),
        ("Q5_1", "5-bit standard"),
        ("Q6_K_L", "6-bit large"),
        ("Q6_K_M", "6-bit standard"),
        ("Q6_K", "6-bit very high fidelity"),
        ("Q6_0", "6-bit standard"),
        ("Q8_0", "8-bit maximum precision (Near-perfect fidelity)"),
        ("Q8_1", "8-bit precision"),
        ("Q8_K", "8-bit precision"),
        ("BF16", "16-bit Bfloat (Original unquantized weights)"),
        ("FP16", "16-bit Float (Original unquantized weights)"),
        ("F16", "16-bit Float (Original unquantized weights)"),
        ("FP32", "32-bit Full precision"),
        ("F32", "32-bit Full precision"),
    ];

    let upper_stem = base_stem.to_uppercase();

    for (q_name, q_desc) in quant_patterns {
        let pattern_underscore = format!("_{}", q_name);
        let pattern_dash = format!("-{}", q_name);
        let pattern_dot = format!(".{}", q_name);

        if upper_stem.ends_with(&pattern_underscore)
            || upper_stem.ends_with(&pattern_dash)
            || upper_stem.ends_with(&pattern_dot)
            || upper_stem.contains(&format!("{}-", pattern_dash))
            || upper_stem.contains(&format!("{}_", pattern_underscore))
            || upper_stem == q_name
        {
            return (
                q_name.to_string(),
                q_desc.to_string(),
                shard_info,
                base_stem,
            );
        }
    }

    ("GGUF".to_string(), "GGUF File".to_string(), shard_info, base_stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hf_identifier() {
        assert_eq!(
            parse_hf_identifier("https://huggingface.co/OBLITERATUS/Qwen3.8-27B-OBLITERATED"),
            Some(("OBLITERATUS/Qwen3.8-27B-OBLITERATED".to_string(), None))
        );
        assert_eq!(
            parse_hf_identifier("https://huggingface.co/OBLITERATUS/Qwen3.8-27B-OBLITERATED/tree/main"),
            Some(("OBLITERATUS/Qwen3.8-27B-OBLITERATED".to_string(), None))
        );
        assert_eq!(
            parse_hf_identifier("hf.co/bartowski/DeepSeek-R1-Distill-Qwen-14B-GGUF:Q4_K_M"),
            Some(("bartowski/DeepSeek-R1-Distill-Qwen-14B-GGUF".to_string(), Some("Q4_K_M".to_string())))
        );
        assert_eq!(
            parse_hf_identifier("OBLITERATUS/Qwen3.8-27B-OBLITERATED"),
            Some(("OBLITERATUS/Qwen3.8-27B-OBLITERATED".to_string(), None))
        );
        assert_eq!(
            parse_hf_identifier("llama3.2:3b"),
            None
        );
        assert_eq!(
            parse_hf_identifier("mistral"),
            None
        );
    }

    #[test]
    fn test_extract_quant_info() {
        let (q, desc, shards, _) = extract_quant_info("Qwen3.8-27B-OBLITERATED-Q4_K_M.gguf");
        assert_eq!(q, "Q4_K_M");
        assert!(desc.contains("Recommended"));
        assert_eq!(shards, None);

        let (q_shard, _, shards_found, _) = extract_quant_info("Qwen3.8-27B-OBLITERATED-Q8_0-00001-of-00003.gguf");
        assert_eq!(q_shard, "Q8_0");
        assert_eq!(shards_found, Some((1, 3)));
    }
}
