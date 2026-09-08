use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct GpuMetrics {
    pub index: usize,
    pub name: String,
    pub vram_total: u64,
    pub vram_used: u64,
    pub vram_free: u64,
    pub temperature_c: Option<u32>,
    pub utilization_percent: Option<u32>,
    pub power_watts: Option<f32>,
}

pub struct NvidiaBackend {
    nvml: Option<Nvml>,
}

impl Default for NvidiaBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses CUDA_VISIBLE_DEVICES string into an optional list of allowed device indices.
/// Returns:
/// - `None` if the variable is not set (all devices visible)
/// - `Some(vec![])` if empty, "-1", or "none" (no devices visible / CPU-only mode)
/// - `Some(vec![...])` if specific device indices are allowed (e.g. "0,1" -> vec![0, 1])
pub fn parse_cuda_visible_devices(raw: Option<&str>) -> Option<Vec<usize>> {
    let s = raw?;
    let trimmed = s.trim();
    if trimmed.is_empty()
        || trimmed == "-1"
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("nodevfiles")
    {
        return Some(Vec::new());
    }
    let indices: Vec<usize> = trimmed
        .split(',')
        .filter_map(|part| part.trim().parse::<usize>().ok())
        .collect();
    Some(indices)
}

fn get_cuda_visible_devices() -> Option<Vec<usize>> {
    let var = std::env::var("CUDA_VISIBLE_DEVICES").ok();
    parse_cuda_visible_devices(var.as_deref())
}

impl NvidiaBackend {
    pub fn new() -> Self {
        let nvml = match Nvml::init() {
            Ok(n) => Some(n),
            Err(e) => {
                tracing::info!(
                    "Direct NVML initialization not available ({}), falling back to nvidia-smi.",
                    e
                );
                None
            }
        };

        Self { nvml }
    }

    /// Queries metrics for all available NVIDIA GPUs on the system
    pub fn get_all_metrics(&self) -> Vec<GpuMetrics> {
        let allowed = get_cuda_visible_devices();
        if let Some(ref list) = allowed {
            if list.is_empty() {
                return Vec::new();
            }
        }

        // 1. Try native NVML directly
        if let Some(ref nvml) = self.nvml {
            if let Ok(count) = nvml.device_count() {
                let mut gpus = Vec::new();
                for i in 0..count {
                    let idx = i as usize;
                    if let Some(ref list) = allowed {
                        if !list.contains(&idx) {
                            continue;
                        }
                    }
                    if let Ok(device) = nvml.device_by_index(i) {
                        let name = device.name().unwrap_or_else(|_| format!("NVIDIA GPU {}", i));
                        if let Ok(mem) = device.memory_info() {
                            let temp = device.temperature(TemperatureSensor::Gpu).ok();
                            let util = device.utilization_rates().ok().map(|u| u.gpu);
                            let power = device.power_usage().ok().map(|p| p as f32 / 1000.0);

                            gpus.push(GpuMetrics {
                                index: idx,
                                name,
                                vram_total: mem.total,
                                vram_used: mem.used,
                                vram_free: mem.free,
                                temperature_c: temp,
                                utilization_percent: util,
                                power_watts: power,
                            });
                        }
                    }
                }
                if !gpus.is_empty() {
                    return gpus;
                }
            }
        }

        // 2. Fallback to nvidia-smi if NVML is not bound
        let mut fallback = Self::fallback_nvidia_smi_all();
        if let Some(ref list) = allowed {
            fallback.retain(|g| list.contains(&g.index));
        }
        fallback
    }

    /// Legacy / Single-GPU backward compatible helper returning first or aggregated GPU
    pub fn get_metrics(&self) -> Option<GpuMetrics> {
        let all = self.get_all_metrics();
        if all.is_empty() {
            None
        } else if all.len() == 1 {
            all.into_iter().next()
        } else {
            Some(aggregate_gpus(&all))
        }
    }

    fn fallback_nvidia_smi_all() -> Vec<GpuMetrics> {
        let output = match Command::new("nvidia-smi")
            .args([
                "--query-gpu=index,name,memory.total,memory.used,memory.free,temperature.gpu,utilization.gpu,power.draw",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };

        if !output.status.success() {
            return Vec::new();
        }

        let txt = String::from_utf8_lossy(&output.stdout);
        let mut gpus = Vec::new();

        for (row_idx, line) in txt.lines().enumerate() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 5 {
                let idx: usize = parts[0].parse().unwrap_or(row_idx);
                let name = parts[1].to_string();
                let total_mib: u64 = parts[2].parse().unwrap_or(0);
                let used_mib: u64 = parts[3].parse().unwrap_or(0);
                let free_mib: u64 = parts[4].parse().unwrap_or(0);

                let temp: Option<u32> = parts.get(5).and_then(|s| s.parse().ok());
                let util: Option<u32> = parts.get(6).and_then(|s| s.parse().ok());
                let power: Option<f32> = parts.get(7).and_then(|s| s.parse().ok());

                gpus.push(GpuMetrics {
                    index: idx,
                    name,
                    vram_total: total_mib * 1024 * 1024,
                    vram_used: used_mib * 1024 * 1024,
                    vram_free: free_mib * 1024 * 1024,
                    temperature_c: temp,
                    utilization_percent: util,
                    power_watts: power,
                });
            }
        }

        gpus
    }
}

/// Helper function to aggregate multiple GPUs into a unified pool metrics object
pub fn aggregate_gpus(gpus: &[GpuMetrics]) -> GpuMetrics {
    if gpus.is_empty() {
        return GpuMetrics {
            index: 0,
            name: "GPU".to_string(),
            vram_total: 0,
            vram_used: 0,
            vram_free: 0,
            temperature_c: None,
            utilization_percent: None,
            power_watts: None,
        };
    }

    if gpus.len() == 1 {
        return gpus[0].clone();
    }

    let all_same_name = gpus.iter().all(|g| g.name == gpus[0].name);
    let combined_name = if all_same_name {
        format!("{}x {}", gpus.len(), gpus[0].name)
    } else {
        gpus.iter()
            .map(|g| {
                g.name
                    .replace("NVIDIA GeForce ", "")
                    .replace("NVIDIA ", "")
                    .replace("AMD Radeon ", "")
                    .replace("Intel ", "")
            })
            .collect::<Vec<_>>()
            .join(" + ")
    };

    let vram_total: u64 = gpus.iter().map(|g| g.vram_total).sum();
    let vram_used: u64 = gpus.iter().map(|g| g.vram_used).sum();
    let vram_free: u64 = gpus.iter().map(|g| g.vram_free).sum();

    let max_temp = gpus.iter().filter_map(|g| g.temperature_c).max();
    let avg_util = {
        let utils: Vec<u32> = gpus.iter().filter_map(|g| g.utilization_percent).collect();
        if utils.is_empty() {
            None
        } else {
            Some((utils.iter().sum::<u32>() as usize / utils.len()) as u32)
        }
    };
    let total_power: Option<f32> = {
        let powers: Vec<f32> = gpus.iter().filter_map(|g| g.power_watts).collect();
        if powers.is_empty() {
            None
        } else {
            Some(powers.iter().sum())
        }
    };

    GpuMetrics {
        index: 0,
        name: combined_name,
        vram_total,
        vram_used,
        vram_free,
        temperature_c: max_temp,
        utilization_percent: avg_util,
        power_watts: total_power,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_gpu_aggregation() {
        let gpu0 = GpuMetrics {
            index: 0,
            name: "NVIDIA GeForce RTX 3090".to_string(),
            vram_total: 24 * 1024 * 1024 * 1024,
            vram_used: 12 * 1024 * 1024 * 1024,
            vram_free: 12 * 1024 * 1024 * 1024,
            temperature_c: Some(55),
            utilization_percent: Some(60),
            power_watts: Some(250.0),
        };

        let gpu1 = GpuMetrics {
            index: 1,
            name: "NVIDIA GeForce RTX 3090".to_string(),
            vram_total: 24 * 1024 * 1024 * 1024,
            vram_used: 10 * 1024 * 1024 * 1024,
            vram_free: 14 * 1024 * 1024 * 1024,
            temperature_c: Some(62),
            utilization_percent: Some(70),
            power_watts: Some(240.0),
        };

        let agg = aggregate_gpus(&[gpu0.clone(), gpu1.clone()]);
        assert_eq!(agg.name, "2x NVIDIA GeForce RTX 3090");
        assert_eq!(agg.vram_total, 48 * 1024 * 1024 * 1024);
        assert_eq!(agg.vram_used, 22 * 1024 * 1024 * 1024);
        assert_eq!(agg.vram_free, 26 * 1024 * 1024 * 1024);
        assert_eq!(agg.temperature_c, Some(62));
        assert_eq!(agg.utilization_percent, Some(65));
        assert_eq!(agg.power_watts, Some(490.0));

        // Mixed GPUs
        let gpu2 = GpuMetrics {
            index: 1,
            name: "NVIDIA GeForce RTX 4080".to_string(),
            vram_total: 16 * 1024 * 1024 * 1024,
            vram_used: 8 * 1024 * 1024 * 1024,
            vram_free: 8 * 1024 * 1024 * 1024,
            temperature_c: Some(50),
            utilization_percent: Some(40),
            power_watts: Some(200.0),
        };

        let agg_mixed = aggregate_gpus(&[gpu0, gpu2]);
        assert_eq!(agg_mixed.name, "RTX 3090 + RTX 4080");
        assert_eq!(agg_mixed.vram_total, 40 * 1024 * 1024 * 1024);
    }
}


