use crate::core::config::{ApiProfile, ApiProviderType};
use tracing::{debug, info, warn};

const SERVICE_NAME: &str = "neuradex";

/// Manages secure storage of API tokens.
///
/// - **Primary**: FreeDesktop Secret Service D-Bus (GNOME Keyring / KDE KWallet) via `keyring` crate
/// - **Fallback**: `ApiProfile.token` field in config.json (chmod 0600) when D-Bus is unavailable
///
/// SSH profiles are excluded from keyring storage since their `token` field
/// contains a filesystem path (e.g. `~/.ssh/id_ed25519`), not a secret.
pub struct SecretStore;

impl SecretStore {
    /// Stores a token in the native keyring for a given profile ID.
    /// Returns Ok(()) on success, Err with message on failure.
    pub fn store_token(profile_id: &str, token: &str) -> Result<(), String> {
        if token.trim().is_empty() {
            return Ok(());
        }

        let entry = keyring::Entry::new(SERVICE_NAME, profile_id)
            .map_err(|e| format!("Keyring entry creation failed: {}", e))?;

        entry
            .set_password(token)
            .map_err(|e| format!("Keyring store failed: {}", e))?;

        debug!("Token stored in keyring for profile '{}'", profile_id);
        Ok(())
    }

    /// Retrieves a token from the native keyring for a given profile ID.
    /// Returns None if not found or keyring is unavailable.
    pub fn get_token(profile_id: &str) -> Option<String> {
        let entry = keyring::Entry::new(SERVICE_NAME, profile_id).ok()?;

        match entry.get_password() {
            Ok(token) if !token.trim().is_empty() => Some(token),
            Ok(_) => None,
            Err(keyring::Error::NoEntry) => None,
            Err(e) => {
                debug!("Keyring read failed for '{}': {}", profile_id, e);
                None
            }
        }
    }

    /// Deletes a token from the native keyring for a given profile ID.
    /// Silently ignores errors (token may not exist or keyring may be unavailable).
    pub fn delete_token(profile_id: &str) {
        if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, profile_id) {
            match entry.delete_credential() {
                Ok(_) => debug!("Token deleted from keyring for profile '{}'", profile_id),
                Err(keyring::Error::NoEntry) => {}
                Err(e) => debug!("Keyring delete failed for '{}': {}", profile_id, e),
            }
        }
    }

    /// Tests whether the native keyring (Secret Service D-Bus) is available.
    /// Performs a lightweight probe by attempting to create an entry.
    pub fn is_keyring_available() -> bool {
        let probe_id = "__neuradex_keyring_probe__";
        match keyring::Entry::new(SERVICE_NAME, probe_id) {
            Ok(entry) => {
                // Try a read — NoEntry is fine, other errors mean keyring is broken
                match entry.get_password() {
                    Ok(_) | Err(keyring::Error::NoEntry) => true,
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }

    /// Resolves the effective token for a profile.
    ///
    /// Priority:
    /// 1. Keyring (if `token_in_keyring` is true)
    /// 2. Profile's `token` field (fallback / legacy)
    ///
    /// SSH profiles always return `profile.token` directly (it's a path, not a secret).
    pub fn resolve_token(profile: &ApiProfile) -> String {
        // SSH profiles store a filesystem path, not a secret
        if profile.provider_type == ApiProviderType::OllamaSsh {
            return profile.token.clone();
        }

        // Try keyring first if token was stored there
        if profile.token_in_keyring {
            if let Some(keyring_token) = Self::get_token(&profile.id) {
                return keyring_token;
            }
            // Keyring flag is set but token not found — fall through to config.json fallback
            warn!(
                "Token for profile '{}' marked as in keyring but not found, using config fallback",
                profile.id
            );
        }

        // Fallback: config.json field
        profile.token.clone()
    }

    /// Migrates existing plaintext tokens from config.json to the native keyring.
    ///
    /// For each non-SSH profile that has a non-empty token in config.json and is not yet
    /// marked as `token_in_keyring`, attempts to store it in the keyring.
    /// On success, sets `token_in_keyring = true` so the token can be cleared from JSON on next save.
    ///
    /// Returns the number of tokens successfully migrated.
    pub fn migrate_from_config(profiles: &mut [ApiProfile]) -> usize {
        if !Self::is_keyring_available() {
            info!("Keyring not available — skipping token migration, using config.json fallback");
            return 0;
        }

        let mut migrated = 0;
        for profile in profiles.iter_mut() {
            // Skip SSH profiles (path, not secret) and already-migrated profiles
            if profile.provider_type == ApiProviderType::OllamaSsh || profile.token_in_keyring {
                continue;
            }

            let token = profile.token.trim();
            if token.is_empty() {
                continue;
            }

            match Self::store_token(&profile.id, token) {
                Ok(_) => {
                    profile.token_in_keyring = true;
                    migrated += 1;
                    info!("Migrated token for profile '{}' to keyring", profile.name);
                }
                Err(e) => {
                    warn!(
                        "Failed to migrate token for profile '{}' to keyring: {}. Keeping in config.json.",
                        profile.name, e
                    );
                }
            }
        }

        migrated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_token_ssh_profile_returns_path_directly() {
        let profile = ApiProfile {
            id: "ssh-alice".to_string(),
            name: "Alice SSH".to_string(),
            provider_type: ApiProviderType::OllamaSsh,
            token: "/home/alice/.ssh/id_ed25519".to_string(),
            endpoint: None,
            is_valid: None,
            last_checked: None,
            token_in_keyring: false,
        };

        assert_eq!(
            SecretStore::resolve_token(&profile),
            "/home/alice/.ssh/id_ed25519"
        );
    }

    #[test]
    fn test_resolve_token_config_fallback() {
        let profile = ApiProfile {
            id: "cloud-test".to_string(),
            name: "Ollama Cloud".to_string(),
            provider_type: ApiProviderType::OllamaCloud,
            token: "sk-secret-fallback-123".to_string(),
            endpoint: None,
            is_valid: None,
            last_checked: None,
            token_in_keyring: false,
        };

        assert_eq!(
            SecretStore::resolve_token(&profile),
            "sk-secret-fallback-123"
        );
    }

    #[test]
    fn test_migrate_skips_ssh_and_empty_profiles() {
        let mut profiles = vec![
            ApiProfile {
                id: "ssh-profile".to_string(),
                name: "SSH Key".to_string(),
                provider_type: ApiProviderType::OllamaSsh,
                token: "/home/user/.ssh/id_ed25519".to_string(),
                endpoint: None,
                is_valid: None,
                last_checked: None,
                token_in_keyring: false,
            },
            ApiProfile {
                id: "empty-cloud".to_string(),
                name: "Empty Cloud".to_string(),
                provider_type: ApiProviderType::OllamaCloud,
                token: "".to_string(),
                endpoint: None,
                is_valid: None,
                last_checked: None,
                token_in_keyring: false,
            },
        ];

        // Neither profile should be migrated
        let count = SecretStore::migrate_from_config(&mut profiles);
        assert_eq!(count, 0);
        assert!(!profiles[0].token_in_keyring);
        assert!(!profiles[1].token_in_keyring);
    }
}
