pub mod amd;
pub mod cpu;
pub mod estimator;
pub mod intel;
pub mod nvidia;

pub use amd::AmdBackend;
pub use cpu::{CpuBackend, CpuMetrics};
pub use intel::IntelBackend;
pub use nvidia::{aggregate_gpus, parse_cuda_visible_devices, GpuMetrics, NvidiaBackend};

#[derive(Debug, Clone, Default)]
pub struct HardwareSnapshot {
    pub gpu: Option<GpuMetrics>,
    pub gpus: Vec<GpuMetrics>,
    pub cpu: CpuMetrics,
}

pub struct HardwareMonitor {
    nvidia: NvidiaBackend,
    amd: AmdBackend,
    intel: IntelBackend,
    cpu: CpuBackend,
}

impl Default for HardwareMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareMonitor {
    pub fn new() -> Self {
        Self {
            nvidia: NvidiaBackend::new(),
            amd: AmdBackend::new(),
            intel: IntelBackend::new(),
            cpu: CpuBackend::new(),
        }
    }

    pub fn snapshot(&mut self) -> HardwareSnapshot {
        if std::env::var("NEURADEX_DISABLE_GPU")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            let cpu = self.cpu.get_metrics();
            return HardwareSnapshot {
                gpu: None,
                gpus: Vec::new(),
                cpu,
            };
        }

        let mut gpus = Vec::new();

        // 1. Scan NVIDIA GPUs
        let mut nvidia_gpus = self.nvidia.get_all_metrics();
        gpus.append(&mut nvidia_gpus);

        // 2. Scan AMD GPUs
        let mut amd_gpus = self.amd.get_all_metrics();
        gpus.append(&mut amd_gpus);

        // 3. Scan Intel GPUs
        let mut intel_gpus = self.intel.get_all_metrics();
        gpus.append(&mut intel_gpus);

        // Re-index all detected GPUs sequentially (0, 1, 2, ...)
        for (i, gpu) in gpus.iter_mut().enumerate() {
            gpu.index = i;
        }

        let gpu = if gpus.is_empty() {
            None
        } else if gpus.len() == 1 {
            Some(gpus[0].clone())
        } else {
            Some(aggregate_gpus(&gpus))
        };
        let cpu = self.cpu.get_metrics();

        HardwareSnapshot { gpu, gpus, cpu }
    }
}

