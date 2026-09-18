use neuradex::api::types::{ChatMessage, ChatMetrics, ChatRequest, ChatStreamChunk, ModelPs, ModelTag};

#[test]
fn test_chat_message_creation_and_serialization() {
    let user_msg = ChatMessage::user("Hello!".to_string());
    assert_eq!(user_msg.role, "user");
    assert_eq!(user_msg.content, "Hello!");

    let asst_msg = ChatMessage::assistant("I am here to help you.".to_string());
    assert_eq!(asst_msg.role, "assistant");

    let sys_msg = ChatMessage::system("You are a Rust expert.".to_string());
    assert_eq!(sys_msg.role, "system");

    let json = serde_json::to_string(&user_msg).expect("serialization ok");
    assert!(json.contains("\"role\":\"user\""));
    assert!(json.contains("\"content\":\"Hello!\""));

    let deserialized: ChatMessage = serde_json::from_str(&json).expect("deserialization ok");
    assert_eq!(deserialized.role, "user");
    assert_eq!(deserialized.content, "Hello!");
}

#[test]
fn test_chat_request_payload() {
    let req = ChatRequest {
        model: "llama3:8b".to_string(),
        messages: vec![
            ChatMessage::system("System instructions".to_string()),
            ChatMessage::user("User question".to_string()),
        ],
        stream: true,
        options: Some(serde_json::json!({
            "temperature": 0.7,
            "num_ctx": 4096
        })),
        keep_alive: Some(serde_json::json!("10m")),
    };

    let json = serde_json::to_string(&req).expect("serialize request");
    assert!(json.contains("\"model\":\"llama3:8b\""));
    assert!(json.contains("\"stream\":true"));
    assert!(json.contains("\"temperature\":0.7"));
    assert!(json.contains("\"num_ctx\":4096"));
    assert!(json.contains("\"keep_alive\":\"10m\""));
}

#[test]
fn test_chat_stream_chunk_parsing() {
    let raw_chunk = r#"{
        "model": "llama3:8b",
        "created_at": "2026-08-26T00:00:00Z",
        "message": {
            "role": "assistant",
            "content": "Hello world"
        },
        "done": false
    }"#;

    let chunk: ChatStreamChunk = serde_json::from_str(raw_chunk).expect("parse chunk");
    assert_eq!(chunk.model, Some("llama3:8b".to_string()));
    assert!(!chunk.done);
    let msg = chunk.message.expect("has message");
    assert_eq!(msg.content, "Hello world");
    assert_eq!(msg.role, "assistant");

    let final_chunk = r#"{
        "model": "llama3:8b",
        "created_at": "2026-08-26T00:00:01Z",
        "message": {
            "role": "assistant",
            "content": ""
        },
        "done": true,
        "total_duration": 1500000000,
        "load_duration": 100000000,
        "prompt_eval_count": 15,
        "prompt_eval_duration": 250000000,
        "eval_count": 120,
        "eval_duration": 1150000000
    }"#;

    let end_chunk: ChatStreamChunk = serde_json::from_str(final_chunk).expect("parse end chunk");
    assert!(end_chunk.done);
    assert_eq!(end_chunk.eval_count, Some(120));
    assert_eq!(end_chunk.prompt_eval_count, Some(15));
    assert_eq!(end_chunk.prompt_eval_duration, Some(250000000));
    assert_eq!(end_chunk.eval_duration, Some(1150000000));

    // Metrics calculation
    let eval_dur_secs = end_chunk.eval_duration.unwrap() as f64 / 1_000_000_000.0;
    let tok_s = end_chunk.eval_count.unwrap() as f64 / eval_dur_secs;
    let ttft_ms = end_chunk.prompt_eval_duration.unwrap() as f64 / 1_000_000.0;

    assert!((tok_s - 104.3478).abs() < 0.01);
    assert_eq!(ttft_ms, 250.0);

    let metrics = ChatMetrics {
        total_duration_ms: end_chunk.total_duration.unwrap() as f64 / 1_000_000.0,
        ttft_ms,
        eval_count: end_chunk.eval_count.unwrap(),
        prompt_eval_count: end_chunk.prompt_eval_count.unwrap(),
        tokens_per_sec: tok_s,
        vram_bytes: 4_294_967_296,
    };

    assert_eq!(metrics.ttft_ms, 250.0);
    assert_eq!(metrics.eval_count, 120);
    assert_eq!(metrics.vram_bytes, 4_294_967_296);
}

#[test]
fn test_model_sorting_preloaded_first() {
    let installed = vec![
        ModelTag {
            name: "mistral:latest".to_string(),
            model: None,
            remote_model: None,
            remote_host: None,
            size: 4_000_000_000,
            modified_at: Some("2026-08-20T00:00:00Z".to_string()),
            digest: Some("abc".to_string()),
            details: None,
        },
        ModelTag {
            name: "qwen2.5-coder:7b".to_string(),
            model: None,
            remote_model: None,
            remote_host: None,
            size: 5_000_000_000,
            modified_at: Some("2026-08-22T00:00:00Z".to_string()),
            digest: Some("def".to_string()),
            details: None,
        },
        ModelTag {
            name: "llama3.2:3b".to_string(),
            model: None,
            remote_model: None,
            remote_host: None,
            size: 2_000_000_000,
            modified_at: Some("2026-08-25T00:00:00Z".to_string()),
            digest: Some("ghi".to_string()),
            details: None,
        },
    ];

    let running = [
        ModelPs {
            name: "qwen2.5-coder:7b".to_string(),
            model: Some("qwen2.5-coder:7b".to_string()),
            size: 5_000_000_000,
            size_vram: 5_000_000_000,
            digest: Some("def".to_string()),
            details: None,
            expires_at: None,
            context_length: None,
        },
    ];

    // Identical algorithm to ChatView::update_models
    let running_names: std::collections::HashSet<String> = running
        .iter()
        .flat_map(|r| {
            let mut v = vec![r.name.clone()];
            if let Some(ref m) = r.model {
                v.push(m.clone());
            }
            v
        })
        .collect();

    let mut preloaded = Vec::new();
    let mut other_models = Vec::new();

    for m in &installed {
        if running_names.contains(&m.name) {
            preloaded.push(m);
        } else {
            other_models.push(m);
        }
    }

    let snap = neuradex::core::hardware::HardwareSnapshot {
        gpu: Some(neuradex::core::hardware::GpuMetrics {
            index: 0,
            name: "NVIDIA RTX 4080".to_string(),
            vram_total: 16_000_000_000,
            vram_used: 5_000_000_000,
            vram_free: 11_000_000_000,
            temperature_c: Some(50),
            utilization_percent: Some(30),
            power_watts: Some(100.0),
        }),
        gpus: vec![neuradex::core::hardware::GpuMetrics {
            index: 0,
            name: "NVIDIA RTX 4080".to_string(),
            vram_total: 16_000_000_000,
            vram_used: 5_000_000_000,
            vram_free: 11_000_000_000,
            temperature_c: Some(50),
            utilization_percent: Some(30),
            power_watts: Some(100.0),
        }],
        cpu: neuradex::core::hardware::CpuMetrics {
            cpu_name: "AMD Ryzen 9".to_string(),
            cpu_count: 16,
            cpu_usage_percent: 10.0,
            ram_total: 32_000_000_000,
            ram_used: 8_000_000_000,
        },
    };

    let mut ordered_labels = Vec::new();
    for m in preloaded {
        let fitness = neuradex::core::hub::HubManager::evaluate_fitness(m.size, &snap);
        let (circle, color) = match fitness {
            neuradex::core::hub::HardwareFitness::FullGpu => ("●", "#2ec27e"),
            neuradex::core::hub::HardwareFitness::GpuOffload => ("●", "#ff7800"),
            neuradex::core::hub::HardwareFitness::Insufficient => ("●", "#e01b24"),
        };
        ordered_labels.push((circle, color, m.name.clone()));
    }
    for m in other_models {
        let fitness = neuradex::core::hub::HubManager::evaluate_fitness(m.size, &snap);
        let (circle, color) = match fitness {
            neuradex::core::hub::HardwareFitness::FullGpu => ("○", "#2ec27e"),
            neuradex::core::hub::HardwareFitness::GpuOffload => ("○", "#ff7800"),
            neuradex::core::hub::HardwareFitness::Insufficient => ("○", "#e01b24"),
        };
        ordered_labels.push((circle, color, m.name.clone()));
    }

    assert_eq!(ordered_labels.len(), 3);
    // The first model must be preloaded (filled circle ●, green #2ec27e)
    assert_eq!(ordered_labels[0], ("●", "#2ec27e", "qwen2.5-coder:7b".to_string()));
    // Subsequent models on disk (empty circle ○, green #2ec27e)
    assert_eq!(ordered_labels[1], ("○", "#2ec27e", "mistral:latest".to_string()));
    assert_eq!(ordered_labels[2], ("○", "#2ec27e", "llama3.2:3b".to_string()));
}

#[test]
fn test_model_capability_icons_detection() {
    use neuradex::core::capabilities::{detect_capabilities, format_capability_icons, ModelCapability};

    let r1 = detect_capabilities("deepseek-r1:8b", None, None, None);
    assert_eq!(r1, vec![ModelCapability::Reasoning]);
    assert_eq!(format_capability_icons(&r1), " 🧠");

    let qwq = detect_capabilities("qwq:32b", None, None, None);
    assert_eq!(qwq, vec![ModelCapability::Reasoning]);
    assert_eq!(format_capability_icons(&qwq), " 🧠");

    let vision = detect_capabilities("llama3.2-vision:11b", None, None, None);
    assert_eq!(vision, vec![ModelCapability::Vision]);
    assert_eq!(format_capability_icons(&vision), " 👁️");

    let llava = detect_capabilities("llava:7b", None, None, None);
    assert_eq!(llava, vec![ModelCapability::Vision]);
    assert_eq!(format_capability_icons(&llava), " 👁️");

    let coder = detect_capabilities("qwen2.5-coder:7b", None, None, None);
    assert_eq!(coder, vec![ModelCapability::Code]);
    assert_eq!(format_capability_icons(&coder), " 💻");

    let embed = detect_capabilities("nomic-embed-text:latest", None, None, None);
    assert_eq!(embed, vec![ModelCapability::Embedding]);
    assert_eq!(format_capability_icons(&embed), " 🧬");

    let mistral = detect_capabilities("mistral:latest", None, None, None);
    assert_eq!(mistral, vec![ModelCapability::Tools]);
    assert_eq!(format_capability_icons(&mistral), " 🔧");
}

#[test]
fn test_chat_session_serialization_and_lifecycle() {
    let mut session = neuradex::ui::views::chat_view::ChatSession::new_empty(Some("llama3.2:3b".to_string()));
    assert!(neuradex::core::chat_session::ChatSession::is_default_title(&session.title));
    assert_eq!(session.model.as_deref(), Some("llama3.2:3b"));
    assert_eq!(session.messages.len(), 0);

    // Add a user message and an assistant message
    session.messages.push(ChatMessage::user("Hello NeuraDex!".to_string()));
    session.title = "Hello NeuraDex!".to_string();
    session.messages.push(ChatMessage::assistant("Hello! How can I help you?".to_string()));

    // JSON serialization
    let json = serde_json::to_string(&session).expect("serialize session");
    assert!(json.contains("Hello NeuraDex!"));
    assert!(json.contains("llama3.2:3b"));

    // Deserialization
    let restored: neuradex::ui::views::chat_view::ChatSession = serde_json::from_str(&json).expect("deserialize session");
    assert_eq!(restored.id, session.id);
    assert_eq!(restored.title, "Hello NeuraDex!");
    assert_eq!(restored.messages.len(), 2);
    assert_eq!(restored.messages[0].role, "user");
    assert_eq!(restored.messages[1].role, "assistant");
}

#[test]
fn test_default_context_length_prefilling() {
    use neuradex::ui::components::chat_settings_popover::ChatSettingsPopover;
    assert_eq!(ChatSettingsPopover::default_context_for_model("deepseek-r1:8b"), 32768);
    assert_eq!(ChatSettingsPopover::default_context_for_model("qwen2.5-coder:7b"), 32768);
    assert_eq!(ChatSettingsPopover::default_context_for_model("llama3.2:3b"), 16384);
    assert_eq!(ChatSettingsPopover::default_context_for_model("mistral:7b"), 16384);
    assert_eq!(ChatSettingsPopover::default_context_for_model("gemma2:9b"), 8192);
    assert_eq!(ChatSettingsPopover::default_context_for_model("tinyllama:latest"), 4096);
}

#[test]
fn test_check_model_running_detection() {
    use neuradex::api::types::ModelNameUtils;
    let running = vec![
        neuradex::api::types::ModelPs {
            name: "registry.ollama.ai/library/llama3.2:latest".to_string(),
            model: Some("llama3.2:latest".to_string()),
            size: 2_000_000_000,
            digest: Some("sha256:123".to_string()),
            details: None,
            expires_at: Some("2026-08-26T12:00:00Z".to_string()),
            size_vram: 2_000_000_000,
            context_length: None,
        },
        neuradex::api::types::ModelPs {
            name: "qwen2.5-coder:7b".to_string(),
            model: None,
            size: 4_700_000_000,
            digest: Some("sha256:456".to_string()),
            details: None,
            expires_at: Some("2026-08-26T12:00:00Z".to_string()),
            size_vram: 4_700_000_000,
            context_length: None,
        },
    ];

    // Case 1: "llama3.2" (without :latest, without prefix)
    let (is_running, vram) = ModelNameUtils::check_model_running("llama3.2", 2_000_000_000, &running);
    assert!(is_running);
    assert_eq!(vram, 2_000_000_000);

    // Case 2: "llama3.2:latest"
    let (is_running, vram) = ModelNameUtils::check_model_running("llama3.2:latest", 2_000_000_000, &running);
    assert!(is_running);
    assert_eq!(vram, 2_000_000_000);

    // Case 3: "qwen2.5-coder:7b"
    let (is_running, vram) = ModelNameUtils::check_model_running("qwen2.5-coder:7b", 4_700_000_000, &running);
    assert!(is_running);
    assert_eq!(vram, 4_700_000_000);

    // Case 4: "mistral:latest" (not loaded)
    let (is_running, _) = ModelNameUtils::check_model_running("mistral:latest", 4_000_000_000, &running);
    assert!(!is_running);
}

#[test]
fn test_model_custom_settings_lookup_and_prefilling() {
    let mut config = neuradex::core::config::AppConfig::default();
    config.set_model_settings(
        "qwen2.5-coder:7b".to_string(),
        neuradex::core::config::CustomModelSettings {
            num_ctx: Some(65536),
            temperature: Some(0.25),
            num_gpu: Some(99),
            system_prompt: Some("You are an expert Rust programmer.".to_string()),
            ..Default::default()
        },
    );

    // Verify lookup with and without :latest
    let found = config.get_model_settings("qwen2.5-coder:7b").expect("exact match");
    assert_eq!(found.num_ctx, Some(65536));
    assert_eq!(found.temperature, Some(0.25));

    let found_latest = config.get_model_settings("qwen2.5-coder:7b:latest").expect("latest match");
    assert_eq!(found_latest.num_ctx, Some(65536));

    let found_library = config.get_model_settings("library/qwen2.5-coder:7b").expect("library match");
    assert_eq!(found_library.num_ctx, Some(65536));
}

#[test]
fn test_detect_max_context_for_model() {
    use neuradex::ui::components::chat_settings_popover::ChatSettingsPopover;
    assert_eq!(ChatSettingsPopover::detect_max_context_for_model("llama3.2:3b"), 131072);
    assert_eq!(ChatSettingsPopover::detect_max_context_for_model("qwen2.5-coder:7b"), 131072);
    assert_eq!(ChatSettingsPopover::detect_max_context_for_model("deepseek-r1:8b"), 131072);
    assert_eq!(ChatSettingsPopover::detect_max_context_for_model("mistral:7b"), 32768);
    assert_eq!(ChatSettingsPopover::detect_max_context_for_model("gemma2:9b"), 8192);
    assert_eq!(ChatSettingsPopover::detect_max_context_for_model("tinyllama:latest"), 4096);
}

#[test]
fn test_model_show_response_extractors() {
    let mut model_info = std::collections::HashMap::new();
    model_info.insert("llama.context_length".to_string(), serde_json::json!(131072));
    model_info.insert("llama.block_count".to_string(), serde_json::json!(28));
    model_info.insert("llama.attention.head_count_kv".to_string(), serde_json::json!(8));
    model_info.insert("llama.attention.head_count".to_string(), serde_json::json!(24));
    model_info.insert("llama.embedding_length".to_string(), serde_json::json!(3072));

    let show = neuradex::api::types::ModelShowResponse {
        license: None,
        modelfile: None,
        parameters: Some("num_ctx 131072\ntemperature 0.7".to_string()),
        template: None,
        system: None,
        details: None,
        model_info: Some(model_info),
        modified_at: None,
    };

    assert_eq!(show.extract_context_length(), Some(131072));
    assert_eq!(show.extract_layer_count(), Some(28));
    assert_eq!(show.extract_kv_heads(), Some(8));
    assert_eq!(show.extract_head_count(), Some(24));
    assert_eq!(show.extract_embedding_length(), Some(3072));
}

#[test]
fn test_cloud_model_icon_selector() {
    let cloud_tag = ModelTag {
        name: "gpt-oss:cloud".to_string(),
        model: None,
        remote_model: Some("gpt-oss".to_string()),
        remote_host: Some("ollama.com".to_string()),
        size: 0,
        modified_at: None,
        digest: None,
        details: None,
    };
    assert!(cloud_tag.is_cloud());

    let local_tag = ModelTag {
        name: "qwen2.5:7b".to_string(),
        model: None,
        remote_model: None,
        remote_host: None,
        size: 4_500_000_000,
        modified_at: None,
        digest: None,
        details: None,
    };
    assert!(!local_tag.is_cloud());
}

