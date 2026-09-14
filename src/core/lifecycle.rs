use crate::core::config::AppConfig;
use std::path::{Path, PathBuf};
use sysinfo::Disks;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceState {
    Active,
    Inactive,
    Failed,
    NotInstalled,
    Unknown(String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiskSpaceInfo {
    pub mount_point: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct MigrationProgress {
    pub step: String,
    pub fraction: Option<f64>,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub transferred_bytes: Option<u64>,
}

pub fn parse_rsync_progress_line(line: &str) -> Option<(u64, f64, String, String)> {
    let trimmed = line.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() >= 4 {
        let pct_idx = parts.iter().position(|p| p.ends_with('%'))?;
        if pct_idx > 0 && parts.len() > pct_idx + 2 {
            let bytes_str = parts[pct_idx - 1].replace([',', '.'], "");
            let pct_str = parts[pct_idx].trim_end_matches('%');
            if let (Ok(bytes), Ok(pct)) = (bytes_str.parse::<u64>(), pct_str.parse::<f64>()) {
                let speed = parts[pct_idx + 1].to_string();
                let eta = parts[pct_idx + 2].to_string();
                return Some((bytes, (pct / 100.0).clamp(0.0, 1.0), speed, eta));
            }
        }
    }
    None
}

pub struct LifecycleManager;

/// Characters that are unsafe in paths passed to privileged shell scripts via pkexec.
/// Prevents shell injection when paths are used as positional arguments in bash -c scripts.
const UNSAFE_PATH_CHARS: &[char] = &['$', '`', ';', '|', '&', '(', ')', '{', '}', '!', '\\', '\n', '\r', '\0'];

impl LifecycleManager {
    /// Validates that a path string does not contain shell metacharacters.
    /// Must be called before passing any user-controlled path to pkexec/bash scripts.
    fn validate_path_safe(path: &str, field_name: &str) -> Result<(), String> {
        if let Some(bad) = path.chars().find(|c| UNSAFE_PATH_CHARS.contains(c)) {
            return Err(format!(
                "Security: '{}' contains forbidden character {:?}. Only standard filesystem characters are allowed.",
                field_name, bad
            ));
        }
        Ok(())
    }

    /// Searches for the ollama binary in PATH or standard Linux directories
    pub fn find_ollama_binary() -> Option<PathBuf> {
        let candidates = [
            "/usr/bin/ollama",
            "/usr/local/bin/ollama",
            "/opt/ollama/ollama",
        ];

        for c in &candidates {
            let p = PathBuf::from(c);
            if p.is_file() {
                return Some(p);
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let user_bin = PathBuf::from(home).join(".local/bin/ollama");
            if user_bin.is_file() {
                return Some(user_bin);
            }
        }

        if let Ok(output) = std::process::Command::new("which").arg("ollama").output() {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    return Some(PathBuf::from(path_str));
                }
            }
        }

        None
    }

    /// Detects the status of the ollama systemd service
    pub async fn check_systemd_status() -> ServiceState {
        if Self::find_ollama_binary().is_none() {
            return ServiceState::NotInstalled;
        }

        // Try system-level systemctl first
        if let Ok(output) = Command::new("systemctl")
            .args(["is-active", "ollama"])
            .output()
            .await
        {
            let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
            match status.as_str() {
                "active" => return ServiceState::Active,
                "inactive" => return ServiceState::Inactive,
                "failed" => return ServiceState::Failed,
                other if !other.is_empty() => return ServiceState::Unknown(other.to_string()),
                _ => {}
            }
        }

        // Try systemctl --user next
        if let Ok(output) = Command::new("systemctl")
            .args(["--user", "is-active", "ollama"])
            .output()
            .await
        {
            let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
            match status.as_str() {
                "active" => return ServiceState::Active,
                "inactive" => return ServiceState::Inactive,
                "failed" => return ServiceState::Failed,
                other if !other.is_empty() => return ServiceState::Unknown(other.to_string()),
                _ => {}
            }
        }

        ServiceState::Inactive
    }

    /// Executes a systemctl action on the ollama service with fallback: system -> --user -> pkexec
    async fn run_systemctl_action(action: &str) -> Result<(), String> {
        // 1. Try system service first
        let output = Command::new("systemctl")
            .args([action, "ollama"])
            .output()
            .await
            .map_err(|e| format!("Failed to execute systemctl {}: {}", action, e))?;

        if output.status.success() {
            return Ok(());
        }

        // 2. Try user service next
        let user_output = Command::new("systemctl")
            .args(["--user", action, "ollama"])
            .output()
            .await
            .map_err(|e| format!("Failed to execute systemctl --user {}: {}", action, e))?;

        if user_output.status.success() {
            return Ok(());
        }

        // 3. Fallback to Polkit privilege elevation
        let pkexec_output = Command::new("pkexec")
            .args(["systemctl", action, "ollama"])
            .output()
            .await
            .map_err(|e| format!("Failed to execute pkexec systemctl {}: {}", action, e))?;

        if pkexec_output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&pkexec_output.stderr);
            Err(format!("Unable to {} Ollama service: {}", action, err.trim()))
        }
    }

    /// Starts the ollama service (via systemctl)
    pub async fn start_service() -> Result<(), String> {
        Self::run_systemctl_action("start").await
    }

    /// Stops the ollama service (via systemctl)
    pub async fn stop_service() -> Result<(), String> {
        Self::run_systemctl_action("stop").await
    }

    /// Restarts the ollama service
    pub async fn restart_service() -> Result<(), String> {
        Self::run_systemctl_action("restart").await
    }

    /// Retrieves disk space (total and available) for a given path
    pub fn get_disk_space_for_path(path: &Path) -> Option<DiskSpaceInfo> {
        let disks = Disks::new_with_refreshed_list();
        let target = if path.exists() {
            path.to_path_buf()
        } else {
            path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("/"))
        };

        let target_canonical = target.canonicalize().unwrap_or(target);

        let mut best_match: Option<DiskSpaceInfo> = None;
        let mut best_match_len = 0;

        for disk in disks.list() {
            let mount = disk.mount_point();
            if target_canonical.starts_with(mount) {
                let mount_len = mount.as_os_str().len();
                if mount_len >= best_match_len {
                    best_match_len = mount_len;
                    best_match = Some(DiskSpaceInfo {
                        mount_point: mount.to_path_buf(),
                        total_bytes: disk.total_space(),
                        available_bytes: disk.available_space(),
                    });
                }
            }
        }

        best_match
    }

    /// Extracts the OLLAMA_MODELS path currently active in the systemd override.conf configuration
    pub fn get_active_systemd_models_dir() -> Option<PathBuf> {
        let override_path = Path::new("/etc/systemd/system/ollama.service.d/override.conf");
        if let Ok(content) = std::fs::read_to_string(override_path) {
            for line in content.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("Environment=\"OLLAMA_MODELS=") {
                    let cleaned = val.trim_end_matches('"').trim();
                    if !cleaned.is_empty() {
                        return Some(PathBuf::from(cleaned));
                    }
                }
            }
        }

        if let Ok(output) = std::process::Command::new("systemctl")
            .args(["show", "ollama.service", "-p", "Environment"])
            .output()
        {
            let out_str = String::from_utf8_lossy(&output.stdout);
            for part in out_str.split_whitespace() {
                if let Some(val) = part.strip_prefix("OLLAMA_MODELS=") {
                    let cleaned = val.trim_matches('"').trim();
                    if !cleaned.is_empty() {
                        return Some(PathBuf::from(cleaned));
                    }
                }
            }
        }

        None
    }

    /// Recursively inspects an Ollama storage directory to count models and total size
    pub fn scan_models_dir_info(dir: &Path) -> Option<(usize, u64)> {
        // Special case: if it is /usr/share/ollama (often mode 0700 protected from standard users)
        if dir.starts_with("/usr/share/ollama") && Path::new("/usr/share/ollama").exists() {
            let active_custom = Self::get_active_systemd_models_dir();
            let is_running_on_usr_share = match active_custom.as_deref() {
                None => true,
                Some(p) => p.starts_with("/usr/share/ollama"),
            };

            if is_running_on_usr_share {
                if let Ok(output) = std::process::Command::new("curl")
                    .args(["-s", "--max-time", "1", "http://127.0.0.1:11434/api/tags"])
                    .output()
                {
                    if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        if let Some(models) = json_val.get("models").and_then(|m| m.as_array()) {
                            let count = models.len();
                            let total_size: u64 = models.iter().filter_map(|m| m.get("size").and_then(|s| s.as_u64())).sum();
                            if count > 0 {
                                return Some((count, total_size));
                            }
                        }
                    }
                }
            }
            return None;
        }

        if !dir.exists() {
            return None;
        }

        // Try standard or nested directory structures
        let manifests_dir = if dir.join("manifests").is_dir() {
            dir.join("manifests")
        } else if dir.join("models/manifests").is_dir() {
            dir.join("models/manifests")
        } else {
            dir.join("manifests")
        };

        let blobs_dir = if dir.join("blobs").is_dir() {
            dir.join("blobs")
        } else if dir.join("models/blobs").is_dir() {
            dir.join("models/blobs")
        } else {
            dir.join("blobs")
        };

        let mut model_count = 0;
        let mut total_size = 0u64;

        if manifests_dir.is_dir() {
            fn count_manifests_recursive(p: &Path, count: &mut usize) {
                if let Ok(rd) = std::fs::read_dir(p) {
                    for entry in rd.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            count_manifests_recursive(&path, count);
                        } else if path.is_file() {
                            *count += 1;
                        }
                    }
                }
            }
            count_manifests_recursive(&manifests_dir, &mut model_count);
        }

        if blobs_dir.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&blobs_dir) {
                for entry in rd.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        total_size += meta.len();
                    }
                }
            }
        }

        if model_count > 0 || total_size > 0 {
            return Some((model_count, total_size));
        }

        None
    }

    /// Detects if models are present in standard directories or the previous folder
    pub fn detect_default_models_store(previous_dir: Option<&Path>, custom_dir: Option<&Path>) -> Option<(PathBuf, usize, u64)> {
        let Some(target) = custom_dir else {
            // If no target folder is configured (default mode), no migration needs to be proposed
            return None;
        };

        let mut candidates = Vec::new();

        // 1. Previously configured folder
        if let Some(prev) = previous_dir {
            candidates.push(prev.to_path_buf());
        }

        // 2. Folder currently configured in the systemd service
        if let Some(sysd_dir) = Self::get_active_systemd_models_dir() {
            candidates.push(sysd_dir);
        }

        // 3. Standard system locations (/usr/share/ollama, /var/lib/ollama)
        if Path::new("/usr/share/ollama").exists() {
            candidates.push(PathBuf::from("/usr/share/ollama/.ollama/models"));
            candidates.push(PathBuf::from("/usr/share/ollama/.ollama"));
            candidates.push(PathBuf::from("/usr/share/ollama"));
        }
        candidates.push(PathBuf::from("/var/lib/ollama/models"));

        // 4. Standard user locations (~/.ollama/models)
        if let Ok(home) = std::env::var("HOME") {
            let home_p = Path::new(&home);
            candidates.push(home_p.join(".ollama/models"));
            candidates.push(home_p.join(".ollama"));
        }

        let mut seen = std::collections::HashSet::new();

        for candidate in candidates {
            if !seen.insert(candidate.clone()) {
                continue;
            }

            // Ignore if the candidate source matches the destination target (or subfolder)
            if candidate == target
                || candidate == target.join("models")
                || target == candidate.join("models")
                || (target.starts_with("/usr/share/ollama") && candidate.starts_with("/usr/share/ollama"))
            {
                continue;
            }

            if let Some((count, size)) = Self::scan_models_dir_info(&candidate) {
                if count > 0 || size > 0 {
                    return Some((candidate, count, size));
                }
            }
        }

        None
    }

    /// Moves or synchronizes models from the old folder to the new folder via pkexec
    /// Protected against I/O freezes and system crashes with real-time progress reporting
    pub async fn migrate_models(
        source: &Path,
        destination: &Path,
        progress_tx: Option<async_channel::Sender<MigrationProgress>>,
    ) -> Result<String, String> {
        if source == destination {
            return Err(crate::t!("settings.mig_err_same_dir"));
        }

        // If the source directory is protected by system permissions (e.g. /usr/share/ollama in 0700 mode),
        // the standard user lacks traversal permissions and source.exists() returns false.
        // We delegate the exact existence check to pkexec (root).
        let is_sys_protected = source.starts_with("/usr/share/ollama")
            || source.starts_with("/var/lib/ollama")
            || source.starts_with("/root")
            || (Path::new("/usr/share/ollama").exists() && source.to_string_lossy().contains("ollama"));

        if !source.exists() && !is_sys_protected {
            return Err(crate::t!("settings.mig_err_source_not_found", src = source.display()));
        }

        // Pre-flight check of available disk space on target partition
        if let Some((_count, needed_bytes)) = Self::scan_models_dir_info(source) {
            if needed_bytes > 0 {
                if let Some(dst_space) = Self::get_disk_space_for_path(destination) {
                    let is_cross_device = match (std::fs::metadata(source), std::fs::metadata(destination)) {
                        (Ok(m_src), Ok(m_dst)) => {
                            use std::os::unix::fs::MetadataExt;
                            m_src.dev() != m_dst.dev()
                        }
                        _ => true,
                    };
                    if is_cross_device {
                        let required_bytes = needed_bytes.saturating_add(1024 * 1024 * 1024); // 1 GB safety margin
                        if dst_space.available_bytes < required_bytes {
                            return Err(crate::t!(
                                "settings.mig_err_disk_space",
                                dst = destination.display(),
                                req = crate::core::hardware::estimator::format_gib(required_bytes),
                                avail = crate::core::hardware::estimator::format_gib(dst_space.available_bytes)
                            ));
                        }
                    }
                }
            }
        }

        let src_str = source.to_string_lossy().to_string();
        let dst_str = destination.to_string_lossy().to_string();
        let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());

        // Reject paths containing shell metacharacters before passing to pkexec
        Self::validate_path_safe(&src_str, "source")?;
        Self::validate_path_safe(&dst_str, "destination")?;
        Self::validate_path_safe(&user, "USER")?;

        let script = r#"set -e
export LC_ALL=C
SRC="$1"
DST="$2"
USER_NAME="$3"

echo "PROGRESS:STEP:STOPPING_OLLAMA"
WAS_ACTIVE=0
if systemctl is-active --quiet ollama 2>/dev/null; then
    WAS_ACTIVE=1
    systemctl stop ollama 2>/dev/null || true
fi

# Verify exact source folder
SRC_DIR="$SRC"
if [ ! -d "$SRC_DIR" ]; then
    echo "ERROR: Source directory '$SRC' not found on disk." >&2
    exit 1
fi

if [ ! -d "$SRC_DIR/manifests" ] && [ -d "$SRC_DIR/models/manifests" ]; then
    SRC_DIR="$SRC_DIR/models"
fi

mkdir -p "$DST"

shopt -s dotglob nullglob
FILES=("$SRC_DIR"/*)

if [ ${#FILES[@]} -gt 0 ]; then
    DEV_SRC=$(stat -c '%d' "$SRC_DIR" 2>/dev/null || stat -c '%d' "$SRC")
    DEV_DST=$(stat -c '%d' "$DST")

    if [ "$DEV_SRC" = "$DEV_DST" ]; then
        echo "PROGRESS:STEP:INSTANT_MOVE"
        mv "$SRC_DIR"/* "$DST"/
    else
        echo "PROGRESS:STEP:SECURE_COPY"
        IONICE=""
        if command -v ionice >/dev/null 2>&1; then
            if ionice -c 3 true 2>/dev/null; then
                IONICE="ionice -c 3"
            elif ionice -c 2 -n 7 true 2>/dev/null; then
                IONICE="ionice -c 2 -n 7"
            fi
        fi

        # Dynamically detect target drive type to throttle transfer rate appropriately
        BWLIMIT="250M"
        TARGET_DEV=$(findmnt -no SOURCE -T "$DST" 2>/dev/null || df --output=source "$DST" 2>/dev/null | tail -n 1)
        PARENT_DEV=$(lsblk -no PKNAME "$TARGET_DEV" 2>/dev/null || true)
        [ -z "$PARENT_DEV" ] && PARENT_DEV=$(basename "$TARGET_DEV" 2>/dev/null || true)

        ROTATIONAL=$(cat "/sys/block/$PARENT_DEV/queue/rotational" 2>/dev/null || echo 0)
        if [ "$ROTATIONAL" = "1" ]; then
            # Mechanical hard drive (HDD): 60 MB/s to prevent disk thrashing and RAM saturation
            BWLIMIT="60M"
        elif [[ "$PARENT_DEV" == sd* ]]; then
            # SATA SSD or USB storage: 160 MB/s (sustained TLC write without cache exhaustion)
            BWLIMIT="160M"
        else
            # High-performance NVMe / PCIe SSD: 250 MB/s
            BWLIMIT="250M"
        fi

        # Throttled transfer tailored to target storage medium
        # --partial enables seamless resume if interrupted.
        nice -n 19 $IONICE rsync -a --sparse --partial --bwlimit="$BWLIMIT" --info=progress2 "$SRC_DIR"/ "$DST"/

        # Clean up old source only after rsync succeeds completely and ensure path is safe
        if [ -n "$SRC_DIR" ] && [ "$SRC_DIR" != "/" ] && [ "$SRC_DIR" != "/usr" ] && [ "$SRC_DIR" != "/var" ] && [ "$SRC_DIR" != "/home" ]; then
            rm -rf "$SRC_DIR"/*
        fi
    fi
fi

echo "PROGRESS:STEP:PERMISSIONS"
(chown -R ollama:ollama "$DST" 2>/dev/null || chown -R "$USER_NAME":ollama "$DST" 2>/dev/null || chown -R "$USER_NAME":"$USER_NAME" "$DST" 2>/dev/null) || true
chmod -R 775 "$DST" 2>/dev/null || true
if command -v setfacl >/dev/null 2>&1; then
    setfacl -R -m u:"$USER_NAME":rwx,g:ollama:rwx,d:u:"$USER_NAME":rwx,d:g:ollama:rwx,o::rx "$DST" 2>/dev/null || true
fi

# Ensure all parent path elements are traversable (+x) by the ollama user
PARENT="$(dirname "$DST")"
while [ -n "$PARENT" ] && [ "$PARENT" != "/" ] && [ "$PARENT" != "." ]; do
    setfacl -m u:ollama:rx "$PARENT" 2>/dev/null || chmod a+x "$PARENT" 2>/dev/null || true
    PARENT="$(dirname "$PARENT")"
done

if [ "$WAS_ACTIVE" -eq 1 ]; then
    echo "PROGRESS:STEP:RESTARTING_OLLAMA"
    systemctl start ollama 2>/dev/null || true
fi

echo "PROGRESS:STEP:DONE"
"#;

        let mut child = Command::new("pkexec")
            .args(["bash", "-c", script, "--", &src_str, &dst_str, &user])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| crate::t!("settings.mig_err_privileges", err = e))?;


        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let progress_sender = progress_tx.clone();
        let stdout_handle = tokio::spawn(async move {
            let mut captured = String::new();
            if let Some(mut reader) = stdout {
                use tokio::io::AsyncReadExt;
                let mut buf = [0u8; 1024];
                let mut line_buf = Vec::new();
                let mut last_send = std::time::Instant::now();
                while let Ok(n) = reader.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    for &b in &buf[..n] {
                        if b == b'\n' || b == b'\r' {
                            if !line_buf.is_empty() {
                                if let Ok(line_str) = std::str::from_utf8(&line_buf) {
                                    let trimmed = line_str.trim();
                                    if !trimmed.is_empty() {
                                        captured.push_str(trimmed);
                                        captured.push('\n');

                                        if let Some(ref tx) = progress_sender {
                                            if let Some(step_code) = trimmed.strip_prefix("PROGRESS:STEP:") {
                                                let (step_text, frac) = match step_code {
                                                    "STOPPING_OLLAMA" => (crate::t!("settings.mig_step_stopping"), None),
                                                    "INSTANT_MOVE" => (crate::t!("settings.mig_step_instant"), Some(1.0)),
                                                    "SECURE_COPY" => (crate::t!("settings.mig_step_copying"), None),
                                                    "PERMISSIONS" => (crate::t!("settings.mig_step_permissions"), None),
                                                    "RESTARTING_OLLAMA" => (crate::t!("settings.mig_step_restarting"), None),
                                                    "DONE" => (crate::t!("settings.mig_step_done"), Some(1.0)),
                                                    other => (other.to_string(), None),
                                                };
                                                let _ = tx.send(MigrationProgress {
                                                    step: step_text,
                                                    fraction: frac,
                                                    speed: None,
                                                    eta: None,
                                                    transferred_bytes: None,
                                                }).await;
                                            } else if let Some((bytes, frac, speed, eta)) = parse_rsync_progress_line(trimmed) {
                                                // Throttle progress updates to avoid flooding the GTK event loop
                                                if last_send.elapsed() >= std::time::Duration::from_millis(80) || frac >= 0.999 {
                                                    last_send = std::time::Instant::now();
                                                    let _ = tx.send(MigrationProgress {
                                                        step: crate::t!("settings.mig_step_copying_files"),
                                                        fraction: Some(frac),
                                                        speed: Some(speed),
                                                        eta: Some(eta),
                                                        transferred_bytes: Some(bytes),
                                                    }).await;
                                                }
                                            }
                                        }
                                    }
                                }
                                line_buf.clear();
                            }
                        } else {
                            line_buf.push(b);
                        }
                    }
                }
            }
            captured
        });

        let stderr_handle = tokio::spawn(async move {
            let mut err_str = String::new();
            if let Some(mut reader) = stderr {
                use tokio::io::AsyncReadExt;
                let mut buf = Vec::new();
                if reader.read_to_end(&mut buf).await.is_ok() {
                    err_str = String::from_utf8_lossy(&buf).to_string();
                }
            }
            err_str
        });

        let status = child.wait().await.map_err(|e| crate::t!("settings.mig_err_process_wait", err = e))?;
        let _out = stdout_handle.await.unwrap_or_default();
        let err = stderr_handle.await.unwrap_or_default();

        if status.success() {
            if let Some(ref tx) = progress_tx {
                let _ = tx.send(MigrationProgress {
                    step: crate::t!("settings.mig_step_done"),
                    fraction: Some(1.0),
                    speed: None,
                    eta: None,
                    transferred_bytes: None,
                }).await;
            }
            Ok(crate::t!("settings.mig_success_body", dst = dst_str))
        } else {
            let full_err = err.trim();
            Err(if full_err.is_empty() {
                crate::t!("settings.mig_error_interrupted")
            } else {
                full_err.to_string()
            })
        }
    }

    /// Generates systemd drop-in override.conf content to configure OLLAMA_MODELS and other settings
    pub fn generate_systemd_override(config: &AppConfig) -> String {
        let mut content = String::from("[Service]\n");

        if let Some(ref dir) = config.models_directory {
            if !dir.trim().is_empty() {
                content.push_str(&format!("Environment=\"OLLAMA_MODELS={}\"\n", dir.trim()));
            }
        }

        if !config.ollama_host.trim().is_empty() && config.ollama_host != "127.0.0.1:11434" {
            content.push_str(&format!("Environment=\"OLLAMA_HOST={}\"\n", config.ollama_host.trim()));
        } else if config.exposure_mode == crate::core::config::ExposureMode::Lan || config.exposure_mode == crate::core::config::ExposureMode::Exposed {
            content.push_str("Environment=\"OLLAMA_HOST=0.0.0.0:11434\"\n");
        }

        if let Some(ref origins) = config.ollama_origins {
            if !origins.trim().is_empty() {
                content.push_str(&format!("Environment=\"OLLAMA_ORIGINS={}\"\n", origins.trim()));
            }
        } else if config.exposure_mode == crate::core::config::ExposureMode::Exposed {
            content.push_str("Environment=\"OLLAMA_ORIGINS=*\"\n");
        }

        if let Some(num) = config.num_parallel {
            content.push_str(&format!("Environment=\"OLLAMA_NUM_PARALLEL={}\"\n", num));
        }

        if config.flash_attention {
            content.push_str("Environment=\"OLLAMA_FLASH_ATTENTION=1\"\n");
        }

        if let Some(ref ka) = config.keep_alive {
            if !ka.trim().is_empty() {
                content.push_str(&format!("Environment=\"OLLAMA_KEEP_ALIVE={}\"\n", ka.trim()));
            }
        }

        // Inject Ollama Cloud API key or active profile
        let cloud_token = config
            .active_profile()
            .and_then(|p| {
                if p.provider_type != crate::core::config::ApiProviderType::OllamaSsh {
                    let tok = crate::core::secret_store::SecretStore::resolve_token(p);
                    if !tok.trim().is_empty() {
                        Some(tok.trim().to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .or_else(|| {
                config.profiles.iter().find_map(|p| {
                    if p.provider_type == crate::core::config::ApiProviderType::OllamaCloud {
                        let tok = crate::core::secret_store::SecretStore::resolve_token(p);
                        if !tok.trim().is_empty() {
                            Some(tok.trim().to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            });

        if let Some(token) = cloud_token {
            content.push_str(&format!("Environment=\"OLLAMA_API_KEY={}\"\n", token));
        }

        content
    }

    /// Writes systemd drop-in file via pkexec, synchronizes active SSH key for the ollama user, and reloads systemd
    pub async fn apply_systemd_override(config: &AppConfig) -> Result<String, String> {
        let override_content = Self::generate_systemd_override(config);
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("neuradex_override.conf");

        std::fs::write(&temp_file, &override_content)
            .map_err(|e| format!("Unable to write temporary file: {}", e))?;

        let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
        let mut priv_key_str = String::new();
        let mut pub_key_str = String::new();
        if let Some(active_prof) = config.active_profile() {
            if active_prof.provider_type == crate::core::config::ApiProviderType::OllamaSsh {
                let priv_key_path = std::path::PathBuf::from(active_prof.token.trim());
                let pub_key_path = std::path::PathBuf::from(format!("{}.pub", active_prof.token.trim()));
                if priv_key_path.is_file() && pub_key_path.is_file() {
                    priv_key_str = priv_key_path.to_string_lossy().to_string();
                    pub_key_str = pub_key_path.to_string_lossy().to_string();
                }
            }
        }

        let models_dir_str = config.models_directory.as_deref().unwrap_or("").trim().to_string();
        let temp_file_str = temp_file.to_string_lossy().to_string();

        // Reject paths containing shell metacharacters before passing to pkexec
        if !models_dir_str.is_empty() {
            Self::validate_path_safe(&models_dir_str, "models_directory")?;
        }
        if !priv_key_str.is_empty() {
            Self::validate_path_safe(&priv_key_str, "ssh_private_key")?;
        }
        if !pub_key_str.is_empty() {
            Self::validate_path_safe(&pub_key_str, "ssh_public_key")?;
        }
        Self::validate_path_safe(&user, "USER")?;

        let script = r#"set -e
priv_key="$1"
pub_key="$2"
models_dir="$3"
user_name="$4"
temp_file="$5"

if [ -n "$priv_key" ] && [ -f "$priv_key" ] && [ -n "$pub_key" ] && [ -f "$pub_key" ]; then
    mkdir -p /usr/share/ollama/.ollama
    cp "$priv_key" /usr/share/ollama/.ollama/id_ed25519
    cp "$pub_key" /usr/share/ollama/.ollama/id_ed25519.pub
    chown -R ollama:ollama /usr/share/ollama/.ollama 2>/dev/null || true
    chmod 700 /usr/share/ollama/.ollama
    chmod 600 /usr/share/ollama/.ollama/id_ed25519
fi

if [ -n "$models_dir" ]; then
    mkdir -p "$models_dir"
    (chown -R ollama:ollama "$models_dir" 2>/dev/null || chown -R "$user_name":ollama "$models_dir" 2>/dev/null || chown -R "$user_name":"$user_name" "$models_dir" 2>/dev/null || true)
    chmod -R 775 "$models_dir" 2>/dev/null || true
    if command -v setfacl >/dev/null 2>&1; then
        setfacl -R -m u:"$user_name":rwx,g:ollama:rwx,d:u:"$user_name":rwx,d:g:ollama:rwx,o::rx "$models_dir" 2>/dev/null || true
    fi

    # Ensure all parent path elements are traversable (+x) by the ollama user
    parent="$(dirname "$models_dir")"
    while [ -n "$parent" ] && [ "$parent" != "/" ] && [ "$parent" != "." ]; do
        setfacl -m u:ollama:rx "$parent" 2>/dev/null || chmod a+x "$parent" 2>/dev/null || true
        parent="$(dirname "$parent")"
    done
fi

mkdir -p /etc/systemd/system/ollama.service.d
cp "$temp_file" /etc/systemd/system/ollama.service.d/override.conf
systemctl daemon-reload
systemctl restart ollama
"#;

        let res = Command::new("pkexec")
            .args([
                "bash",
                "-c",
                script,
                "--",
                &priv_key_str,
                &pub_key_str,
                &models_dir_str,
                &user,
                &temp_file_str,
            ])
            .output()
            .await
            .map_err(|e| format!("Privileged execution failed: {}", e))?;

        let _ = std::fs::remove_file(temp_file);

        if res.status.success() {
            Ok(crate::t!("settings.systemd_applied_success"))
        } else {
            let err = String::from_utf8_lossy(&res.stderr);
            Err(crate::t!("settings.systemd_apply_failed", err = err.trim().to_string()))
        }
    }

    /// Checks if system user 'ollama' has access rights to the directory (non-blocking POSIX check)
    pub fn check_directory_permissions_for_ollama(path: &Path) -> Result<(), String> {
        let ollama_uid = std::process::Command::new("id")
            .arg("-u")
            .arg("ollama")
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    String::from_utf8_lossy(&out.stdout).trim().parse::<u32>().ok()
                } else {
                    None
                }
            });

        // If system user 'ollama' does not exist on this machine, no specific restrictions apply
        let Some(uid_ollama) = ollama_uid else {
            return Ok(());
        };

        let ollama_gid = std::process::Command::new("id")
            .arg("-g")
            .arg("ollama")
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    String::from_utf8_lossy(&out.stdout).trim().parse::<u32>().ok()
                } else {
                    None
                }
            });

        // POSIX verification via metadata (without blocking sudo call)
        if let Ok(meta) = std::fs::metadata(path) {
            use std::os::unix::fs::MetadataExt;
            let mode = meta.mode();
            let dir_uid = meta.uid();
            let dir_gid = meta.gid();

            let user_write_exec = (mode & 0o300) == 0o300;
            let group_write_exec = (mode & 0o030) == 0o030;
            let other_write_exec = (mode & 0o003) == 0o003;

            // 1. If everyone has write and traversal permissions (e.g. rwxrwxrwx or 777/775)
            if other_write_exec {
                return Ok(());
            }

            // 2. If directory is directly owned by user 'ollama'
            if dir_uid == uid_ollama && user_write_exec {
                return Ok(());
            }

            // 3. If directory is owned by group 'ollama'
            if let Some(gid_ollama) = ollama_gid {
                if dir_gid == gid_ollama && group_write_exec {
                    return Ok(());
                }
            }

            return Err(crate::t!(
                "settings.perm_not_writable",
                path = path.display().to_string(),
                uid = dir_uid.to_string(),
                gid = dir_gid.to_string(),
                mode = format!("{:o}", mode & 0o777)
            ));
        }

        Ok(())
    }

    /// Automatically fixes directory permissions for user 'ollama' via pkexec
    pub async fn fix_directory_permissions(path: &Path) -> Result<String, String> {
        let path_str = path.to_string_lossy().to_string();
        let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());

        // Reject paths containing shell metacharacters before passing to pkexec
        Self::validate_path_safe(&path_str, "directory")?;
        Self::validate_path_safe(&user, "USER")?;
        let script = r#"set -e
dir="$1"
user_name="$2"
mkdir -p "$dir"
(chown -R ollama:ollama "$dir" 2>/dev/null || chown -R "$user_name":ollama "$dir" 2>/dev/null || chown -R "$user_name":"$user_name" "$dir" 2>/dev/null || true)
chmod -R 775 "$dir"
if command -v setfacl >/dev/null 2>&1; then
    setfacl -R -m "u:$user_name:rwx,g:ollama:rwx,d:u:$user_name:rwx,d:g:ollama:rwx,o::rx" "$dir" 2>/dev/null || true
fi
"#;
        let res = Command::new("pkexec")
            .args(["bash", "-c", script, "--", &path_str, &user])
            .output()
            .await
            .map_err(|e| e.to_string())?;

        if res.status.success() {
            Ok(crate::t!(
                "settings.perm_success_body",
                user = user,
                path = path_str
            ))
        } else {
            let err = String::from_utf8_lossy(&res.stderr);
            Err(err.trim().to_string())
        }
    }
}



