use crate::api::client::OllamaClient;
use crate::api::error::ApiError;
use crate::api::hub_remote::{parse_hf_identifier, HfGgufFile, OllamaWebClient};
use crate::api::types::PullProgress;
use futures_util::StreamExt;
use reqwest::StatusCode;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

/// Manages direct downloading of GGUF models from Hugging Face and registering them into Ollama.
pub async fn download_and_register_hf_model<F>(
    client: &OllamaClient,
    model_tag: &str,
    hf_token: Option<&str>,
    mut on_progress: F,
) -> Result<(), ApiError>
where
    F: FnMut(PullProgress) -> bool,
{
    // 1. Parse repository ID and target tag/quantization
    let (repo_id, tag_opt) = parse_hf_identifier(model_tag).ok_or_else(|| {
        ApiError::Custom(format!("Invalid Hugging Face model identifier: '{}'", model_tag))
    })?;

    let repo_clean = repo_id.trim().trim_start_matches('/').trim_end_matches('/');

    let _ = on_progress(PullProgress {
        status: format!("Resolving Hugging Face repository '{}'...", repo_clean),
        digest: None,
        total: None,
        completed: None,
        error: None,
    });

    // 2. Fetch repository metadata and GGUF files list
    let web_client = OllamaWebClient::new();
    let details = web_client.fetch_hf_repo_details(repo_clean, hf_token).await?;

    if details.files.is_empty() {
        return Err(ApiError::Custom(format!(
            "No GGUF files found in Hugging Face repository '{}'.",
            repo_clean
        )));
    }

    // 3. Match the requested quantization or filename
    let chosen_file = find_matching_file(&details.files, tag_opt.as_deref())
        .ok_or_else(|| {
            ApiError::Custom(format!(
                "Could not find matching quantization or GGUF file for tag '{:?}' in repository '{}'.",
                tag_opt, repo_clean
            ))
        })?;

    // 4. Prepare local download directory
    let cache_dir = glib::user_cache_dir().join("neuradex").join("hf_downloads");
    let safe_repo_dir = repo_clean.replace('/', "_");
    let target_dir = cache_dir.join(&safe_repo_dir);
    tokio::fs::create_dir_all(&target_dir).await.map_err(|e| {
        ApiError::Custom(format!("Failed to create download directory '{:?}': {}", target_dir, e))
    })?;

    // Determine the list of files to download (supports single file or multi-part shards)
    let files_to_download = if !chosen_file.files.is_empty() {
        chosen_file.files.clone()
    } else {
        vec![chosen_file.filename.clone()]
    };

    let overall_total = chosen_file.size_bytes;
    let mut overall_completed: u64 = 0;

    let download_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(86400)) // 24h streaming timeout
        .user_agent(concat!("NeuraDex/", env!("CARGO_PKG_VERSION"), " (Direct HF Downloader)"))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // 5. Download each file / shard sequentially with resume support
    for filename in &files_to_download {
        let final_path = target_dir.join(filename);
        let part_path = target_dir.join(format!("{}.part", filename));

        // If subdirectories exist inside the repository tree, ensure parent dir exists locally
        if let Some(parent) = final_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        // Check if fully downloaded already
        if final_path.exists() && !part_path.exists() {
            if let Ok(meta) = std::fs::metadata(&final_path) {
                let len = meta.len();
                if len > 0 {
                    overall_completed = overall_completed.saturating_add(len);
                    let _ = on_progress(PullProgress {
                        status: format!("Cached: {}", filename),
                        digest: None,
                        total: if overall_total > 0 { Some(overall_total) } else { None },
                        completed: Some(overall_completed),
                        error: None,
                    });
                    continue;
                }
            }
        }

        // Check partial size for HTTP Range resume
        let mut existing_len: u64 = 0;
        if part_path.exists() {
            if let Ok(meta) = std::fs::metadata(&part_path) {
                existing_len = meta.len();
            }
        }

        let download_url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo_clean, filename
        );

        let mut req = download_client.get(&download_url);
        if let Some(token) = hf_token {
            let t = token.trim();
            if !t.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", t));
            }
        }

        if existing_len > 0 {
            req = req.header("Range", format!("bytes={}-", existing_len));
        }

        let resp = req.send().await.map_err(|e| {
            ApiError::Custom(format!("Network error while contacting Hugging Face: {}", e))
        })?;

        let status = resp.status();
        if !status.is_success() && status != StatusCode::PARTIAL_CONTENT {
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message: format!("Failed to download '{}' from Hugging Face (HTTP {})", filename, status),
            });
        }

        let is_resumed = status == StatusCode::PARTIAL_CONTENT;
        let mut file = if is_resumed {
            overall_completed = overall_completed.saturating_add(existing_len);
            tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&part_path)
                .await
                .map_err(ApiError::Io)?
        } else {
            // Server did not accept Range or starting fresh
            tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&part_path)
                .await
                .map_err(ApiError::Io)?
        };

        let mut stream = resp.bytes_stream();
        let mut last_emit = Instant::now();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(|e| {
                ApiError::Custom(format!("Streaming interrupted during download of '{}': {}", filename, e))
            })?;

            file.write_all(&chunk).await.map_err(ApiError::Io)?;
            overall_completed = overall_completed.saturating_add(chunk.len() as u64);

            // Throttle UI progress notifications to avoid channel flooding
            if last_emit.elapsed() >= Duration::from_millis(120) || overall_completed >= overall_total {
                last_emit = Instant::now();
                let keep_going = on_progress(PullProgress {
                    status: format!("downloading {}", filename),
                    digest: None,
                    total: if overall_total > 0 { Some(overall_total) } else { None },
                    completed: Some(overall_completed),
                    error: None,
                });

                if !keep_going {
                    let _ = file.flush().await;
                    return Err(ApiError::Custom("Download cancelled by user".to_string()));
                }
            }
        }

        file.flush().await.map_err(ApiError::Io)?;
        drop(file);

        // Rename .part to completed file name
        tokio::fs::rename(&part_path, &final_path).await.map_err(|e| {
            ApiError::Custom(format!("Failed to finalize downloaded file '{}': {}", filename, e))
        })?;
    }

    // 6. Register model into Ollama via Modelfile
    let first_file = files_to_download.first().unwrap_or(&chosen_file.filename);
    let main_gguf_path = target_dir.join(first_file);

    let _ = on_progress(PullProgress {
        status: "registering model into Ollama...".to_string(),
        digest: None,
        total: if overall_total > 0 { Some(overall_total) } else { None },
        completed: Some(overall_completed),
        error: None,
    });

    let modelfile = format!("FROM {}\n", main_gguf_path.to_string_lossy());

    let create_res = client
        .create_model_stream(model_tag, &modelfile, |cp| {
            on_progress(PullProgress {
                status: cp.status,
                digest: cp.digest,
                total: cp.total,
                completed: cp.completed,
                error: cp.error,
            })
        })
        .await;

    // 7. Cleanup temporary GGUF files after successful Ollama ingestion
    if create_res.is_ok() {
        for f in &files_to_download {
            let p = target_dir.join(f);
            let _ = tokio::fs::remove_file(&p).await;
        }
        let _ = tokio::fs::remove_dir(&target_dir).await;

        let _ = on_progress(PullProgress {
            status: "success".to_string(),
            digest: None,
            total: if overall_total > 0 { Some(overall_total) } else { None },
            completed: Some(overall_completed),
            error: None,
        });
    }

    create_res
}

/// Helper function to match target tag to one of the repository files
fn find_matching_file<'a>(files: &'a [HfGgufFile], tag_opt: Option<&str>) -> Option<&'a HfGgufFile> {
    if let Some(target) = tag_opt {
        let trimmed = target.trim();

        // 1. Exact match on quantization (case-insensitive)
        if let Some(f) = files.iter().find(|f| f.quantization.eq_ignore_ascii_case(trimmed)) {
            return Some(f);
        }

        // 2. Exact match on base filename
        if let Some(f) = files.iter().find(|f| f.filename.eq_ignore_ascii_case(trimmed)) {
            return Some(f);
        }

        // 3. Match on tag suffix
        if let Some(f) = files.iter().find(|f| f.tag.ends_with(&format!(":{}", trimmed))) {
            return Some(f);
        }

        // 4. Normalized variants (Q4_K_M vs Q4-K-M)
        let alt = if trimmed.contains('_') {
            trimmed.replace('_', "-")
        } else {
            trimmed.replace('-', "_")
        };

        if let Some(f) = files.iter().find(|f| f.quantization.eq_ignore_ascii_case(&alt)) {
            return Some(f);
        }
    }

    // Default fallback: recommended quantization or first available
    files.iter().find(|f| f.is_recommended).or_else(|| files.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matching_file_quantization() {
        let files = vec![
            HfGgufFile {
                filename: "model.Q4_K_M.gguf".to_string(),
                files: vec!["model.Q4_K_M.gguf".to_string()],
                quantization: "Q4_K_M".to_string(),
                description: "".to_string(),
                size_bytes: 4_000_000_000,
                size_formatted: "4.0 GB".to_string(),
                shard_count: 1,
                tag: "hf.co/user/model:Q4_K_M".to_string(),
                is_recommended: true,
            },
            HfGgufFile {
                filename: "model.Q8_0.gguf".to_string(),
                files: vec!["model.Q8_0.gguf".to_string()],
                quantization: "Q8_0".to_string(),
                description: "".to_string(),
                size_bytes: 8_000_000_000,
                size_formatted: "8.0 GB".to_string(),
                shard_count: 1,
                tag: "hf.co/user/model:Q8_0".to_string(),
                is_recommended: false,
            },
        ];

        // Exact match
        let m1 = find_matching_file(&files, Some("Q4_K_M"));
        assert_eq!(m1.unwrap().quantization, "Q4_K_M");

        // Case insensitive
        let m2 = find_matching_file(&files, Some("q8_0"));
        assert_eq!(m2.unwrap().quantization, "Q8_0");

        // Dash vs Underscore
        let m3 = find_matching_file(&files, Some("Q4-K-M"));
        assert_eq!(m3.unwrap().quantization, "Q4_K_M");

        // Filename match
        let m4 = find_matching_file(&files, Some("model.Q8_0.gguf"));
        assert_eq!(m4.unwrap().quantization, "Q8_0");

        // None -> recommended
        let m_def = find_matching_file(&files, None);
        assert_eq!(m_def.unwrap().quantization, "Q4_K_M");
    }
}
