use super::nvidia::GpuMetrics;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// AMD Backend (Radeon / ROCm / amdgpu)
pub struct AmdBackend {
    drm_path: PathBuf,
}

impl Default for AmdBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AmdBackend {
    pub fn new() -> Self {
        Self::with_drm_path(PathBuf::from("/sys/class/drm"))
    }

    pub fn with_drm_path(drm_path: PathBuf) -> Self {
        Self { drm_path }
    }

    /// Queries metrics for all available AMD GPUs on the system
    pub fn get_all_metrics(&self) -> Vec<GpuMetrics> {
        // 1. Try reading Linux sysfs for amdgpu / radeon
        let sysfs_gpus = self.scan_sysfs_gpus();
        if !sysfs_gpus.is_empty() {
            return sysfs_gpus;
        }

        // 2. Fallback to rocm-smi CLI if sysfs returned nothing
        Self::fallback_rocm_smi_all()
    }

    /// Single-GPU backward compatible helper returning first or aggregated GPU
    pub fn get_metrics(&self) -> Option<GpuMetrics> {
        let all = self.get_all_metrics();
        if all.is_empty() {
            None
        } else if all.len() == 1 {
            all.into_iter().next()
        } else {
            Some(super::nvidia::aggregate_gpus(&all))
        }
    }

    fn scan_sysfs_gpus(&self) -> Vec<GpuMetrics> {
        if !self.drm_path.exists() {
            return Vec::new();
        }

        let read_dir = match fs::read_dir(&self.drm_path) {
            Ok(rd) => rd,
            Err(_) => return Vec::new(),
        };

        let mut card_dirs: Vec<PathBuf> = Vec::new();
        for entry in read_dir.flatten() {
            let filename = entry.file_name();
            let name_str = filename.to_string_lossy();
            // Match card0, card1, etc. Avoid sub-elements like card0-DP-1
            if name_str.starts_with("card")
                && name_str[4..].chars().all(|c| c.is_ascii_digit())
                && !name_str[4..].is_empty()
            {
                card_dirs.push(entry.path());
            }
        }

        // Sort by card index (card0, card1, ...)
        card_dirs.sort_by_key(|p| {
            let s = p.file_name().unwrap_or_default().to_string_lossy();
            s[4..].parse::<usize>().unwrap_or(usize::MAX)
        });

        let mut gpus = Vec::new();
        for (idx, card_path) in card_dirs.into_iter().enumerate() {
            if let Some(gpu) = Self::parse_amd_card(&card_path, idx) {
                gpus.push(gpu);
            }
        }

        gpus
    }

    fn parse_amd_card(card_path: &Path, default_index: usize) -> Option<GpuMetrics> {
        let device_dir = card_path.join("device");
        if !device_dir.exists() {
            return None;
        }

        // Verify driver is amdgpu or radeon
        if !Self::is_amd_device(&device_dir) {
            return None;
        }

        // 1. Read VRAM metrics
        let mut vram_total = Self::read_sysfs_u64(&device_dir.join("mem_info_vram_total")).unwrap_or(0);
        let mut vram_used = Self::read_sysfs_u64(&device_dir.join("mem_info_vram_used")).unwrap_or(0);

        // Fallback for APUs / integrated graphics where dedicated VRAM is 0
        if vram_total == 0 {
            vram_total = Self::read_sysfs_u64(&device_dir.join("mem_info_gtt_total")).unwrap_or(0);
            vram_used = Self::read_sysfs_u64(&device_dir.join("mem_info_gtt_used")).unwrap_or(0);
        }

        let vram_free = vram_total.saturating_sub(vram_used);

        // 2. Read GPU Load / Utilization
        let utilization_percent = Self::read_sysfs_u32(&device_dir.join("gpu_busy_percent"));

        // 3. Read Hardware Monitoring (hwmon) Sensors
        let (temperature_c, power_watts) = Self::read_hwmon_sensors(&device_dir);

        // 4. Read GPU Model Name
        let name = Self::read_sysfs_string(&device_dir.join("product_name"))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                if vram_total == 0 {
                    "AMD Radeon Graphics (APU)".to_string()
                } else {
                    format!("AMD Radeon GPU {}", default_index)
                }
            });

        Some(GpuMetrics {
            index: default_index,
            name,
            vram_total,
            vram_used,
            vram_free,
            temperature_c,
            utilization_percent,
            power_watts,
        })
    }

    fn is_amd_device(device_dir: &Path) -> bool {
        // Check uevent file for DRIVER=amdgpu or DRIVER=radeon
        if let Some(uevent) = Self::read_sysfs_string(&device_dir.join("uevent")) {
            for line in uevent.lines() {
                if line.starts_with("DRIVER=") {
                    let driver = line.trim_start_matches("DRIVER=").trim();
                    if driver == "amdgpu" || driver == "radeon" {
                        return true;
                    }
                }
            }
        }

        // Check driver symlink
        if let Ok(driver_target) = fs::read_link(device_dir.join("driver")) {
            let target_str = driver_target.to_string_lossy();
            if target_str.ends_with("/amdgpu") || target_str.ends_with("/radeon") || target_str == "amdgpu" || target_str == "radeon" {
                return true;
            }
        }

        // Check vendor ID (0x1002 is AMD/ATI)
        if let Some(vendor) = Self::read_sysfs_string(&device_dir.join("vendor")) {
            if vendor.trim() == "0x1002" {
                return true;
            }
        }

        false
    }

    fn read_hwmon_sensors(device_dir: &Path) -> (Option<u32>, Option<f32>) {
        let hwmon_root = device_dir.join("hwmon");
        if !hwmon_root.exists() {
            return (None, None);
        }

        let read_dir = match fs::read_dir(&hwmon_root) {
            Ok(rd) => rd,
            Err(_) => return (None, None),
        };

        for entry in read_dir.flatten() {
            let hwmon_dir = entry.path();
            if !hwmon_dir.is_dir() {
                continue;
            }

            // Read temperature (temp1_input or temp2_input in millidegrees Celsius)
            let temp_millideg = Self::read_sysfs_u32(&hwmon_dir.join("temp1_input"))
                .or_else(|| Self::read_sysfs_u32(&hwmon_dir.join("temp2_input")));
            let temperature_c = temp_millideg.map(|m| m / 1000);

            // Read power (power1_average or power1_input in microWatts)
            let power_microwatts = Self::read_sysfs_u64(&hwmon_dir.join("power1_average"))
                .or_else(|| Self::read_sysfs_u64(&hwmon_dir.join("power1_input")));
            let power_watts = power_microwatts.map(|p| p as f32 / 1_000_000.0);

            if temperature_c.is_some() || power_watts.is_some() {
                return (temperature_c, power_watts);
            }
        }

        (None, None)
    }

    fn read_sysfs_u64(path: &Path) -> Option<u64> {
        fs::read_to_string(path).ok().and_then(|s| s.trim().parse::<u64>().ok())
    }

    fn read_sysfs_u32(path: &Path) -> Option<u32> {
        fs::read_to_string(path).ok().and_then(|s| s.trim().parse::<u32>().ok())
    }

    fn read_sysfs_string(path: &Path) -> Option<String> {
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    fn fallback_rocm_smi_all() -> Vec<GpuMetrics> {
        let output = match Command::new("rocm-smi")
            .args(["--showmeminfo", "vram", "--showtemp", "--showuse", "--showpower", "--csv"])
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

        // CSV Parser for rocm-smi
        for (row_idx, line) in txt.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("device") || line.starts_with("GPU") {
                continue;
            }

            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 2 {
                let idx: usize = parts[0].trim_start_matches("card").parse().unwrap_or(row_idx);
                gpus.push(GpuMetrics {
                    index: idx,
                    name: format!("AMD Radeon GPU {}", idx),
                    vram_total: 0,
                    vram_used: 0,
                    vram_free: 0,
                    temperature_c: None,
                    utilization_percent: None,
                    power_watts: None,
                });
            }
        }

        gpus
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_amd_sysfs_mock_parsing() {
        let temp_dir = std::env::temp_dir().join(format!("neuradex_test_amd_sysfs_{}", std::process::id()));
        let card0_dir = temp_dir.join("card0").join("device");
        let hwmon_dir = card0_dir.join("hwmon").join("hwmon1");
        fs::create_dir_all(&hwmon_dir).unwrap();

        // Mock sysfs files
        File::create(card0_dir.join("uevent")).unwrap().write_all(b"DRIVER=amdgpu\nPCI_SLOT_NAME=0000:03:00.0\n").unwrap();
        File::create(card0_dir.join("vendor")).unwrap().write_all(b"0x1002\n").unwrap();
        File::create(card0_dir.join("product_name")).unwrap().write_all(b"AMD Radeon RX 7900 XTX\n").unwrap();
        File::create(card0_dir.join("mem_info_vram_total")).unwrap().write_all(b"25769803776\n").unwrap(); // 24 GB
        File::create(card0_dir.join("mem_info_vram_used")).unwrap().write_all(b"8589934592\n").unwrap(); // 8 GB
        File::create(card0_dir.join("gpu_busy_percent")).unwrap().write_all(b"42\n").unwrap();
        File::create(hwmon_dir.join("name")).unwrap().write_all(b"amdgpu\n").unwrap();
        File::create(hwmon_dir.join("temp1_input")).unwrap().write_all(b"55000\n").unwrap(); // 55°C
        File::create(hwmon_dir.join("power1_average")).unwrap().write_all(b"210000000\n").unwrap(); // 210W

        let backend = AmdBackend::with_drm_path(temp_dir.clone());
        let metrics = backend.get_all_metrics();

        assert_eq!(metrics.len(), 1);
        let gpu = &metrics[0];
        assert_eq!(gpu.name, "AMD Radeon RX 7900 XTX");
        assert_eq!(gpu.vram_total, 25769803776);
        assert_eq!(gpu.vram_used, 8589934592);
        assert_eq!(gpu.vram_free, 25769803776 - 8589934592);
        assert_eq!(gpu.utilization_percent, Some(42));
        assert_eq!(gpu.temperature_c, Some(55));
        assert_eq!(gpu.power_watts, Some(210.0));

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_amd_non_amd_device_ignored() {
        let temp_dir = std::env::temp_dir().join(format!("neuradex_test_non_amd_{}", std::process::id()));
        let card0_dir = temp_dir.join("card0").join("device");
        fs::create_dir_all(&card0_dir).unwrap();

        File::create(card0_dir.join("uevent")).unwrap().write_all(b"DRIVER=nouveau\n").unwrap();
        File::create(card0_dir.join("vendor")).unwrap().write_all(b"0x10de\n").unwrap();

        let backend = AmdBackend::with_drm_path(temp_dir.clone());
        let metrics = backend.get_all_metrics();

        assert!(metrics.is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }
}
