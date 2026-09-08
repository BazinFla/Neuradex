#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::process::Command;
    use neuradex::core::hardware::{aggregate_gpus, AmdBackend, GpuMetrics, HardwareMonitor, IntelBackend};

    #[test]
    fn test_nvidia_smi_detect() {
        if let Ok(output) = Command::new("nvidia-smi").output() {
            println!("nvidia-smi detected successfully, status: {}", output.status);
        } else {
            println!("nvidia-smi not present (system without NVIDIA GPU or CI runner).");
        }
    }

    #[test]
    fn test_hardware_monitor_snapshot_safe() {
        let mut monitor = HardwareMonitor::new();
        let snapshot = monitor.snapshot();
        println!("Hardware snapshot: CPU load: {:.1}%, GPUs detected: {}", snapshot.cpu.cpu_usage_percent, snapshot.gpus.len());
        // Should not panic on any host system
    }

    #[test]
    fn test_mock_amd_and_intel_aggregation() {
        let amd_gpu = GpuMetrics {
            index: 0,
            name: "AMD Radeon RX 7900 XTX".to_string(),
            vram_total: 24 * 1024 * 1024 * 1024,
            vram_used: 6 * 1024 * 1024 * 1024,
            vram_free: 18 * 1024 * 1024 * 1024,
            temperature_c: Some(58),
            utilization_percent: Some(50),
            power_watts: Some(240.0),
        };

        let intel_gpu = GpuMetrics {
            index: 1,
            name: "Intel Arc A770 Graphics".to_string(),
            vram_total: 16 * 1024 * 1024 * 1024,
            vram_used: 4 * 1024 * 1024 * 1024,
            vram_free: 12 * 1024 * 1024 * 1024,
            temperature_c: Some(52),
            utilization_percent: Some(30),
            power_watts: Some(150.0),
        };

        let agg = aggregate_gpus(&[amd_gpu, intel_gpu]);
        assert_eq!(agg.name, "RX 7900 XTX + Arc A770 Graphics");
        assert_eq!(agg.vram_total, 40 * 1024 * 1024 * 1024);
        assert_eq!(agg.vram_used, 10 * 1024 * 1024 * 1024);
        assert_eq!(agg.vram_free, 30 * 1024 * 1024 * 1024);
        assert_eq!(agg.temperature_c, Some(58)); // max temp
        assert_eq!(agg.utilization_percent, Some(40)); // avg util (50 + 30)/2
        assert_eq!(agg.power_watts, Some(390.0)); // total power (240 + 150)
    }

    #[test]
    fn test_custom_sysfs_backends() {
        let temp_dir = std::env::temp_dir().join(format!("neuradex_test_multi_backend_{}", std::process::id()));
        let card0_device = temp_dir.join("card0").join("device");
        let card1_device = temp_dir.join("card1").join("device");
        fs::create_dir_all(&card0_device).unwrap();
        fs::create_dir_all(card1_device.join("tile0")).unwrap();

        // Card 0: AMD
        File::create(card0_device.join("uevent")).unwrap().write_all(b"DRIVER=amdgpu\n").unwrap();
        File::create(card0_device.join("vendor")).unwrap().write_all(b"0x1002\n").unwrap();
        File::create(card0_device.join("product_name")).unwrap().write_all(b"AMD Radeon RX 6800 XT\n").unwrap();
        File::create(card0_device.join("mem_info_vram_total")).unwrap().write_all(b"17179869184\n").unwrap();
        File::create(card0_device.join("mem_info_vram_used")).unwrap().write_all(b"2147483648\n").unwrap();

        // Card 1: Intel
        File::create(card1_device.join("uevent")).unwrap().write_all(b"DRIVER=xe\n").unwrap();
        File::create(card1_device.join("vendor")).unwrap().write_all(b"0x8086\n").unwrap();
        File::create(card1_device.join("product_name")).unwrap().write_all(b"Intel Arc A580\n").unwrap();
        File::create(card1_device.join("tile0").join("vram0_total_bytes")).unwrap().write_all(b"8589934592\n").unwrap();
        File::create(card1_device.join("tile0").join("vram0_used_bytes")).unwrap().write_all(b"1073741824\n").unwrap();

        let amd_backend = AmdBackend::with_drm_path(temp_dir.clone());
        let intel_backend = IntelBackend::with_drm_path(temp_dir.clone());

        let amd_gpus = amd_backend.get_all_metrics();
        let intel_gpus = intel_backend.get_all_metrics();

        assert_eq!(amd_gpus.len(), 1);
        assert_eq!(amd_gpus[0].name, "AMD Radeon RX 6800 XT");
        assert_eq!(amd_gpus[0].vram_total, 17179869184);

        assert_eq!(intel_gpus.len(), 1);
        assert_eq!(intel_gpus[0].name, "Intel Arc A580");
        assert_eq!(intel_gpus[0].vram_total, 8589934592);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
