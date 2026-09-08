use crate::api::error::ApiError;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
}

pub struct GitHubClient;

impl GitHubClient {
    pub async fn check_latest_release(current_version: &str) -> Result<UpdateCheckResult, ApiError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        let url = "https://api.github.com/repos/ollama/ollama/releases/latest";
        let resp = client
            .get(url)
            .header("User-Agent", concat!("NeuraDex/", env!("CARGO_PKG_VERSION"), " (Linux; GTK4)"))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ApiError::HttpStatus {
                status: resp.status().as_u16(),
                message: format!("GitHub API error: HTTP {}", resp.status()),
            });
        }

        let release: GitHubRelease = resp.json().await?;

        let clean_latest = release.tag_name.trim_start_matches('v').trim();
        let clean_current = current_version.trim_start_matches('v').trim();

        let has_update = Self::is_newer_version(clean_current, clean_latest);

        Ok(UpdateCheckResult {
            current_version: current_version.to_string(),
            latest_version: release.tag_name,
            has_update,
        })
    }

    fn is_newer_version(current: &str, latest: &str) -> bool {
        let parse_version = |v: &str| -> Vec<u64> {
            v.split('.')
                .filter_map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
                .collect()
        };

        let curr_parts = parse_version(current);
        let latest_parts = parse_version(latest);

        for (c, l) in curr_parts.iter().zip(latest_parts.iter()) {
            if l > c {
                return true;
            } else if l < c {
                return false;
            }
        }

        latest_parts.len() > curr_parts.len()
    }
}
