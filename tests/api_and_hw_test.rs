use neuradex::api::OllamaClient;
use neuradex::core::hardware::HardwareMonitor;

#[tokio::test]
async fn test_ollama_client_version_and_tags() {
    let client = OllamaClient::default();
    let version = client.get_version().await;
    if let Ok(_ver) = version {
        let tags = client.list_tags().await;
        if let Ok(models) = tags {
            let mut installed_set = std::collections::HashSet::new();
            for m in &models {
                installed_set.insert(m.name.clone());
                if let Some(ref tag) = m.model {
                    installed_set.insert(tag.clone());
                }
                if m.is_cloud() {
                    if let Some(ref rem) = m.remote_model {
                        if !rem.ends_with(":cloud") && !rem.ends_with("-cloud") {
                            installed_set.insert(format!("{}:cloud", rem));
                            installed_set.insert(format!("{}-cloud", rem));
                        } else {
                            installed_set.insert(rem.clone());
                        }
                    }
                } else if let Some(ref rem) = m.remote_model {
                    installed_set.insert(rem.clone());
                }
            }

            if let Some(first) = models.first() {
                let is_first_inst = neuradex::api::types::ModelNameUtils::is_tag_installed(installed_set.iter().map(|s| s.as_str()), &first.name);
                assert!(is_first_inst, "The model present in Ollama must be detected as installed");
            }

            let is_fake_inst = neuradex::api::types::ModelNameUtils::is_tag_installed(installed_set.iter().map(|s| s.as_str()), "non_existent_fake_model_xyz:999b");
            assert!(!is_fake_inst, "A non-existent model must NOT be considered installed");
        }
    }
}



#[tokio::test]
async fn test_ollama_client_show_model() {
    let client = OllamaClient::default();
    let tags = client.list_tags().await;
    if let Ok(models) = tags {
        if let Some(first) = models.first() {
            let show_res = client.show_model(&first.name).await;
            assert!(show_res.is_ok(), "/api/show call must succeed for an existing model");
            let details = show_res.unwrap();
            assert!(details.details.is_some() || details.modelfile.is_some(), "Details or a Modelfile must be returned");
        }
    }
}

#[test]
fn test_hardware_snapshot() {
    let mut hw = HardwareMonitor::new();
    let snap = hw.snapshot();
    println!("GPU: {:?}", snap.gpu);
    println!("CPU: {} cores, RAM: {} MB", snap.cpu.cpu_count, snap.cpu.ram_total / (1024 * 1024));
    assert!(snap.cpu.ram_total > 0);
}

#[tokio::test]
async fn test_ollama_offline_returns_error_gracefully() {
    // Port 59999 is reserved/unlikely to have any listening service
    let client = OllamaClient::new("http://127.0.0.1:59999".to_string());
    let version_res = client.get_version().await;
    assert!(version_res.is_err(), "An unreachable Ollama server must return a network error without panicking");

    let tags_res = client.list_tags().await;
    assert!(tags_res.is_err(), "Fetching offline tags must return an error without panicking");
}

#[test]
fn test_cpu_only_degraded_mode_behavior() {
    use neuradex::core::hardware::estimator::{calculate_memory_estimate, ModelTopology};
    use neuradex::core::hardware::{CpuMetrics, HardwareSnapshot};

    // Simulate an environment with NO GPU at all (pure CPU-only system)
    let cpu_only_snap = HardwareSnapshot {
        gpu: None,
        gpus: vec![],
        cpu: CpuMetrics {
            cpu_name: "Intel/AMD x86_64 CPU".to_string(),
            cpu_count: 8,
            cpu_usage_percent: 5.0,
            ram_total: 32 * 1024 * 1024 * 1024,
            ram_used: 8 * 1024 * 1024 * 1024,
        },
    };

    let topo = ModelTopology {
        total_layers: 32,
        base_model_bytes: 4 * 1024 * 1024 * 1024,
        context_length: 4096,
        kv_heads: 8,
        head_dim: 128,
        sliding_window: None,
    };

    let estimate = calculate_memory_estimate(&topo, 2048, -1, &cpu_only_snap);

    // In degraded CPU-only mode:
    // 1. All layers must be routed to CPU
    assert_eq!(estimate.effective_gpu_layers, 0, "No layers should be allocated to GPU");
    assert_eq!(estimate.effective_cpu_layers, 32, "All 32 layers must be allocated to CPU");
    assert_eq!(estimate.total_vram_bytes, 0, "VRAM footprint must be 0 bytes");
    assert!(estimate.total_ram_bytes > 0, "RAM footprint must be calculated");
    assert!(
        estimate.advice_message.contains("CPU"),
        "The advice message must explicitly mention smooth CPU mode"
    );
}

#[tokio::test]
async fn test_ollama_custom_port_and_host_configuration() {
    use neuradex::core::config::AppConfig;

    // 1. Verify URL normalization with various port formats
    let client_bare_port = OllamaClient::new("127.0.0.1:9999".to_string());
    assert_eq!(client_bare_port.base_url(), "http://127.0.0.1:9999");

    let client_with_proto_and_slash = OllamaClient::new("http://127.0.0.1:9999/".to_string());
    assert_eq!(client_with_proto_and_slash.base_url(), "http://127.0.0.1:9999");

    let client_double_proto = OllamaClient::new("http://http://127.0.0.1:9999".to_string());
    assert_eq!(client_double_proto.base_url(), "http://127.0.0.1:9999");

    let client_https = OllamaClient::new("https://remote.ollama.lan:8443".to_string());
    assert_eq!(client_https.base_url(), "https://remote.ollama.lan:8443");

    // 2. Verify AppConfig custom port storage
    let config = AppConfig {
        ollama_host: "127.0.0.1:9999".to_string(),
        ..Default::default()
    };
    assert_eq!(config.ollama_host, "127.0.0.1:9999");

    // 3. Verify that connecting to an inactive custom port (e.g. 9999) fails cleanly
    let custom_port_client = OllamaClient::new(format!("http://{}", config.ollama_host));
    let version_result = custom_port_client.get_version().await;
    assert!(
        version_result.is_err(),
        "Attempting to connect to an inactive port (9999) must return a clean network error without panicking"
    );
}

#[test]
fn test_cuda_visible_devices_parsing_and_filtering() {
    use neuradex::core::hardware::{parse_cuda_visible_devices, HardwareMonitor, NvidiaBackend};

    // 1. Unit testing the CUDA_VISIBLE_DEVICES parser
    assert_eq!(parse_cuda_visible_devices(None), None, "Missing variable => all GPUs visible");
    assert_eq!(parse_cuda_visible_devices(Some("")), Some(vec![]), "Empty string => 0 visible GPUs (CPU mode)");
    assert_eq!(parse_cuda_visible_devices(Some("   ")), Some(vec![]), "Spaces only => 0 GPUs");
    assert_eq!(parse_cuda_visible_devices(Some("-1")), Some(vec![]), "-1 CUDA standard => 0 GPUs");
    assert_eq!(parse_cuda_visible_devices(Some("none")), Some(vec![]), "none CUDA standard => 0 GPUs");
    assert_eq!(parse_cuda_visible_devices(Some("NoDevFiles")), Some(vec![]), "NoDevFiles => 0 GPUs");
    assert_eq!(parse_cuda_visible_devices(Some("0")), Some(vec![0]), "Single index 0");
    assert_eq!(parse_cuda_visible_devices(Some("0,2,3")), Some(vec![0, 2, 3]), "Multiple indices");
    assert_eq!(parse_cuda_visible_devices(Some(" 0 , 1 ")), Some(vec![0, 1]), "Whitespace handling");

    // 2. Integration test with CUDA_VISIBLE_DEVICES=""
    let prev_cuda = std::env::var("CUDA_VISIBLE_DEVICES").ok();
    std::env::set_var("CUDA_VISIBLE_DEVICES", "");
    let nvidia = NvidiaBackend::new();
    let metrics = nvidia.get_all_metrics();
    assert!(
        metrics.is_empty(),
        "When CUDA_VISIBLE_DEVICES=\"\", NvidiaBackend must return an empty list (0 GPUs)"
    );

    // 3. Integration test with NEURADEX_DISABLE_GPU="1"
    let prev_disable = std::env::var("NEURADEX_DISABLE_GPU").ok();
    std::env::set_var("NEURADEX_DISABLE_GPU", "1");
    let mut hw = HardwareMonitor::new();
    let snap = hw.snapshot();
    assert!(snap.gpu.is_none(), "NEURADEX_DISABLE_GPU=1 must force gpu: None");
    assert!(snap.gpus.is_empty(), "NEURADEX_DISABLE_GPU=1 must force gpus: []");
    assert!(snap.cpu.ram_total > 0, "CPU metrics must remain available");

    // Restore environment
    match prev_cuda {
        Some(v) => std::env::set_var("CUDA_VISIBLE_DEVICES", v),
        None => std::env::remove_var("CUDA_VISIBLE_DEVICES"),
    }
    match prev_disable {
        Some(v) => std::env::set_var("NEURADEX_DISABLE_GPU", v),
        None => std::env::remove_var("NEURADEX_DISABLE_GPU"),
    }
}

