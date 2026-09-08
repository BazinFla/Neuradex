use neuradex::core::config::{ApiProfile, ApiProviderType, AppConfig};
use neuradex::core::lifecycle::LifecycleManager;
use neuradex::core::vault::ApiVault;

#[test]
fn test_app_config_serialization() {
    let mut config = AppConfig {
        models_directory: Some("/data/ollama-models".to_string()),
        flash_attention: true,
        ollama_host: "0.0.0.0:11434".to_string(),
        ..Default::default()
    };

    let p1 = ApiProfile::new(
        "Ollama Cloud Personal".to_string(),
        ApiProviderType::OllamaCloud,
        "ollama_secret_api_key_xyz".to_string(),
        Some("https://ollama.com".to_string()),
    );
    config.add_or_update_profile(p1);

    assert_eq!(config.profiles.len(), 1);
    assert_eq!(config.active_profile().unwrap().name, "Ollama Cloud Personal");
    assert_eq!(config.active_profile().unwrap().provider_type, ApiProviderType::OllamaCloud);

    let serialized = serde_json::to_string(&config).expect("Serialization failed");
    let deserialized: AppConfig = serde_json::from_str(&serialized).expect("Deserialization failed");

    assert_eq!(deserialized.models_directory, Some("/data/ollama-models".to_string()));
    assert!(deserialized.flash_attention);
    assert_eq!(deserialized.ollama_host, "0.0.0.0:11434");
    assert_eq!(deserialized.profiles.len(), 1);
    assert_eq!(deserialized.profiles[0].name, "Ollama Cloud Personal");
    assert_eq!(deserialized.profiles[0].provider_type, ApiProviderType::OllamaCloud);
}

#[test]
fn test_custom_model_settings_persistence() {
    use neuradex::core::config::CustomModelSettings;

    let mut config = AppConfig::default();
    let settings = CustomModelSettings {
        num_ctx: Some(8192),
        temperature: Some(0.7),
        top_p: Some(0.9),
        top_k: Some(40),
        min_p: Some(0.05),
        seed: Some(42),
        num_predict: Some(2048),
        num_thread: Some(8),
        repeat_penalty: Some(1.1),
        repeat_last_n: Some(128),
        presence_penalty: Some(0.5),
        frequency_penalty: Some(0.5),
        num_gpu: Some(-1),
        use_mlock: Some(true),
        main_gpu: None,
        keep_alive: Some("30m".to_string()),
        system_prompt: Some("You are an expert AI assistant.".to_string()),
    };

    config.set_model_settings("deepseek-r1:1.5b".to_string(), settings.clone());

    assert_eq!(config.get_model_settings("deepseek-r1:1.5b"), Some(&settings));

    let options_map = settings.to_options_map();
    assert_eq!(options_map["num_ctx"], 8192);
    assert_eq!(options_map["min_p"], 0.05);
    assert_eq!(options_map["seed"], 42);
    assert_eq!(options_map["num_predict"], 2048);
    assert_eq!(options_map["num_thread"], 8);
    assert_eq!(options_map["repeat_last_n"], 128);
    assert_eq!(options_map["use_mlock"], true);

    let json = serde_json::to_string(&config).unwrap();
    let reloaded: AppConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(reloaded.get_model_settings("deepseek-r1:1.5b"), Some(&settings));
    assert_eq!(reloaded.get_model_settings("deepseek-r1:1.5b").unwrap().num_ctx, Some(8192));
    assert_eq!(reloaded.get_model_settings("deepseek-r1:1.5b").unwrap().min_p, Some(0.05));
    assert_eq!(reloaded.get_model_settings("non-existent"), None);
}

#[test]
fn test_systemd_override_generation_with_ollama_cloud_key() {
    let mut config = AppConfig {
        models_directory: Some("/mnt/nvme/ollama-models".to_string()),
        ollama_host: "0.0.0.0:11434".to_string(),
        flash_attention: true,
        num_parallel: Some(4),
        ..Default::default()
    };

    let p = ApiProfile::new(
        "Ollama Cloud".to_string(),
        ApiProviderType::OllamaCloud,
        "ollama_key_abc123".to_string(),
        None,
    );
    config.add_or_update_profile(p);

    let override_text = LifecycleManager::generate_systemd_override(&config);

    assert!(override_text.contains("[Service]"));
    assert!(override_text.contains("Environment=\"OLLAMA_MODELS=/mnt/nvme/ollama-models\""));
    assert!(override_text.contains("Environment=\"OLLAMA_HOST=0.0.0.0:11434\""));
    assert!(override_text.contains("Environment=\"OLLAMA_FLASH_ATTENTION=1\""));
    assert!(override_text.contains("Environment=\"OLLAMA_NUM_PARALLEL=4\""));
    assert!(override_text.contains("Environment=\"OLLAMA_API_KEY=ollama_key_abc123\""));
}

#[test]
fn test_exposure_mode_systemd_override() {
    use neuradex::core::config::ExposureMode;

    // 1. LocalHost mode (default)
    let mut config = AppConfig {
        exposure_mode: ExposureMode::LocalHost,
        ..Default::default()
    };
    let override_text = LifecycleManager::generate_systemd_override(&config);
    assert!(!override_text.contains("OLLAMA_ORIGINS"));
    assert!(!override_text.contains("OLLAMA_HOST=0.0.0.0"));

    // 2. LAN mode
    config.exposure_mode = ExposureMode::Lan;
    config.ollama_host = "0.0.0.0:11434".to_string();
    let override_lan = LifecycleManager::generate_systemd_override(&config);
    assert!(override_lan.contains("Environment=\"OLLAMA_HOST=0.0.0.0:11434\""));
    assert!(!override_lan.contains("OLLAMA_ORIGINS"));

    // 3. Exposed mode
    config.exposure_mode = ExposureMode::Exposed;
    config.ollama_origins = Some("*".to_string());
    let override_exposed = LifecycleManager::generate_systemd_override(&config);
    assert!(override_exposed.contains("Environment=\"OLLAMA_HOST=0.0.0.0:11434\""));
    assert!(override_exposed.contains("Environment=\"OLLAMA_ORIGINS=*\""));
}

#[test]
fn test_profile_lifecycle() {
    let mut config = AppConfig::default();
    let p1 = ApiProfile::new(
        "Ollama Cloud".to_string(),
        ApiProviderType::OllamaCloud,
        "ollama_key_111".to_string(),
        None,
    );
    let p2 = ApiProfile::new(
        "Remote Server".to_string(),
        ApiProviderType::OllamaRemote,
        "token_222".to_string(),
        Some("http://192.168.1.50:11434".to_string()),
    );
    let p3 = ApiProfile::new(
        "Ollama SSH Ed25519".to_string(),
        ApiProviderType::OllamaSsh,
        "/home/test/.ollama/id_ed25519".to_string(),
        None,
    );

    let id1 = p1.id.clone();
    let id2 = p2.id.clone();

    config.add_or_update_profile(p1);
    config.add_or_update_profile(p2);
    config.add_or_update_profile(p3);

    assert_eq!(config.active_profile_id, Some(id1.clone()));

    config.set_active_profile(Some(id2.clone()));
    assert_eq!(config.active_profile().unwrap().id, id2);

    config.delete_profile(&id2);
    assert_eq!(config.profiles.len(), 2);
    assert_eq!(config.active_profile().unwrap().id, id1);

    // Verify non-Ollama identity profiles (e.g. HuggingFace) cannot become active profile
    let p_hf = ApiProfile::new(
        "Hugging Face Secret".to_string(),
        ApiProviderType::HuggingFace,
        "hf_secret_token".to_string(),
        None,
    );
    let id_hf = p_hf.id.clone();
    assert!(!p_hf.provider_type.is_ollama_identity());
    config.add_or_update_profile(p_hf);
    assert_eq!(config.active_profile().unwrap().id, id1);

    // Attempting to set HF as active profile is rejected
    config.set_active_profile(Some(id_hf));
    assert_eq!(config.active_profile().unwrap().id, id1);
}

#[tokio::test]
async fn test_key_validation() {
    // 1. Empty key
    let empty_res = ApiVault::validate_ollama_cloud("https://ollama.com", "   ").await;
    assert!(!empty_res.is_valid);

    // 2. Fake Ollama Cloud API key (rejected with 401 by ollama.com)
    let fake_str_res = ApiVault::validate_ollama_cloud("https://ollama.com", "fake_random_key_12345").await;
    assert!(!fake_str_res.is_valid, "A fake API key for ollama.com must be rejected");

    // 3. Fake Hugging Face token (rejected with 401 by HF API)
    let fake_hf = ApiVault::validate_huggingface("hf_fake_token_random_abcdef").await;
    assert!(!fake_hf.is_valid, "Fake Hugging Face token must be rejected");

    // 4. Fake OpenAI Cloud key (rejected with 401 by API)
    let fake_cloud = ApiVault::validate_custom_cloud("https://api.openai.com/v1", "sk-fake_key_1234567890abcdef").await;
    assert!(!fake_cloud.is_valid, "Fake Cloud API key must be rejected");
}

#[test]
fn test_clean_public_key() {
    // 1. Ed25519 key with comment
    let raw_ed = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGX8QzK... flavien@neuradex\n";
    assert_eq!(ApiVault::clean_public_key(raw_ed), "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGX8QzK...");

    // 2. Ed25519 key without comment
    let clean_ed = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGX8QzK...";
    assert_eq!(ApiVault::clean_public_key(clean_ed), "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGX8QzK...");

    // 3. RSA key with multi-word comment
    let raw_rsa = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC... user@my-machine.local domain";
    assert_eq!(ApiVault::clean_public_key(raw_rsa), "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC...");

    // 4. ECDSA key
    let raw_ecdsa = "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTY... user@host";
    assert_eq!(ApiVault::clean_public_key(raw_ecdsa), "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTY...");

    // 5. Whitespace only / single token
    assert_eq!(ApiVault::clean_public_key("   "), "");
    assert_eq!(ApiVault::clean_public_key("single_token"), "single_token");
}

#[test]
fn test_ssh_key_generation_and_discovery() {
    let test_name = format!("test_member_{}", chrono::Utc::now().timestamp_millis());
    let gen_res = ApiVault::generate_ssh_key_pair(&test_name);
    assert!(gen_res.is_ok(), "SSH key generation must succeed");

    let key_info = gen_res.unwrap();
    assert!(key_info.private_key_path.is_file());
    assert!(key_info.public_key_path.is_file());
    assert!(key_info.public_key_content.starts_with("ssh-ed25519 "));

    // Verify that the public key content contains no comment (exactly 2 parts)
    let parts: Vec<&str> = key_info.public_key_content.split_whitespace().collect();
    assert_eq!(parts.len(), 2, "Cleaned public key must only contain key type and base64 blob");
    assert_eq!(parts[0], "ssh-ed25519");

    // Test get_public_key_for_profile
    let profile = ApiProfile::new(
        test_name.clone(),
        ApiProviderType::OllamaSsh,
        key_info.private_key_path.to_string_lossy().to_string(),
        None,
    );
    let pub_key_opt = ApiVault::get_public_key_for_profile(&profile);
    assert!(pub_key_opt.is_some());
    assert_eq!(pub_key_opt.unwrap(), key_info.public_key_content);

    // Validation of generated key
    let val_res = ApiVault::validate_ollama_ssh(&key_info.private_key_path.to_string_lossy());
    assert!(val_res.is_valid, "Generated SSH key must be validated");

    // Cleanup generated test keys
    let _ = std::fs::remove_file(&key_info.private_key_path);
    let _ = std::fs::remove_file(&key_info.public_key_path);
}

#[test]
fn test_original_modelfile_snapshot_and_reset() {
    let mut config = AppConfig::default();
    let model = "qwen2.5-coder:14b".to_string();
    let orig_modelfile = "FROM /blobs/sha256:123456\nTEMPLATE \"\"\"{{ .Prompt }}\"\"\"\n".to_string();
    let orig_template = "{{ .Prompt }}".to_string();

    assert_eq!(config.get_original_modelfile(&model), None);
    assert_eq!(config.get_original_template(&model), None);

    config.set_original_modelfile(model.clone(), orig_modelfile.clone());
    config.set_original_template(model.clone(), orig_template.clone());

    assert_eq!(config.get_original_modelfile(&model), Some(orig_modelfile.as_str()));
    assert_eq!(config.get_original_modelfile("registry.ollama.ai/library/qwen2.5-coder:14b"), Some(orig_modelfile.as_str()));
    assert_eq!(config.get_original_template(&model), Some(orig_template.as_str()));

    // Test settings removal (reset)
    let settings = neuradex::core::config::CustomModelSettings {
        num_ctx: Some(32768),
        temperature: Some(0.2),
        ..Default::default()
    };
    config.set_model_settings(model.clone(), settings);
    assert!(config.get_model_settings(&model).is_some());

    config.remove_model_settings(&model);
    assert!(config.get_model_settings(&model).is_none());
    assert_eq!(config.get_original_modelfile(&model), Some(orig_modelfile.as_str()));
}

#[test]
fn test_detect_models_and_migration_store() {
    let temp_dir = std::env::temp_dir().join(format!("neuradex_test_models_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let manifests_dir = temp_dir.join("manifests/registry.ollama.ai/library/testmodel");
    let blobs_dir = temp_dir.join("blobs");

    let _ = std::fs::create_dir_all(&manifests_dir);
    let _ = std::fs::create_dir_all(&blobs_dir);

    let manifest_file = manifests_dir.join("latest");
    let blob_file = blobs_dir.join("sha256-abc123456789");

    let _ = std::fs::write(&manifest_file, "{\"schemaVersion\": 2}");
    let _ = std::fs::write(&blob_file, vec![0u8; 1024 * 1024]); // 1 MB

    let scan = LifecycleManager::scan_models_dir_info(&temp_dir);
    assert!(scan.is_some(), "Directory with manifests and blobs must be scanned successfully");
    let (count, size) = scan.unwrap();
    assert_eq!(count, 1, "One model (manifest) must be found");
    assert!(size >= 1024 * 1024, "Blobs size must be at least 1 MB");

    let other_temp = std::env::temp_dir().join("neuradex_other_target");
    let detected = LifecycleManager::detect_default_models_store(Some(&temp_dir), Some(&other_temp));
    assert!(detected.is_some(), "detect_default_models_store must detect previous_dir containing models");
    let (src, detected_count, _) = detected.unwrap();
    assert_eq!(src, temp_dir);
    assert_eq!(detected_count, 1);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_parse_rsync_progress_line() {
    use neuradex::core::lifecycle::parse_rsync_progress_line;

    // Standard rsync format --info=progress2
    let line1 = " 1,234,567,890  45%  125.40MB/s  0:01:23 (xfr#1, to-chk=0/10)";
    let parsed1 = parse_rsync_progress_line(line1);
    assert!(parsed1.is_some());
    let (bytes, frac, speed, eta) = parsed1.unwrap();
    assert_eq!(bytes, 1234567890);
    assert!((frac - 0.45).abs() < 0.001);
    assert_eq!(speed, "125.40MB/s");
    assert_eq!(eta, "0:01:23");

    // Transfer start format
    let line2 = " 32,768   0%    0.00kB/s    0:00:00 (xfr#0, to-chk=0/5)";
    let parsed2 = parse_rsync_progress_line(line2);
    assert!(parsed2.is_some());
    let (bytes2, frac2, speed2, eta2) = parsed2.unwrap();
    assert_eq!(bytes2, 32768);
    assert_eq!(frac2, 0.0);
    assert_eq!(speed2, "0.00kB/s");
    assert_eq!(eta2, "0:00:00");

    // 100% format
    let line3 = " 4,567,890,123 100%  340.12MB/s    0:00:00";
    let parsed3 = parse_rsync_progress_line(line3);
    assert!(parsed3.is_some());
    let (bytes3, frac3, speed3, eta3) = parsed3.unwrap();
    assert_eq!(bytes3, 4567890123);
    assert_eq!(frac3, 1.0);
    assert_eq!(speed3, "340.12MB/s");
    assert_eq!(eta3, "0:00:00");

    // Format with localized thousands separators (e.g. French with dots)
    let line_fr = " 52.428.800 100%  110,80MB/s    0:00:00 (xfr#1, to-chk=0/1)";
    let parsed_fr = parse_rsync_progress_line(line_fr);
    assert!(parsed_fr.is_some());
    let (bytes_fr, frac_fr, speed_fr, eta_fr) = parsed_fr.unwrap();
    assert_eq!(bytes_fr, 52428800);
    assert_eq!(frac_fr, 1.0);
    assert_eq!(speed_fr, "110,80MB/s");
    assert_eq!(eta_fr, "0:00:00");

    // Invalid line
    let invalid = "sending incremental file list";
    assert!(parse_rsync_progress_line(invalid).is_none());
}

#[tokio::test]
async fn test_migration_preflight_validation() {
    use std::path::PathBuf;

    // Same source and destination
    let same_path = PathBuf::from("/tmp/same_models_dir");
    let res = LifecycleManager::migrate_models(&same_path, &same_path, None).await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), neuradex::core::i18n::t("settings.mig_err_same_dir"));

    // Non-existent source
    let non_existent = PathBuf::from("/tmp/non_existent_models_dir_xyz_123");
    let dst = PathBuf::from("/tmp/dst_models_dir");
    let res2 = LifecycleManager::migrate_models(&non_existent, &dst, None).await;
    assert!(res2.is_err());
    assert_eq!(
        res2.unwrap_err(),
        neuradex::core::i18n::t_args(
            "settings.mig_err_source_not_found",
            &[("src", &non_existent.display().to_string())]
        )
    );
}

#[test]
fn test_config_keyring_token_sanitization() {
    let mut config = AppConfig::default();
    let mut p = ApiProfile::new(
        "Keyring Profile".to_string(),
        ApiProviderType::OllamaCloud,
        "super-secret-key-123".to_string(),
        None,
    );
    p.token_in_keyring = true;
    config.add_or_update_profile(p);

    let mut sanitized = config.clone();
    for prof in &mut sanitized.profiles {
        if prof.token_in_keyring {
            prof.token.clear();
        }
    }
    let json = serde_json::to_string(&sanitized).expect("Serialization failed");
    assert!(!json.contains("super-secret-key-123"));
    assert!(json.contains("\"token_in_keyring\":true"));

    let deserialized: AppConfig = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(deserialized.profiles[0].token, "");
    assert!(deserialized.profiles[0].token_in_keyring);
}

#[tokio::test]
async fn test_generate_diagnostic_report() {
    let config = AppConfig {
        language: "fr".to_string(),
        ollama_host: "http://127.0.0.1:11434".to_string(),
        ..Default::default()
    };

    let report = neuradex::core::generate_diagnostic_report(&config).await;

    assert!(report.contains("NeuraDex System & Environment Diagnostic Report"));
    assert!(report.contains("NeuraDex Version"));
    assert!(report.contains("Operating System"));
    assert!(report.contains("Hardware & Memory"));
    assert!(report.contains("CPU"));
    assert!(report.contains("System RAM"));
    assert!(report.contains("Ollama Service & Storage"));
}

