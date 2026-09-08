use neuradex::api::OllamaWebClient;
use neuradex::core::hardware::{GpuMetrics, HardwareSnapshot};
use neuradex::core::hub::{HardwareFitness, HubManager};


#[test]
fn test_hardware_fitness_evaluation() {
    let mut snap = HardwareSnapshot {
        gpu: Some(GpuMetrics {
            index: 0,
            name: "NVIDIA RTX 4070 Ti".to_string(),
            vram_total: 12_000_000_000,
            vram_used: 2_000_000_000,
            vram_free: 10_000_000_000, // 10 GB free
            temperature_c: Some(40),
            utilization_percent: Some(0),
            power_watts: Some(20.0),
        }),
        gpus: vec![GpuMetrics {
            index: 0,
            name: "NVIDIA RTX 4070 Ti".to_string(),
            vram_total: 12_000_000_000,
            vram_used: 2_000_000_000,
            vram_free: 10_000_000_000,
            temperature_c: Some(40),
            utilization_percent: Some(0),
            power_watts: Some(20.0),
        }],
        cpu: neuradex::core::hardware::CpuMetrics {
            cpu_name: "AMD Ryzen 9".to_string(),
            cpu_count: 16,
            cpu_usage_percent: 5.0,
            ram_total: 32_000_000_000,
            ram_used: 8_000_000_000, // 24 GB free
        },
    };

    // 1. 4 GB Model -> Fits 100% into 10 GB free VRAM
    let fit1 = HubManager::evaluate_fitness(4_000_000_000, &snap);
    assert_eq!(fit1, HardwareFitness::FullGpu);

    // 2. 16 GB Model -> Overflows VRAM (10 GB) but fits into VRAM + RAM (34 GB available)
    let fit2 = HubManager::evaluate_fitness(16_000_000_000, &snap);
    assert_eq!(fit2, HardwareFitness::GpuOffload);

    // 3. 50 GB Model -> Exceeds total available memory (34 GB)
    let fit3 = HubManager::evaluate_fitness(50_000_000_000, &snap);
    assert_eq!(fit3, HardwareFitness::Insufficient);

    // 4. Without dedicated GPU (CPU/RAM only)
    snap.gpu = None;
    let fit4 = HubManager::evaluate_fitness(4_000_000_000, &snap);
    assert_eq!(fit4, HardwareFitness::GpuOffload);
}

#[test]
fn test_hub_catalog_structure() {
    // Test cache saving and reloading
    let mock_models = vec![
        neuradex::core::hub::HubModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            category: "General".to_string(),
            description: "Test model".to_string(),
            default_tag: "test-model:latest".to_string(),
            variants: vec![
                neuradex::core::hub::HubModelVariant::new(
                    "test-model:latest",
                    4_000_000_000,
                    "7B",
                    "Q4_K_M",
                    32_768,
                ),
            ],
            icon_name: "computer-symbolic".to_string(),
            is_cloud: false,
            pulls_count: 100,
            updated_at: Some("2 days ago".to_string()),
            updated_date_key: 202608230000,
            page_url: "https://ollama.com/library/test-model".to_string(),
            ..Default::default()
        },
    ];

    let cache_path = HubManager::cache_path();
    let original_cache = std::fs::read_to_string(&cache_path).ok();

    HubManager::save_cache(&mock_models);
    let loaded = HubManager::load_catalog();
    assert!(!loaded.is_empty(), "The catalog reloaded from cache must not be empty");
    assert_eq!(loaded[0].id, "test-model");

    // Mandatory cleanup to avoid polluting user environment
    let _ = std::fs::remove_file(&cache_path);
    if let Some(orig) = original_cache {
        let _ = std::fs::write(&cache_path, orig);
    }
}

#[test]
fn test_cloud_model_detection() {
    use neuradex::core::hub::{HubManager, HubModelVariant};

    // 1. Variant with size 0 or tag "cloud"
    let variants_cloud = vec![HubModelVariant::new("gemini:cloud", 0, "Cloud", "API", 128_000)];
    assert!(HubManager::is_cloud_model("gemini", "Gemini Pro", "API Model", "General", &variants_cloud));

    // 2. ID ending with :cloud or -cloud
    let variants_local = vec![HubModelVariant::new("qwen:7b", 4_000_000_000, "7B", "Q4", 32_768)];
    assert!(HubManager::is_cloud_model("gpt-oss:cloud", "GPT OSS", "Cloud model", "General", &variants_local));
    assert!(HubManager::is_cloud_model("nemotron-cloud", "Nemotron", "Model", "General", &variants_local));

    // 3. Name or category containing cloud/distant/remote
    assert!(HubManager::is_cloud_model("kimi", "Kimi Cloud 2.5", "Remote model", "General", &variants_local));
    assert!(HubManager::is_cloud_model("mistral-remote", "Mistral", "Description", "Remote API", &variants_local));

    // 4. Standard local model should NOT be cloud
    assert!(!HubManager::is_cloud_model("llama3.2", "Llama 3.2", "Local LLM", "General", &variants_local));
}

#[test]
fn test_hub_sorting_criteria() {
    use neuradex::core::hub::{HubModelInfo, HubModelVariant, SortCriterion};

    let mut models = vec![
        HubModelInfo {
            id: "qwen".to_string(),
            name: "Qwen 2.5".to_string(),
            category: "General".to_string(),
            description: "Qwen model".to_string(),
            default_tag: "qwen:7b".to_string(),
            icon_name: "computer-symbolic".to_string(),
            is_cloud: false,
            pulls_count: 5_000_000,
            updated_at: Some("1 month ago".to_string()),
            updated_date_key: 202607250000,
            page_url: "https://ollama.com/library/qwen".to_string(),
            variants: vec![HubModelVariant::new("qwen:7b", 4_700_000_000, "7B", "Q4_K_M", 32_768)],
            ..Default::default()
        },
        HubModelInfo {
            id: "deepseek".to_string(),
            name: "DeepSeek R1".to_string(),
            category: "Reasoning".to_string(),
            description: "DeepSeek model".to_string(),
            default_tag: "deepseek:8b".to_string(),
            icon_name: "weather-clear-symbolic".to_string(),
            is_cloud: false,
            pulls_count: 20_000_000,
            updated_at: Some("6 hours ago".to_string()),
            updated_date_key: 202608251400,
            page_url: "https://ollama.com/library/deepseek".to_string(),
            variants: vec![HubModelVariant::new("deepseek:8b", 4_900_000_000, "8B", "Q4_K_M", 32_768)],
            ..Default::default()
        },
        HubModelInfo {
            id: "llama".to_string(),
            name: "Llama 3.2 (1B)".to_string(),
            category: "Lightweight".to_string(),
            description: "Lightweight model".to_string(),
            default_tag: "llama:1b".to_string(),
            icon_name: "computer-symbolic".to_string(),
            is_cloud: false,
            pulls_count: 15_000_000,
            updated_at: Some("1 year ago".to_string()),
            updated_date_key: 202508250000,
            page_url: "https://ollama.com/library/llama".to_string(),
            variants: vec![HubModelVariant::new("llama:1b", 1_300_000_000, "1B", "Q4_K_M", 32_768)],
            ..Default::default()
        },
    ];

    // 1. Sort by Popularity (Descending pulls) -> DeepSeek (20M), Llama (15M), Qwen (5M)
    HubManager::sort_models(&mut models, SortCriterion::Popularity);
    assert_eq!(models[0].id, "deepseek");
    assert_eq!(models[1].id, "llama");
    assert_eq!(models[2].id, "qwen");

    // 2. Sort by Date / Recent (Newest) -> DeepSeek (2026-08-25), Qwen (2026-07-25), Llama (2025-08-25)
    HubManager::sort_models(&mut models, SortCriterion::Newest);
    assert_eq!(models[0].id, "deepseek");
    assert_eq!(models[1].id, "qwen");
    assert_eq!(models[2].id, "llama");

    // 3. Sort by Name A-Z -> DeepSeek, Llama, Qwen
    HubManager::sort_models(&mut models, SortCriterion::NameAsc);
    assert_eq!(models[0].name, "DeepSeek R1");
    assert_eq!(models[1].name, "Llama 3.2 (1B)");
    assert_eq!(models[2].name, "Qwen 2.5");

    // 4. Sort by Name Z-A -> Qwen, Llama, DeepSeek
    HubManager::sort_models(&mut models, SortCriterion::NameDesc);
    assert_eq!(models[0].name, "Qwen 2.5");
    assert_eq!(models[1].name, "Llama 3.2 (1B)");
    assert_eq!(models[2].name, "DeepSeek R1");

    // 5. Sort by Increasing Size -> Llama (1.3 GB), Qwen (4.7 GB), DeepSeek (4.9 GB)
    HubManager::sort_models(&mut models, SortCriterion::SizeAsc);
    assert_eq!(models[0].id, "llama");
    assert_eq!(models[1].id, "qwen");
    assert_eq!(models[2].id, "deepseek");

    // 6. Sort by Decreasing Size -> DeepSeek (4.9 GB), Qwen (4.7 GB), Llama (1.3 GB)
    HubManager::sort_models(&mut models, SortCriterion::SizeDesc);
    assert_eq!(models[0].id, "deepseek");
    assert_eq!(models[1].id, "qwen");
    assert_eq!(models[2].id, "llama");
}

#[tokio::test]
async fn test_online_library_fetch() {
    let client = OllamaWebClient::new();
    let res = client.fetch_library_catalog().await;
    match res {
        Ok(models) => {
            assert!(models.len() > 50, "ollama.com/library must return more than 50 models");
            println!("✅ {} models dynamically fetched from ollama.com", models.len());
        }
        Err(e) => {
            println!("⚠️ Network test skipped or offline: {}", e);
        }
    }
}

#[test]
fn test_parse_modelfile_components() {
    use neuradex::api::client::OllamaClient;

    let modelfile = r#"FROM qwen2.5-coder:14b
TEMPLATE """{{ if .System }}<|im_start|>system
{{ .System }}<|im_end|>
{{ end }}"""
PARAMETER num_ctx 75776
PARAMETER temperature 0.7
PARAMETER top_p 0.9
PARAMETER top_k 40
PARAMETER use_mlock true
PARAMETER presence_penalty 0.5
SYSTEM """You are a coding expert."""
"#;

    let (from, template, system, params) =
        OllamaClient::parse_modelfile_components(modelfile, "default-model");

    assert_eq!(from, "qwen2.5-coder:14b");
    assert!(template.unwrap().contains("{{ .System }}"));
    assert_eq!(system.unwrap(), "You are a coding expert.");

    let p = params.expect("Parameters should be parsed");
    assert_eq!(p["num_ctx"], 75776);
    assert_eq!(p["temperature"], 0.7);
    assert_eq!(p["top_p"], 0.9);
    assert_eq!(p["top_k"], 40);
    assert_eq!(p["use_mlock"], true);
    assert_eq!(p["presence_penalty"], 0.5);
}

#[tokio::test]
async fn test_hf_repo_details_fetch() {
    let client = OllamaWebClient::new();
    let res = client.fetch_hf_repo_details("OBLITERATUS/Qwen3.8-27B-OBLITERATED", None).await;
    match res {
        Ok(details) => {
            assert_eq!(details.model_name, "Qwen3.8-27B-OBLITERATED");
            assert!(!details.files.is_empty(), "GGUF files must be detected");
            println!("✅ {} GGUF versions detected for {}", details.files.len(), details.model_name);
            for f in details.files.iter().take(3) {
                println!("  - {} ({}): {}", f.quantization, f.size_formatted, f.tag);
            }
            assert!(details.files.iter().any(|f| f.quantization.contains("Q4") || f.quantization.contains("IQ4")));
            assert!(details.files.iter().all(|f| !f.files.is_empty()), "All GGUF files must list their fragments");
        }
        Err(e) => {
            println!("⚠️ HF network test skipped or unreachable: {}", e);
        }
    }
}


