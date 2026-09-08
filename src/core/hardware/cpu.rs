use sysinfo::System;

#[derive(Debug, Clone, Default)]
pub struct CpuMetrics {
    pub cpu_name: String,
    pub cpu_count: usize,
    pub cpu_usage_percent: f32,
    pub ram_total: u64,
    pub ram_used: u64,
}

pub struct CpuBackend {
    system: System,
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuBackend {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        Self { system }
    }

    pub fn get_metrics(&mut self) -> CpuMetrics {
        self.system.refresh_memory();
        self.system.refresh_cpu_usage();

        let cpu_count = self.system.cpus().len();
        let cpu_name = self
            .system
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Generic CPU".to_string());

        let cpu_usage_percent = self.system.global_cpu_usage();
        let ram_total = self.system.total_memory();
        let ram_used = self.system.used_memory();

        CpuMetrics {
            cpu_name,
            cpu_count,
            cpu_usage_percent,
            ram_total,
            ram_used,
        }
    }
}
