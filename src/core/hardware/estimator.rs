use crate::core::hardware::HardwareSnapshot;
use crate::core::hub::HardwareFitness;

pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    const TIB: u64 = GIB * 1024;

    let is_fr = crate::core::i18n::get_effective_language() == "fr";

    if bytes >= TIB {
        format!("{:.2} {}", bytes as f64 / TIB as f64, if is_fr { "To" } else { "TB" })
    } else if bytes >= GIB {
        format!("{:.2} {}", bytes as f64 / GIB as f64, if is_fr { "Go" } else { "GB" })
    } else if bytes >= MIB {
        format!("{:.1} {}", bytes as f64 / MIB as f64, if is_fr { "Mo" } else { "MB" })
    } else if bytes >= KIB {
        format!("{:.0} {}", bytes as f64 / KIB as f64, if is_fr { "Ko" } else { "KB" })
    } else {
        format!("{} {}", bytes, if is_fr { "o" } else { "B" })
    }
}

pub fn format_gib(bytes: u64) -> String {
    let gib = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let unit = if crate::core::i18n::get_effective_language() == "fr" { "Go" } else { "GB" };
    format!("{:.1} {}", gib, unit)
}

/// Model topological and dimensional parameters for memory load estimation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelTopology {
    pub total_layers: u32,
    pub base_model_bytes: u64,
    pub context_length: u32,
    pub kv_heads: u32,
    pub head_dim: u32,
    pub sliding_window: Option<u32>,
}

impl Default for ModelTopology {
    fn default() -> Self {
        Self {
            total_layers: 32,
            base_model_bytes: 4 * 1024 * 1024 * 1024,
            context_length: 32768,
            kv_heads: 8,
            head_dim: 128,
            sliding_window: None,
        }
    }
}

/// Detailed result of predictive GPU VRAM and CPU RAM load calculation
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryEstimate {
    pub total_layers: u32,
    pub effective_gpu_layers: u32,
    pub effective_cpu_layers: u32,
    pub gpu_weights_bytes: u64,
    pub cpu_weights_bytes: u64,
    pub gpu_kv_bytes: u64,
    pub cpu_kv_bytes: u64,
    pub gpu_overhead_bytes: u64,
    pub cpu_overhead_bytes: u64,
    pub total_vram_bytes: u64,
    pub total_ram_bytes: u64,
    pub vram_total_available_bytes: u64,
    pub ram_total_available_bytes: u64,
    pub ram_free_available_bytes: u64,
    pub vram_fraction: f64,
    pub ram_fraction: f64,
    pub max_fluid_context: u32,
    pub advice_message: String,
    pub fitness: HardwareFitness,
}

/// Calculates predictive memory footprint of an LLM based on topology,
/// requested context window, and number of layers offloaded to GPU.
pub fn calculate_memory_estimate(
    topo: &ModelTopology,
    requested_num_ctx: u32,
    requested_num_gpu: i32,
    snap: &HardwareSnapshot,
) -> MemoryEstimate {
    let total_layers = topo.total_layers.max(1);
    let base_bytes = topo.base_model_bytes;
    let vram_total_available_bytes = snap.gpu.as_ref().map(|g| g.vram_total).unwrap_or(0);
    let vram_free_bytes = snap.gpu.as_ref().map(|g| g.vram_free).unwrap_or(0);
    let ram_total_available_bytes = snap.cpu.ram_total;
    let ram_free_available_bytes = snap.cpu.ram_total.saturating_sub(snap.cpu.ram_used);

    // Precise KV-Cache: 2 (K+V) * kv_heads * head_dim * 2 bytes (FP16) per layer per token
    let bytes_per_layer_per_token = (2 * topo.kv_heads.max(1) * topo.head_dim.max(64) * 2) as f64;
    let kv_cache_per_token = total_layers as f64 * bytes_per_layer_per_token;

    // Account for sliding window attention (e.g. Gemma 2 / Mistral)
    let effective_ctx_per_layer = if let Some(sw) = topo.sliding_window {
        if sw > 0 && requested_num_ctx > sw {
            (requested_num_ctx as f64 + sw as f64) / 2.0
        } else {
            requested_num_ctx as f64
        }
    } else {
        requested_num_ctx as f64
    };

    let (effective_gpu_layers, is_auto_split) = if requested_num_gpu < 0 {
        if vram_free_bytes == 0 && vram_total_available_bytes == 0 {
            (0, false)
        } else {
            // Usable VRAM budget based on actual free VRAM:
            // Ollama safety margin (~500 MB) + runtime overhead (~250 MB)
            let safety_headroom = 500 * 1024 * 1024;
            let gpu_overhead = 250 * 1024 * 1024;
            let target_free = if vram_free_bytes > 0 { vram_free_bytes } else { vram_total_available_bytes.saturating_sub(600 * 1024 * 1024) };
            let usable_vram = target_free.saturating_sub(safety_headroom + gpu_overhead);

            let bytes_per_layer_weight = base_bytes as f64 / total_layers as f64;
            let bytes_per_layer_kv = effective_ctx_per_layer * bytes_per_layer_per_token;
            let bytes_per_layer_total = bytes_per_layer_weight + bytes_per_layer_kv;

            if bytes_per_layer_total > 0.0 {
                let max_fit = (usable_vram as f64 / bytes_per_layer_total).floor() as u32;
                let fit_layers = max_fit.min(total_layers);
                (fit_layers, fit_layers < total_layers)
            } else {
                (total_layers, false)
            }
        }
    } else {
        ((requested_num_gpu.max(0) as u32).min(total_layers), false)
    };

    let effective_cpu_layers = total_layers.saturating_sub(effective_gpu_layers);
    let gpu_ratio = effective_gpu_layers as f64 / total_layers as f64;
    let cpu_ratio = effective_cpu_layers as f64 / total_layers as f64;

    let gpu_weights_bytes = (base_bytes as f64 * gpu_ratio) as u64;
    let cpu_weights_bytes = (base_bytes as f64 * cpu_ratio) as u64;

    let total_kv_bytes = (effective_ctx_per_layer * kv_cache_per_token) as u64;
    let gpu_kv_bytes = (total_kv_bytes as f64 * gpu_ratio) as u64;
    let cpu_kv_bytes = (total_kv_bytes as f64 * cpu_ratio) as u64;

    let gpu_overhead = if effective_gpu_layers > 0 { 250 * 1024 * 1024 } else { 0 };
    let cpu_overhead = 150 * 1024 * 1024;

    let total_vram_bytes = gpu_weights_bytes + gpu_kv_bytes + gpu_overhead;
    let total_ram_bytes = cpu_weights_bytes + cpu_kv_bytes + cpu_overhead;

    let vram_fraction = if vram_total_available_bytes > 0 {
        (total_vram_bytes as f64 / vram_total_available_bytes as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let ram_fraction = if ram_total_available_bytes > 0 {
        (total_ram_bytes as f64 / ram_total_available_bytes as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let max_fluid_context = if vram_total_available_bytes > 0 && effective_gpu_layers == total_layers {
        let vram_for_kv = vram_total_available_bytes
            .saturating_sub(gpu_weights_bytes)
            .saturating_sub(gpu_overhead);
        let kv_total_per_token = (kv_cache_per_token) as u64;
        vram_for_kv
            .checked_div(kv_total_per_token)
            .map_or(topo.context_length, |c| c as u32)
    } else {
        let ram_for_kv = ram_free_available_bytes
            .saturating_sub(cpu_weights_bytes)
            .saturating_sub(cpu_overhead);
        let kv_total_per_token = (kv_cache_per_token) as u64;
        ram_for_kv
            .checked_div(kv_total_per_token)
            .map_or(topo.context_length, |c| c as u32)
    };



    let est_vram_gb = total_vram_bytes as f64 / 1_073_741_824.0;
    let est_ram_gb = total_ram_bytes as f64 / 1_073_741_824.0;
    let vram_total_gb = vram_total_available_bytes as f64 / 1_073_741_824.0;
    let ram_total_gb = ram_total_available_bytes as f64 / 1_073_741_824.0;
    let ram_free_gb = ram_free_available_bytes as f64 / 1_073_741_824.0;

    let (advice_message, fitness) = if vram_total_available_bytes == 0 {
        if total_ram_bytes <= ram_free_available_bytes {
            (
                crate::t!(
                    "model_settings.advice_cpu_smooth",
                    ram = est_ram_gb,
                    free = ram_free_gb as u64,
                    total = ram_total_gb as u64
                ),
                HardwareFitness::GpuOffload,
            )
        } else {
            (
                crate::t!(
                    "model_settings.advice_oom_risk",
                    ram = est_ram_gb,
                    free = ram_free_gb
                ),
                HardwareFitness::Insufficient,
            )
        }
    } else if effective_gpu_layers == total_layers && total_vram_bytes <= vram_total_available_bytes {
        (
            crate::t!(
                "model_settings.advice_full_gpu",
                ctx = max_fluid_context
            ),
            HardwareFitness::FullGpu,
        )
    } else if is_auto_split && total_ram_bytes <= ram_free_available_bytes {
        (
            crate::t!(
                "model_settings.advice_auto_split",
                gpu_layers = effective_gpu_layers,
                total_layers = total_layers,
                vram = est_vram_gb,
                cpu_layers = effective_cpu_layers,
                ram = est_ram_gb
            ),
            HardwareFitness::GpuOffload,
        )
    } else if effective_gpu_layers < total_layers
        && total_vram_bytes <= vram_total_available_bytes
        && total_ram_bytes <= ram_free_available_bytes
    {
        (
            crate::t!(
                "model_settings.advice_hybrid_manual",
                gpu_layers = effective_gpu_layers,
                total_layers = total_layers,
                vram = est_vram_gb,
                cpu_layers = effective_cpu_layers,
                ram = est_ram_gb
            ),
            HardwareFitness::GpuOffload,
        )
    } else if total_vram_bytes > vram_total_available_bytes {
        let over = est_vram_gb - vram_total_gb;
        (
            crate::t!(
                "model_settings.advice_vram_overflow",
                over = over,
                ctx = max_fluid_context
            ),
            HardwareFitness::Insufficient,
        )
    } else {
        (
            crate::t!(
                "model_settings.advice_high_memory",
                vram = est_vram_gb,
                ram = est_ram_gb
            ),
            HardwareFitness::Insufficient,
        )
    };

    MemoryEstimate {
        total_layers,
        effective_gpu_layers,
        effective_cpu_layers,
        gpu_weights_bytes,
        cpu_weights_bytes,
        gpu_kv_bytes,
        cpu_kv_bytes,
        gpu_overhead_bytes: gpu_overhead,
        cpu_overhead_bytes: cpu_overhead,
        total_vram_bytes,
        total_ram_bytes,
        vram_total_available_bytes,
        ram_total_available_bytes,
        ram_free_available_bytes,
        vram_fraction,
        ram_fraction,
        max_fluid_context,
        advice_message,
        fitness,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::hardware::{CpuMetrics, GpuMetrics};

    fn make_snapshot(gpu_total: u64, ram_total: u64, ram_used: u64) -> HardwareSnapshot {
        let gpu = if gpu_total > 0 {
            Some(GpuMetrics {
                index: 0,
                name: "NVIDIA RTX 4090".to_string(),
                vram_total: gpu_total,
                vram_used: 1024 * 1024 * 1024,
                vram_free: gpu_total - 1024 * 1024 * 1024,
                temperature_c: Some(45),
                power_watts: Some(50.0),
                utilization_percent: Some(0),
            })
        } else {
            None
        };

        HardwareSnapshot {
            cpu: CpuMetrics {
                cpu_name: "AMD Ryzen 9".to_string(),
                cpu_count: 16,
                cpu_usage_percent: 10.0,
                ram_total,
                ram_used,
            },
            gpus: gpu.clone().into_iter().collect(),
            gpu,
        }
    }


    #[test]
    fn test_memory_estimate_full_gpu() {
        let topo = ModelTopology {
            total_layers: 32,
            base_model_bytes: 4 * 1024 * 1024 * 1024, // 4 GB
            context_length: 32768,
            kv_heads: 8,
            head_dim: 128,
            sliding_window: None,
        };
        let snap = make_snapshot(16 * 1024 * 1024 * 1024, 32 * 1024 * 1024 * 1024, 8 * 1024 * 1024 * 1024);

        let est = calculate_memory_estimate(&topo, 4096, -1, &snap);
        assert_eq!(est.effective_gpu_layers, 32);
        assert_eq!(est.effective_cpu_layers, 0);
        assert_eq!(est.fitness, HardwareFitness::FullGpu);
        assert!(est.total_vram_bytes > 4 * 1024 * 1024 * 1024);
        assert!(est.vram_fraction < 1.0);
    }

    #[test]
    fn test_memory_estimate_hybrid_offload() {
        let topo = ModelTopology {
            total_layers: 32,
            base_model_bytes: 8 * 1024 * 1024 * 1024,
            context_length: 32768,
            kv_heads: 8,
            head_dim: 128,
            sliding_window: None,
        };
        let snap = make_snapshot(6 * 1024 * 1024 * 1024, 32 * 1024 * 1024 * 1024, 8 * 1024 * 1024 * 1024);

        // Offload 16 out of 32 layers
        let est = calculate_memory_estimate(&topo, 4096, 16, &snap);
        assert_eq!(est.effective_gpu_layers, 16);
        assert_eq!(est.effective_cpu_layers, 16);
        assert_eq!(est.fitness, HardwareFitness::GpuOffload);
    }

    #[test]
    fn test_memory_estimate_auto_split() {
        let topo = ModelTopology {
            total_layers: 42,
            base_model_bytes: 9_600_000_000, // 9.6 GB
            context_length: 131072,
            kv_heads: 2,
            head_dim: 512,
            sliding_window: Some(512),
        };
        // 8 GB available VRAM, 32 GB total RAM
        let snap = make_snapshot(8 * 1024 * 1024 * 1024, 32 * 1024 * 1024 * 1024, 8 * 1024 * 1024 * 1024);

        // In Auto mode (-1), Ollama must perform dynamic automatic split
        let est = calculate_memory_estimate(&topo, 4096, -1, &snap);
        assert!(est.effective_gpu_layers > 0 && est.effective_gpu_layers < 42);
        assert!(est.effective_cpu_layers > 0);
        assert_eq!(est.effective_gpu_layers + est.effective_cpu_layers, 42);
        assert!(est.total_vram_bytes <= 8 * 1024 * 1024 * 1024);
        assert!(est.total_ram_bytes > 1024 * 1024 * 1024);
        assert!(est.advice_message.contains("Auto-Split"));
    }

    #[test]
    fn test_memory_estimate_cpu_oom() {
        let topo = ModelTopology {
            total_layers: 32,
            base_model_bytes: 16 * 1024 * 1024 * 1024,
            context_length: 32768,
            kv_heads: 8,
            head_dim: 128,
            sliding_window: None,
        };
        // 8 GB total RAM, 7 GB used -> 1 GB free
        let snap = make_snapshot(0, 8 * 1024 * 1024 * 1024, 7 * 1024 * 1024 * 1024);

        let est = calculate_memory_estimate(&topo, 4096, 0, &snap);
        assert_eq!(est.fitness, HardwareFitness::Insufficient);
        assert!(est.advice_message.contains("OOM"));
    }
}
