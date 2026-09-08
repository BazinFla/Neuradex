use crate::api::OllamaClient;
use crate::core::config::AppConfig;
use crate::core::hardware::estimator::format_gib;
use crate::core::hardware::HardwareMonitor;
use sysinfo::System;

/// Generates a comprehensive markdown diagnostic report detailing the application version,
/// host operating system, CPU, RAM, GPU(s), Ollama daemon state, and installed models.
pub async fn generate_diagnostic_report(config: &AppConfig) -> String {
    let mut report = String::new();

    report.push_str("### 🩺 NeuraDex System & Environment Diagnostic Report\n\n");

    // 1. Application Details
    report.push_str("#### 📱 Application\n");
    report.push_str(&format!("- **NeuraDex Version** : `v{}`\n", env!("CARGO_PKG_VERSION")));
    report.push_str(&format!("- **Configured Language** : `{}`\n", config.language));
    report.push_str(&format!("- **Daemon Management Mode** : `{:?}`\n\n", config.daemon_mode));

    // 2. Host OS & Desktop
    report.push_str("#### 🐧 Operating System & Desktop\n");
    let mut sys = System::new_all();
    sys.refresh_all();

    let os_name = System::name().unwrap_or_else(|| "Linux".to_string());
    let os_ver = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    let kernel = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
    let arch = System::cpu_arch();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "Unknown".to_string());
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "Unknown".to_string());

    report.push_str(&format!("- **Distribution** : {} {}\n", os_name, os_ver));
    report.push_str(&format!("- **Kernel** : {}\n", kernel));
    report.push_str(&format!("- **Architecture** : {}\n", arch));
    report.push_str(&format!("- **Desktop Session** : {} ({})\n\n", desktop, session_type));

    // 3. Hardware (CPU & RAM)
    report.push_str("#### 💻 Hardware & Memory\n");
    let mut hw_monitor = HardwareMonitor::new();
    let snapshot = hw_monitor.snapshot();

    let cpu_brand = if !snapshot.cpu.cpu_name.is_empty() {
        snapshot.cpu.cpu_name.clone()
    } else {
        "Generic CPU".to_string()
    };
    report.push_str(&format!(
        "- **CPU** : {} ({} threads)\n",
        cpu_brand, snapshot.cpu.cpu_count
    ));
    report.push_str(&format!(
        "- **System RAM** : {} total, {} used\n",
        format_gib(snapshot.cpu.ram_total),
        format_gib(snapshot.cpu.ram_used)
    ));

    // 4. Hardware (GPUs & VRAM)
    if snapshot.gpus.is_empty() {
        if std::env::var("NEURADEX_DISABLE_GPU")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            report.push_str("- **GPU** : None (GPU disabled via `NEURADEX_DISABLE_GPU=1`)\n\n");
        } else {
            report.push_str("- **GPU** : No dedicated GPU detected (CPU & RAM fallback mode)\n\n");
        }
    } else {
        report.push_str(&format!("- **Detected GPU(s)** : {}\n", snapshot.gpus.len()));
        for (i, gpu) in snapshot.gpus.iter().enumerate() {
            let temp_str = gpu
                .temperature_c
                .map(|t| format!(" • {}°C", t))
                .unwrap_or_default();
            let power_str = gpu
                .power_watts
                .map(|p| format!(" • {:.1}W", p))
                .unwrap_or_default();
            report.push_str(&format!("  - **GPU #{}** : {}\n", i, gpu.name));
            report.push_str(&format!(
                "    - VRAM : {} total ({} used, {} free){}{}\n",
                format_gib(gpu.vram_total),
                format_gib(gpu.vram_used),
                format_gib(gpu.vram_free),
                temp_str,
                power_str
            ));
        }
        report.push('\n');
    }

    // 5. Ollama Service & Storage
    report.push_str("#### 🦙 Ollama Service & Storage\n");
    report.push_str(&format!("- **Configured Host** : `{}`\n", config.ollama_host));
    report.push_str(&format!(
        "- **Network Origins (CORS)** : `{}`\n",
        config.ollama_origins.as_deref().unwrap_or("Default (restricted)")
    ));
    report.push_str(&format!(
        "- **Exposure Mode** : `{:?}`\n",
        config.exposure_mode
    ));
    report.push_str(&format!(
        "- **Storage Directory** : `{}`\n",
        config
            .models_directory
            .as_deref()
            .unwrap_or("Default (~/.ollama/models)")
    ));
    report.push_str(&format!(
        "- **Flash Attention** : {}\n",
        if config.flash_attention {
            "Enabled"
        } else {
            "Disabled"
        }
    ));
    report.push_str(&format!(
        "- **Keep Alive** : {}\n",
        config.keep_alive.as_deref().unwrap_or("Default (5m)")
    ));

    // 6. Ollama Connectivity & Installed Models
    let client = OllamaClient::new(config.ollama_host.clone());
    match client.get_version().await {
        Ok(ver) => {
            report.push_str(&format!(
                "- **Ollama Status** : 🟢 Connected (daemon version `{}`)\n",
                ver
            ));

            // List local models
            match client.list_tags().await {
                Ok(tags) => {
                    report.push_str(&format!(
                        "- **Installed Models** : {} model(s)\n",
                        tags.len()
                    ));
                    for m in tags.iter().take(15) {
                        let size_gib = m.size as f64 / (1024.0 * 1024.0 * 1024.0);
                        report.push_str(&format!("  - `{}` ({:.2} GiB)\n", m.name, size_gib));
                    }
                    if tags.len() > 15 {
                        report.push_str(&format!(
                            "  - *... and {} more model(s)*\n",
                            tags.len() - 15
                        ));
                    }
                }
                Err(e) => {
                    report.push_str(&format!(
                        "- **Installed Models** : Error listing models ({})\n",
                        e
                    ));
                }
            }

            // List loaded models
            if let Ok(running) = client.list_running().await {
                if running.is_empty() {
                    report.push_str("- **Loaded Models (VRAM)** : None currently active\n");
                } else {
                    report.push_str(&format!(
                        "- **Loaded Models (VRAM)** : {} active\n",
                        running.len()
                    ));
                    for r in running {
                        let vram_gib = r.size_vram as f64 / (1024.0 * 1024.0 * 1024.0);
                        report.push_str(&format!(
                            "  - `{}` (VRAM: {:.2} GiB)\n",
                            r.name, vram_gib
                        ));
                    }
                }
            }
        }
        Err(e) => {
            report.push_str(&format!(
                "- **Ollama Status** : 🔴 Disconnected / Unreachable (`{}`)\n",
                e
            ));
        }
    }

    report.push_str("\n---\n*Report generated by NeuraDex for diagnostics and technical support.*\n");

    report
}
