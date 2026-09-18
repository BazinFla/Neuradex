# Changelog — NeuraDex

All notable changes to the **NeuraDex** project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.8] — Dual-Source Model Resolution (Ollama & Hugging Face) & Hub Category Modernization

This release enhances the Model Hub Omnibar with simultaneous dual-source resolution across Ollama and Hugging Face, introduces a unified multi-source picker popup, and streamlines model capability categories.

### Added
- **Dual-Source Resolution & Unified Picker (`window.rs`, `HfModelPickerDialog`)**:
  - Concurrent lookup via `tokio::join!` querying both Ollama's registry and Hugging Face API when resolving user input (`author/model`).
  - When a model exists on both Ollama and Hugging Face, the selection popup displays the **🦙 Ollama Library** option prominently at the top, directly above the Hugging Face GGUF quantizations list, giving users the freedom to choose their preferred download source in one click.
  - Dedicated "Open on Ollama" action button linking directly to the model's ollama.com page.
  - Graceful fallback and user notification when a model is not found on either platform.
- **Hub Omnibar "Search" Action**:
  - Replaced the action button label from "Download" to "Search" (`🔍 Search` / `🔍 Rechercher`) with updated contextual tooltips to better reflect the dual-source discovery workflow.

### Changed
- **Hub Category Modernization**:
  - Renamed `Specialized` category to `Tools` across UI filters, card tags, and localization files (`en.json`, `fr.json`).
  - Removed obsolete `General & Versatile` filter pill from Hub category filters for a cleaner, more focused browsing experience.
  - Updated AI models scraper and dataset (`ai-models-list`) to normalize tool-calling capabilities.

---

## [0.1.7] — Unified Smart Omnibar & Arch Linux Packaging

This release introduces a streamlined, space-saving smart Omnibar in the Model Hub and native packaging for Arch Linux.

### Added
- **Unified Smart Omnibar (`HubView`)**:
  - Replaced redundant top input panels with a single, compact search bar combining catalog filtering and model downloading.
  - Automatic real-time identifier detection: typing or pasting Hugging Face URLs/repos (`hf.co/...`, `author/model`) dynamically reveals a contextual **"🤗 Explore GGUFs"** action.
  - Direct Ollama model download: typing custom or unlisted model identifiers reveals a **"⬇️ Download"** action.
  - Instant activation: pressing Enter or clicking the action button triggers GGUF inspection or model download, then resets the search view.
- **Universal Multi-Distro Web Installer (`scripts/get.sh`)**:
  - One-line installation script (`curl -fsSL .../get.sh | bash`) supporting Debian, Ubuntu, Fedora, and Arch Linux.
  - Automatic architecture detection (`x86_64`) and latest release package resolution via GitHub API.
- **Native Arch Linux Packaging**:
  - Integrated `.pkg.tar.zst` Pacman package generation in release CI workflow (`release.yml`).
  - Added official `PKGBUILD` and AUR-ready `PKGBUILD.bin` recipes.

---

## [0.1.6] — Ollama Modelfile Compilation & Storage Permissions Fix

This patch release fixes the HTTP 500 error encountered when saving model settings and compiling Modelfiles in Ollama.

### Fixed
- **Ollama Storage Directory Permissions (`lifecycle.rs`)**:
  - Prioritized `ollama:ollama` ownership for model directories across `migrate_models`, `apply_systemd_override`, and `fix_directory_permissions`.
  - Preserved full read/write/execute rights for desktop users through `775` group permissions and POSIX ACLs (`setfacl`).
  - Resolved Linux kernel `utimensat` / `os.Chtimes` `EPERM` ("operation not permitted") failure in the Ollama service when reusing weight blobs to compile model variants and persist custom parameters (`num_ctx`, `num_gpu`, etc.).
- **API Diagnostics & Error Reporting (`client.rs`)**:
  - Contextual interception of Ollama `chtimes: operation not permitted` errors in `create_model_stream`.
  - Provides clear diagnostic guidance on storage ownership resolution rather than a generic HTTP 500 error toast.


## [0.1.5] — Diagnostic System & Release Preparation

This release introduces a system and environment diagnostic tool.

### Added
- **Diagnostic Tool & Environment Report (`diagnostic.rs`)**:
  - Asynchronous generation of a Markdown-formatted report.
  - Collected information:
    - **Application**: NeuraDex version, active language, daemon management mode.
    - **Operating System & Desktop**: Linux distribution, version, kernel, architecture, desktop environment (GNOME, KDE...), and session type (Wayland or X11).
    - **Hardware & Memory**: CPU model, thread count, total and used system RAM.
    - **Graphics & VRAM**: GPU detection (NVIDIA, AMD, Intel), allocated, used, and free VRAM, temperatures, and power draw (with CPU/RAM fallback support when no GPU is present).
    - **Ollama Service & Storage**: configured host, network exposure policy (CORS), models directory, Flash Attention status, and VRAM retention policy (*Keep Alive*).
    - **Connectivity & Models**: local daemon connectivity status and version, inventory of installed models with sizes in GiB, and list of models currently loaded in memory.
- **Graphical Integration in Settings View (`SettingsView`)**:
  - Dedicated *Diagnostic & System Report* group with system monitoring icon.
  - **"📋 Copy Report"** button with temporary visual feedback badge (*Copied!*).
  - **"Inspect"** button opening a Libadwaita modal dialog (`adw::Dialog`) displaying the complete report in a monospace text viewer with a quick copy button.
- **Diagnostic Shortcut in About Dialog (`Header`)**:
  - Added an action in the *About NeuraDex* dialog to copy system information directly to the clipboard.
- **Integration Test Suite (`config_and_vault_test.rs`)**:
  - Unit test `test_generate_diagnostic_report` verifying the generation of the various report sections.

---

## [0.1.4] — Hub Catalog & Hugging Face Integration

This release adds the **Hub** page to explore and download models, along with support for GGUF files directly from **Hugging Face**.

### Added
- **Hub View (`HubView`)**:
  - GTK4 / Libadwaita browsing interface (`adw::Clamp`, `FlowBox`, `ListBox`, `ScrolledWindow`).
  - Catalog of official and community models with metadata, sizes, and available quantizations.
- **Hugging Face Downloader (`hf_model_picker_dialog.rs`)**:
  - Support for Hugging Face URLs (`https://huggingface.co/...`, `hf.co/...`) and repository identifiers (`author/model_name`).
  - Remote repository tree exploration to list all available GGUF files.
  - Modal selector listing each quantization variant (`IQ3`, `Q4_K_M`, `Q5_K_M`, `Q6_K`, `Q8_0`, `FP16`) with its size in gigabytes.
  - Memory compatibility check: warning when a quantization exceeds available VRAM or RAM before starting the download.
  - Automated Ollama pull operation triggered via the `hf.co/...` protocol.
- **Filters & Thematic Categories (`category_box`)**:
  - Category filter pills: *Reasoning* (`reasoning`), *Vision*, *Code*, *Lightweight* (`lightweight`), *General*, *RAG*, *Cybersecurity*, and *Specialized*.
  - Automatic detection and display of model capability badges: `think`, `vision`, `audio`, `tools`, `code`, `embeddings`.
- **Hardware Compatibility Filter (`btn_compat_filter`)**:
  - Toggle to hide models that exceed available local memory (GPU VRAM + system RAM).
- **Search & Multi-criteria Sorting**:
  - Real-time search bar filtering by model name, author, tags, and descriptions.
  - Sorting dropdown: by popularity, alphabetical order, memory size, or date added.
- **Hub Model Cards (`HubModelCard`)**:
  - Display of creator and brand logos (Alibaba/Qwen, DeepSeek, Google Gemma, Meta Llama, Mistral, Microsoft Phi, NVIDIA, etc.) in SVG format.
  - Embedded quantization variant dropdown selector within each card.
  - Live download progress: dynamic progress bar, percentage, transfer rate (MB/s), downloaded / total volume, and step labels (blobs, SHA256 verification, finalization).
  - Cancel button to stop ongoing downloads.
- **Remote Synchronization (`btn_sync_online`)**:
  - Manual refresh button to update catalog metadata from remote repositories without restarting the application.

---

## [0.1.3] — Chat View for Model Testing

This release introduces a chat interface to test locally installed models and observe their inference performance.

### Added
- **Chat Interface (`ChatView`)**:
  - Asynchronous chat interface designed to interact with local or remote models.
  - Active model selection dropdown with memory allocation indicator (green = fully loaded in VRAM, yellow = split between RAM and VRAM).
- **Real-Time Token Streaming**:
  - Integration with Ollama's Server-Sent Events (SSE) stream (`/api/chat`).
  - Progressive token-by-token rendering without blocking the graphical user interface.
  - Auto-scrolling conversation view with dedicated user and assistant bubbles.
- **Inference Metrics (Information Badges)**:
  - **Generation speed**: real-time calculation and display of throughput in **tokens/second (tok/s)**.
  - **Initial latency (TTFT)**: measurement of *Time To First Token* in milliseconds.
  - **Token counter**: tally of prompt tokens and generated tokens.
  - **VRAM footprint**: tracking of allocated memory during inference.
- **Multi-session Sidebar (`ChatSidebar`, `ChatSession`)**:
  - Collapsible sidebar to optimize screen space.
  - Complete session management: create (`+`), switch, rename, and delete conversations.
  - **Automatic session title generation**: derives a concise title from the initial user prompt (stripping markdown, list formatting, and code blocks, supporting French and English).
- **Temporary Settings Popover (`ChatSettingsPopover`)**:
  - Popover menu accessible directly from the chat toolbar.
  - Adjust temperature (`temperature`) to tweak model creativity during testing.
  - Adjust context window size (`num_ctx`).
  - Temporary system prompt override (`system prompt`) to evaluate different behaviors or system instructions.
- **Inference Controls**:
  - Stop button (`Stop`) connected to an `AtomicBool` flag to interrupt ongoing generation and free GPU resources.
  - Keyboard shortcuts: send messages using `Ctrl+Enter` or `Enter`.
  - Reset button (`Clear chat`) to clear messages from the active session.

---

## [0.1.2] — Ollama Settings & System Logs

This release adds configuration settings for the Ollama service (lifecycle, networking, storage, API keys) and system log inspection.

### Added
- **Settings View (`SettingsView`)**:
  - Libadwaita interface built on `adw::PreferencesPage` and `adw::PreferencesGroup`.
  - Top save bar with automatic detection of unsaved changes (`is_dirty`).
- **Ollama Daemon Lifecycle Management (`lifecycle.rs`)**:
  - Detection of the systemd service state (`ollama.service`).
  - Control buttons: Start, Stop, and Restart the local service.
  - Support for systemd service mode or external user daemon mode.
  - Check for NeuraDex updates via GitHub Releases API.
  - Language selector (French / English) with system locale detection.
- **Network Exposure & CORS Configuration**:
  - Network exposure mode selection:
    - 🔒 **Localhost Only** (`127.0.0.1`) for strictly local usage.
    - 🏠 **Local Area Network (LAN)** (`0.0.0.0`) to share models across the local network.
    - 🌐 **Exposed / Internet** (`0.0.0.0` with CORS policy `OLLAMA_ORIGINS=*`).
  - Custom Ollama host configuration (`OLLAMA_HOST`) supporting non-standard ports and offline fallback when unreachable.
  - Option to enable Flash Attention (`OLLAMA_FLASH_ATTENTION`).
  - Memory retention policy selector (`OLLAMA_KEEP_ALIVE`).
- **Storage Management & Migration Tool**:
  - Custom model storage directory picker (`OLLAMA_MODELS`) using GTK4 file chooser (`FileDialog`).
  - Disk space indicators (free space, total capacity, and usage gauge).
  - Migration tool to move the models directory to another disk or folder with a dedicated progress bar.
  - Path validation (`validate_path_safe`) to protect privileged commands executed via `pkexec`.
- **API Keys & SSH Key Management (`vault.rs`, `secret_store.rs`)**:
  - API key profile manager (Ollama Cloud, OpenAI-compatible endpoints).
  - Integration with **FreeDesktop Secret Service** (GNOME Keyring / KDE KWallet) using pure Rust (`zbus`, without C library dependencies).
  - Fallback to protected local storage (`chmod 0600`) when D-Bus is unavailable, with visual indicators (`🔐 Keyring` vs `🔓 Local`).
  - Automatic migration on startup of plaintext tokens into the desktop keyring.
  - Application of keys to Ollama's systemd drop-in configuration.
  - Ollama SSH member keys management: Ed25519 key pair generation (`~/.ollama/id_ed25519_{member}`), clipboard copy, and link to web settings.
- **System Logs View (`LogsView`, `logs.rs`)**:
  - Live log streaming from the service via `journalctl -u ollama.service`.
  - Syntax highlighting using GTK tags (`TextTagTable`): red for errors, yellow for warnings, cyan for HTTP/GIN routes, green for success, and dimmed for debug.
  - Real-time text filter with search entry.
  - Log level dropdown filter: *All*, *GIN Requests*, *Warnings*, *Errors*.
  - Action buttons: pause/resume auto-scroll, clear log buffer, and copy all to clipboard.

---

## [0.1.1] — Model Settings & Configuration Persistence

This release allows configuring inference hyperparameters per model, adjusting GPU/CPU layer offloading, and persisting user preferences.

### Added
- **Model Settings Dialog (`ModelSettingsDialog`)**:
  - Libadwaita modal interface (`adw::Dialog`, `ViewSwitcher`, `Clamp`) with tabbed navigation.
- **Synchronized Parameter Controls (`bind_slider_and_spin`)**:
  - Synchronized dual controls pairing sliders (`Scale`) and numerical inputs (`SpinButton`):
  - **Sampling**: adjustment of `temperature`, `top_p`, `top_k`, and `min_p`.
  - **Context & Prediction**: context window size (`num_ctx` from 2K to 128K+) and prediction token limit (`num_predict`).
  - **Penalties**: repeat penalty (`repeat_penalty`), history window (`repeat_last_n`), presence penalty (`presence_penalty`), and frequency penalty (`frequency_penalty`).
  - **Execution & Reproducibility**: random seed (`seed`) and allocated CPU thread count (`num_thread`).
- **GPU Layer Offloading (`num_gpu`)**:
  - Automatic detection of model layer count (`total_layers` / `block_count`).
  - Selection of layer count to offload to GPU VRAM (`num_gpu`).
  - Option to set `num_gpu = 0` to force execution entirely on the CPU.
  - Memory-locking option (`use_mlock`) to limit swapping to disk.
- **Memory Footprint Estimation & Hardware Compatibility**:
  - Sticky side panel recalculating estimated memory requirements based on selected `num_ctx` and `num_gpu`.
  - Visual allocation indicators:
    - 🟢 **100% GPU VRAM**: fully loaded in video memory.
    - 🟡 **Hybrid CPU/GPU**: split between video memory and system RAM.
    - 🔴 **Insufficient Memory**: warning when requirements exceed available system memory.
- **Modelfile Editor & Variant Creation**:
  - Text editor to define persistent system instructions (`SYSTEM prompt`).
  - Prompt template editor (`TEMPLATE`).
  - Create customized model variants in Ollama (`/api/create`) incorporating user settings and prompts.
- **Configuration Persistence (`config.rs`)**:
  - Per-model custom configuration storage in `AppConfig` (`CustomModelSettings`).
  - Model key normalization (`normalize_model_keys`) to handle model variants with or without `:latest` tags consistently.
  - Atomic saves using a temporary file strategy followed by atomic rename (`write-to-tmp` + `rename`) to prevent file corruption.
  - Systematic application of `chmod 0600` file permissions on Unix/Linux.

---

## [0.1.0] — Initial Release: Home View & Hardware Monitoring

Initial release of NeuraDex, featuring real-time hardware monitoring and local Ollama model management.

### Added
- **Home View & Instance Dashboard (`InstancesView`)**:
  - Two-column layout built with GTK4 and Libadwaita.
- **Installed Models Management (Left Column)**:
  - Inventory and detection of models available on the local Ollama instance (`/api/tags`).
  - Model cards (`ModelCard`) displaying technical details: disk size, quantization level (`Q4_K_M`, `FP16`, `Q8_0`), parameter count (e.g. `7B`, `14B`), and architecture family.
  - Detection of model capabilities (`capabilities.rs`) with badges (`think`, `vision`, `tools`, `audio`, `code`, `embeddings`).
  - Vector logos for model creators and families (Mistral, Meta Llama, Google, DeepSeek, Qwen, Microsoft, etc.).
  - Quick model actions:
    - Preload into memory / VRAM with visual progress feedback (`ProgressBar`).
    - Shortcut to chat / testing view.
    - Direct access to model settings dialog.
    - Model deletion with confirmation dialog.
  - Installed model counter and empty state handling.
- **Hardware & VRAM Real-Time Monitoring (Right Column)**:
  - **Multi-GPU VRAM Gauge (`VramGauge`)**:
    - **NVIDIA** GPU monitoring via direct NVML wrapper (`nvml-wrapper`).
    - **AMD Radeon** GPU support (via sysfs / ROCm `amdgpu`).
    - **Intel GPU** support (via `sysfs`/`i915`/`xe`).
    - Per-device metrics: used vs total VRAM, percentage, temperature in °C, compute load (GPU Load %), and power consumption in Watts.
    - Aggregate VRAM calculation across multi-GPU environments.
    - Graceful CPU-only fallback when no GPU is detected: clear "CPU Only Mode" indicator using system RAM.
  - **System Telemetry (RAM & CPU)**:
    - GTK4 level bar (`LevelBar`) displaying system RAM usage (used / total in GB, percentage).
    - Continuous CPU utilization measurement (CPU %).
- **Loaded Models Manager (`running_box`)**:
  - Tracking of active models loaded in memory via `/api/ps`.
  - Memory allocation breakdown between GPU VRAM and system RAM.
  - Countdown timer before automatic model unloading (keep-alive expiration).
  - **`🧹 Unload All`** action button: unloads all models currently residing in memory.
- **Header Bar (`HeaderBar`) & Navigation**:
  - Connection status indicator with status badges (`🟢 Connected` / `🔴 Disconnected`).
  - Libadwaita view switcher (`ViewSwitcher`).
