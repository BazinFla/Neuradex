use super::nvidia::GpuMetrics;
use std::fs;
use std::path::{Path, PathBuf};

/// Intel Backend (Arc / Xe / i915 / iGPU)
pub struct IntelBackend {
    drm_path: PathBuf,
}

impl Default for IntelBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl IntelBackend {
    pub fn new() -> Self {
        Self::with_drm_path(PathBuf::from("/sys/class/drm"))
    }

    pub fn with_drm_path(drm_path: PathBuf) -> Self {
        Self { drm_path }
    }

    /// Queries metrics for all available Intel GPUs on the system
    pub fn get_all_metrics(&self) -> Vec<GpuMetrics> {
        self.scan_sysfs_gpus()
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
            if let Some(gpu) = Self::parse_intel_card(&card_path, idx) {
                gpus.push(gpu);
            }
        }

        gpus
    }

    fn parse_intel_card(card_path: &Path, default_index: usize) -> Option<GpuMetrics> {
        let device_dir = card_path.join("device");
        if !device_dir.exists() {
            return None;
        }

        // Verify driver is Intel (xe or i915) or vendor is 0x8086
        if !Self::is_intel_device(&device_dir) {
            return None;
        }

        // 1. Read VRAM metrics (dedicated memory on Intel Arc / Xe / i915 dGPU)
        let (vram_total, vram_used, is_discrete) = Self::read_vram_metrics(&device_dir);
        let vram_free = vram_total.saturating_sub(vram_used);

        // 2. Read GPU Load / Utilization
        let utilization_percent = Self::read_sysfs_u32(&device_dir.join("gpu_busy_percent"));

        // 3. Read Hardware Monitoring (hwmon) Sensors
        let (temperature_c, power_watts) = Self::read_hwmon_sensors(&device_dir);

        // 4. Determine Model Name
        let name = Self::read_sysfs_string(&device_dir.join("product_name"))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                if is_discrete {
                    format!("Intel Arc GPU {}", default_index)
                } else {
                    format!("Intel Graphics (iGPU {})", default_index)
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

    fn is_intel_device(device_dir: &Path) -> bool {
        // Check uevent file for DRIVER=xe or DRIVER=i915
        if let Some(uevent) = Self::read_sysfs_string(&device_dir.join("uevent")) {
            for line in uevent.lines() {
                if line.starts_with("DRIVER=") {
                    let driver = line.trim_start_matches("DRIVER=").trim();
                    if driver == "xe" || driver == "i915" {
                        return true;
                    }
                }
            }
        }

        // Check driver symlink
        if let Ok(driver_target) = fs::read_link(device_dir.join("driver")) {
            let target_str = driver_target.to_string_lossy();
            if target_str.ends_with("/xe") || target_str.ends_with("/i915") || target_str == "xe" || target_str == "i915" {
                return true;
            }
        }

        // Check vendor ID (0x8086 is Intel)
        if let Some(vendor) = Self::read_sysfs_string(&device_dir.join("vendor")) {
            if vendor.trim() == "0x8086" {
                return true;
            }
        }

        false
    }

    fn read_vram_metrics(device_dir: &Path) -> (u64, u64, bool) {
        // Check 'xe' driver paths: tile0/vram0_total_bytes or vram0_total_bytes
        let xe_tile0 = device_dir.join("tile0");
        if let Some(total) = Self::read_sysfs_u64(&xe_tile0.join("vram0_total_bytes"))
            .or_else(|| Self::read_sysfs_u64(&xe_tile0.join("vram_total_bytes")))
            .or_else(|| Self::read_sysfs_u64(&device_dir.join("vram0_total_bytes")))
        {
            let used = Self::read_sysfs_u64(&xe_tile0.join("vram0_used_bytes"))
                .or_else(|| Self::read_sysfs_u64(&xe_tile0.join("vram_used_bytes")))
                .or_else(|| Self::read_sysfs_u64(&device_dir.join("vram0_used_bytes")))
                .unwrap_or(0);
            return (total, used, true);
        }

        // Check 'i915' driver paths: lmem_total_bytes (for discrete Arc cards under i915)
        if let Some(total) = Self::read_sysfs_u64(&device_dir.join("lmem_total_bytes")) {
            let used = Self::read_sysfs_u64(&device_dir.join("lmem_used_bytes")).unwrap_or(0);
            return (total, used, true);
        }

        // Integrated Intel GPU (shares system RAM, no dedicated local VRAM)
        (0, 0, false)
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

            // Read temperature (temp1_input in millidegrees Celsius)
            let temp_millideg = Self::read_sysfs_u32(&hwmon_dir.join("temp1_input"));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_intel_arc_xe_sysfs_mock_parsing() {
        let temp_dir = std::env::temp_dir().join(format!("neuradex_test_intel_arc_{}", std::process::id()));
        let card0_dir = temp_dir.join("card0").join("device");
        let tile0_dir = card0_dir.join("tile0");
        let hwmon_dir = card0_dir.join("hwmon").join("hwmon2");
        fs::create_dir_all(&tile0_dir).unwrap();
        fs::create_dir_all(&hwmon_dir).unwrap();

        // Mock Intel Arc Xe sysfs
        File::create(card0_dir.join("uevent")).unwrap().write_all(b"DRIVER=xe\nPCI_SLOT_NAME=0000:03:00.0\n").unwrap();
        File::create(card0_dir.join("vendor")).unwrap().write_all(b"0x8086\n").unwrap();
        File::create(card0_dir.join("product_name")).unwrap().write_all(b"Intel Arc A770 Graphics\n").unwrap();
        File::create(tile0_dir.join("vram0_total_bytes")).unwrap().write_all(b"17179869184\n").unwrap(); // 16 GB
        File::create(tile0_dir.join("vram0_used_bytes")).unwrap().write_all(b"4294967296\n").unwrap(); // 4 GB
        File::create(card0_dir.join("gpu_busy_percent")).unwrap().write_all(b"35\n").unwrap();
        File::create(hwmon_dir.join("name")).unwrap().write_all(b"xe\n").unwrap();
        File::create(hwmon_dir.join("temp1_input")).unwrap().write_all(b"52000\n").unwrap(); // 52°C
        File::create(hwmon_dir.join("power1_average")).unwrap().write_all(b"150000000\n").unwrap(); // 150W

        let backend = IntelBackend::with_drm_path(temp_dir.clone());
        let metrics = backend.get_all_metrics();

        assert_eq!(metrics.len(), 1);
        let gpu = &metrics[0];
        assert_eq!(gpu.name, "Intel Arc A770 Graphics");
        assert_eq!(gpu.vram_total, 17179869184);
        assert_eq!(gpu.vram_used, 4294967296);
        assert_eq!(gpu.vram_free, 17179869184 - 4294967296);
        assert_eq!(gpu.utilization_percent, Some(35));
        assert_eq!(gpu.temperature_c, Some(52));
        assert_eq!(gpu.power_watts, Some(150.0));

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_intel_igpu_sysfs_mock_parsing() {
        let temp_dir = std::env::temp_dir().join(format!("neuradex_test_intel_igpu_{}", std::process::id()));
        let card0_dir = temp_dir.join("card0").join("device");
        fs::create_dir_all(&card0_dir).unwrap();

        // Mock Intel iGPU (i915 driver, no local VRAM)
        File::create(card0_dir.join("uevent")).unwrap().write_all(b"DRIVER=i915\n").unwrap();
        File::create(card0_dir.join("vendor")).unwrap().write_all(b"0x8086\n").unwrap();

        let backend = IntelBackend::with_drm_path(temp_dir.clone());
        let metrics = backend.get_all_metrics();

        assert_eq!(metrics.len(), 1);
        let gpu = &metrics[0];
        assert_eq!(gpu.name, "Intel Graphics (iGPU 0)");
        assert_eq!(gpu.vram_total, 0);
        assert_eq!(gpu.vram_used, 0);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
