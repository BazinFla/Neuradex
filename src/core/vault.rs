use crate::core::config::{ApiProfile, ApiProviderType};
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct HfWhoamiResponse {
    pub name: Option<String>,
    pub fullname: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub message: String,
}

/// Represents an SSH key pair generated or configured on the system
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SshKeyInfo {
    /// Absolute path to private key (e.g. ~/.ollama/id_ed25519)
    pub private_key_path: PathBuf,
    /// Absolute path to public key (e.g. ~/.ollama/id_ed25519.pub)
    pub public_key_path: PathBuf,
    /// Public key content (e.g. "ssh-ed25519 AAAAC3Nza...")
    pub public_key_content: String,
    /// Human-readable label for display (e.g. "🔑 ~/.ollama/id_ed25519 (Ed25519)")
    pub display_label: String,
    /// Key type (ed25519, rsa, ecdsa)
    pub key_type: String,
    /// Key source (ollama, ssh)
    pub source: String,
}

pub struct ApiVault;

impl ApiVault {

    /// Cleans an SSH public key to retain only the key type and base64 blob (without trailing comment/name).
    /// E.g. "ssh-ed25519 AAAAC3NzaC1... member@neuradex" -> "ssh-ed25519 AAAAC3NzaC1..."
    pub fn clean_public_key(raw: &str) -> String {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.len() >= 2 {
            format!("{} {}", parts[0], parts[1])
        } else {
            raw.trim().to_string()
        }
    }

    /// Retrieves the cleaned SSH public key associated with a profile (if available).
    pub fn get_public_key_for_profile(profile: &ApiProfile) -> Option<String> {
        if profile.provider_type == ApiProviderType::OllamaSsh {
            let token = profile.token.trim();
            if token.starts_with("ssh-") {
                Some(Self::clean_public_key(token))
            } else {
                let pub_path = PathBuf::from(format!("{}.pub", token));
                if pub_path.is_file() {
                    std::fs::read_to_string(&pub_path).ok().map(|s| Self::clean_public_key(&s))
                } else {
                    None
                }
            }
        } else {
            None
        }
    }

    /// Retrieves the first Hugging Face token configured in Vault profiles
    pub fn get_hf_token(profiles: &[ApiProfile]) -> Option<String> {
        profiles
            .iter()
            .find(|p| p.provider_type == ApiProviderType::HuggingFace)
            .and_then(|p| {
                let token = crate::core::secret_store::SecretStore::resolve_token(p);
                if token.trim().is_empty() {
                    None
                } else {
                    Some(token.trim().to_string())
                }
            })
    }


    /// Generates a new dedicated Ed25519 SSH key pair for a profile/member in ~/.ollama/
    pub fn generate_ssh_key_pair(profile_name: &str) -> Result<SshKeyInfo, String> {
        let home = std::env::var("HOME").map_err(|_| "Variable HOME introuvable".to_string())?;
        let ollama_dir = PathBuf::from(&home).join(".ollama");
        if !ollama_dir.exists() {
            let _ = std::fs::create_dir_all(&ollama_dir);
        }

        let clean_name = profile_name
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let clean_name = if clean_name.is_empty() { "member".to_string() } else { clean_name };

        let priv_path = ollama_dir.join(format!("id_ed25519_{}", clean_name));
        let pub_path = ollama_dir.join(format!("id_ed25519_{}.pub", clean_name));

        if priv_path.exists() {
            return Err(crate::t!("vault.priv_key_exists", path = priv_path.display()));
        }

        let comment = format!("{}@neuradex", profile_name.trim());
        let output = std::process::Command::new("ssh-keygen")
            .args([
                "-t", "ed25519",
                "-f", priv_path.to_str().unwrap_or_default(),
                "-N", "",
                "-C", &comment,
            ])
            .output()
            .map_err(|e| crate::t!("vault.ssh_keygen_exec_failed", err = e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(crate::t!("vault.ssh_keygen_failed", err = err));
        }

        let pub_content = std::fs::read_to_string(&pub_path)
            .map_err(|e| crate::t!("vault.pub_key_read_failed", err = e))?;

        let display_label = format!("🔑 ~/.ollama/id_ed25519_{} (Ed25519 · ollama)", clean_name);

        Ok(SshKeyInfo {
            private_key_path: priv_path,
            public_key_path: pub_path,
            public_key_content: Self::clean_public_key(&pub_content),
            display_label,
            key_type: "Ed25519".to_string(),
            source: "ollama".to_string(),
        })
    }

    pub async fn validate_profile(profile: &ApiProfile) -> ValidationResult {
        let token = crate::core::secret_store::SecretStore::resolve_token(profile);
        match profile.provider_type {
            ApiProviderType::OllamaCloud => {
                let endpoint = profile.endpoint.as_deref().unwrap_or("https://ollama.com");
                Self::validate_ollama_cloud(endpoint, &token).await
            }
            ApiProviderType::OllamaSsh => Self::validate_ollama_ssh(&token),
            ApiProviderType::HuggingFace => Self::validate_huggingface(&token).await,
            ApiProviderType::OllamaRemote => {
                let endpoint = profile.endpoint.as_deref().unwrap_or("http://127.0.0.1:11434");
                Self::validate_ollama_remote(endpoint, &token).await
            }
            ApiProviderType::CustomCloud => {
                let endpoint = profile.endpoint.as_deref().unwrap_or("");
                Self::validate_custom_cloud(endpoint, &token).await
            }
        }
    }

    /// Validates a local SSH key by verifying that private and public keys exist.
    /// The token holds the private key path (e.g. /home/user/.ollama/id_ed25519)
    pub fn validate_ollama_ssh(private_key_path: &str) -> ValidationResult {
        let path = PathBuf::from(private_key_path.trim());

        if !path.is_file() {
            return ValidationResult {
                is_valid: false,
                message: crate::t!("vault.priv_key_not_found", path = private_key_path),
            };
        }

        let pub_path = PathBuf::from(format!("{}.pub", private_key_path.trim()));
        if !pub_path.is_file() {
            return ValidationResult {
                is_valid: false,
                message: crate::t!("vault.pub_key_not_found", path = private_key_path),
            };
        }

        // Read public key to verify format
        let pub_content = match std::fs::read_to_string(&pub_path) {
            Ok(c) => c.trim().to_string(),
            Err(e) => {
                return ValidationResult {
                    is_valid: false,
                    message: crate::t!("vault.pub_key_read_failed", err = e),
                };
            }
        };

        let key_type = if pub_content.starts_with("ssh-ed25519") {
            "Ed25519"
        } else if pub_content.starts_with("ssh-rsa") {
            "RSA"
        } else if pub_content.starts_with("ecdsa-") {
            "ECDSA"
        } else {
            &crate::t!("vault.key_unknown")
        };

        // Extract fingerprint (first 12 characters of base64 content)
        let fingerprint = pub_content
            .split_whitespace()
            .nth(1)
            .map(|b64| &b64[..b64.len().min(12)])
            .unwrap_or("???");

        let home_str = std::env::var("HOME").unwrap_or_default();
        let short_priv = private_key_path.replace(&home_str, "~");
        let short_pub = pub_path.to_string_lossy().replace(&home_str, "~");

        ValidationResult {
            is_valid: true,
            message: crate::t!(
                "vault.ssh_key_valid",
                key_type = key_type,
                priv_path = short_priv,
                pub_path = short_pub,
                fp = fingerprint
            ),
        }
    }

    /// Validates an Ollama Cloud API key (Bearer token) live against ollama.com
    pub async fn validate_ollama_cloud(endpoint: &str, token: &str) -> ValidationResult {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return ValidationResult {
                is_valid: false,
                message: crate::t!("vault.ollama_cloud_empty"),
            };
        }

        let clean_ep = endpoint.trim().trim_end_matches('/');
        let base_url = if clean_ep.is_empty() { "https://ollama.com" } else { clean_ep };

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        // POST /api/generate requires a valid Bearer token and returns 401 if key is invalid
        let test_url = format!("{}/api/generate", base_url);
        let resp = client
            .post(&test_url)
            .header("Authorization", format!("Bearer {}", trimmed))
            .header("User-Agent", concat!("NeuraDex/", env!("CARGO_PKG_VERSION")))
            .header("Content-Type", "application/json")
            .body(r#"{"model":"library/llama3.2:1b","prompt":"test","stream":false,"options":{"num_predict":1}}"#)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status().as_u16();
                if status == 401 || status == 403 {
                    ValidationResult {
                        is_valid: false,
                        message: crate::t!("vault.ollama_cloud_invalid"),
                    }
                } else {
                    ValidationResult {
                        is_valid: true,
                        message: crate::t!("vault.ollama_cloud_success"),
                    }
                }
            }
            Err(e) => ValidationResult {
                is_valid: false,
                message: crate::t!("vault.cannot_reach", url = base_url, err = e),
            },
        }
    }

    /// Validates a Hugging Face token live against the whoami API
    pub async fn validate_huggingface(token: &str) -> ValidationResult {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return ValidationResult {
                is_valid: false,
                message: crate::t!("vault.hf_token_empty"),
            };
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_else(|_| Client::new());

        let resp = client
            .get("https://huggingface.co/api/whoami-v2")
            .header("Authorization", format!("Bearer {}", trimmed))
            .header("User-Agent", concat!("NeuraDex/", env!("CARGO_PKG_VERSION")))
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status();
                if status.is_success() {
                    if let Ok(whoami) = r.json::<HfWhoamiResponse>().await {
                        let user = whoami.name.or(whoami.fullname).unwrap_or_else(|| crate::t!("vault.hf_user_fallback"));
                        ValidationResult {
                            is_valid: true,
                            message: crate::t!("vault.hf_logged_as", user = user),
                        }
                    } else {
                        ValidationResult {
                            is_valid: true,
                            message: crate::t!("vault.hf_token_authenticated"),
                        }
                    }
                } else if status.as_u16() == 401 || status.as_u16() == 403 {
                    ValidationResult {
                        is_valid: false,
                        message: crate::t!("vault.hf_token_invalid"),
                    }
                } else {
                    ValidationResult {
                        is_valid: false,
                        message: crate::t!("vault.hf_server_error", status = status),
                    }
                }
            }
            Err(e) => ValidationResult {
                is_valid: false,
                message: crate::t!("vault.hf_cannot_reach", err = e),
            },
        }
    }

    /// Validates connection to a remote Ollama server
    pub async fn validate_ollama_remote(endpoint: &str, token: &str) -> ValidationResult {
        let clean_endpoint = endpoint.trim().trim_end_matches('/');
        if clean_endpoint.is_empty() {
            return ValidationResult {
                is_valid: false,
                message: crate::t!("vault.remote_url_empty"),
            };
        }

        let url = format!("{}/api/version", clean_endpoint);
        let client = Client::builder()
            .timeout(Duration::from_secs(6))
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut req = client.get(&url);
        if !token.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", token.trim()));
        }

        match req.send().await {
            Ok(r) => {
                let status = r.status();
                if status.is_success() {
                    ValidationResult {
                        is_valid: true,
                        message: crate::t!("vault.remote_accessible", url = clean_endpoint),
                    }
                } else if status.as_u16() == 401 || status.as_u16() == 403 {
                    ValidationResult {
                        is_valid: false,
                        message: crate::t!("vault.remote_denied"),
                    }
                } else {
                    ValidationResult {
                        is_valid: false,
                        message: crate::t!("vault.remote_server_error", status = status),
                    }
                }
            }
            Err(e) => ValidationResult {
                is_valid: false,
                message: crate::t!("vault.remote_cannot_reach", err = e),
            },
        }
    }

    /// Validates an OpenAI-compatible Cloud / Proxy provider
    pub async fn validate_custom_cloud(endpoint: &str, token: &str) -> ValidationResult {
        let trimmed_token = token.trim();
        if trimmed_token.is_empty() {
            return ValidationResult {
                is_valid: false,
                message: crate::t!("vault.key_empty"),
            };
        }

        if trimmed_token.len() < 8 {
            return ValidationResult {
                is_valid: false,
                message: crate::t!("vault.key_too_short"),
            };
        }

        let clean_ep = endpoint.trim().trim_end_matches('/');
        if clean_ep.is_empty() {
            return ValidationResult {
                is_valid: true,
                message: crate::t!("vault.key_registered_no_test"),
            };
        }

        let url = if clean_ep.ends_with("/v1") {
            format!("{}/models", clean_ep)
        } else {
            format!("{}/v1/models", clean_ep)
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_else(|_| Client::new());

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", trimmed_token))
            .header("User-Agent", concat!("NeuraDex/", env!("CARGO_PKG_VERSION")))
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status();
                if status.is_success() {
                    ValidationResult {
                        is_valid: true,
                        message: crate::t!("vault.cloud_authenticated", ep = clean_ep),
                    }
                } else if status.as_u16() == 401 || status.as_u16() == 403 {
                    ValidationResult {
                        is_valid: false,
                        message: crate::t!("vault.cloud_denied"),
                    }
                } else {
                    ValidationResult {
                        is_valid: false,
                        message: crate::t!("vault.cloud_response_http", status = status),
                    }
                }
            }
            Err(e) => ValidationResult {
                is_valid: false,
                message: crate::t!("vault.cloud_cannot_reach", err = e),
            },
        }
    }
}
