# NeuraDex

<p align="center">
  <b>🇬🇧 English</b> | <a href="docs/FR/README.md">🇫🇷 Français</a>
</p>

<p align="center">
  <img src="data/icons/io.github.bazinfla.NeuraDex.svg" alt="NeuraDex Logo" width="128" height="128"/>
</p>

<p align="center">
  <strong>A native Linux desktop application for Ollama: model management, system monitoring, and local AI testing</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust-orange.svg?style=flat-square&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/GUI-GTK4%20%2F%20Libadwaita-blue.svg?style=flat-square&logo=gnome" alt="GTK4 / Libadwaita" />
  <img src="https://img.shields.io/badge/Platform-Linux-yellow.svg?style=flat-square&logo=linux" alt="Linux" />
  <img src="https://img.shields.io/badge/Backend-Ollama%20Native%20API-teal.svg?style=flat-square" alt="Ollama" />
  <img src="https://img.shields.io/badge/License-GPL--3.0-green.svg?style=flat-square" alt="License" />
</p>

---

## Overview

**NeuraDex** is a native Linux desktop application written in **Rust** using **GTK4 & Libadwaita**.

It provides a desktop interface to manage local **Ollama** runtimes: monitor GPU VRAM and system memory in real time, unload models on demand, configure daemon and network settings, explore models with memory requirement estimation, download GGUF files from Hugging Face, adjust inference parameters per model, and test prompts through a built-in chat interface.

<!-- <p align="center">
  <img src="docs/screenshots/home.png" alt="NeuraDex Home" width="100%">
</p> -->

---

## Key Features

### 🖥️ 1. Hardware Monitoring & Model Management
- **Multi-GPU & System Monitoring**: Per-GPU VRAM metrics (used, total, temperature, load, power draw) with aggregate multi-GPU calculations, alongside system RAM and CPU utilization.
- **Memory Footprint Estimation**: Estimates model memory requirements before loading:
  - 🟢 **100% GPU VRAM**: All model layers fit entirely within video memory.
  - 🟡 **Hybrid CPU/GPU**: Model layers are split between GPU VRAM and system RAM.
  - 🔴 **Insufficient Memory**: Warns when memory requirements exceed available system capacity.
- **Loaded Models & Memory Management**: Real-time cards showing models currently loaded in memory with VRAM/RAM allocation breakdowns, keep-alive expiration timers, and an **`🧹 Unload All`** button to clear loaded models.
- **Installed Model Library**: Overview of locally installed Ollama models with parameter counts, quantization levels, sizes, and capability tags (`think`, `vision`, `tools`, `code`, `embeddings`).
- **Model Preloading**: Load any model into GPU/system memory directly from the library card with visual progress feedback, preparing it for immediate inference.

<p align="center">
  <img src="docs/screenshots/home_showcase.webp" alt="Home Page - Hardware & Models" width="100%">
</p>

### ⚙️ 2. Model Settings & Customization
- **Synchronized Parameter Controls**: Adjust hyperparameters (`temperature`, `top_p`, `top_k`, `num_ctx`, `repeat_penalty`, `num_gpu`) with synchronized slider and spin-button inputs.
- **GPU Layer Offloading**: Detects model architecture layers (`block_count`) to set the number of layers offloaded to GPU VRAM, or force `num_gpu = 0` for CPU-only execution.
- **System Prompts & Model Variants**: Define persistent system instructions, customize prompt templates, and create new model entries in Ollama (`/api/create`) with saved configurations.

<p align="center">
  <img src="docs/screenshots/model_settings_showcase.webp" alt="Model Settings & Configuration Panel" width="100%">
</p>

### 📚 3. Model Hub & Hugging Face GGUF Downloader
- **Unified Smart Omnibar**: Compact search bar combining real-time catalog filtering, automatic Hugging Face link detection (`hf.co/...`, `author/model`) to explore GGUF quantizations, and direct Ollama model pulling.
- **Model Catalog**: Browse official and community models with metadata, available quantizations (`Q4_K_M`, `FP16`, `Q8_0`), context limits, and capability tags (`think`, `vision`, `tools`, `code`, etc.).
- **Hugging Face GGUF Picker**: Paste a Hugging Face repository URL or identifier (`author/model` or `hf.co/...`) to browse available GGUF quantizations with file sizes and memory compatibility checks.
- **Hardware Filter**: Filter the catalog to hide models exceeding local memory capacity.
- **Direct & Resilient Downloads**: Directly streams GGUF models from Hugging Face with HTTP Range resume support, avoiding Ollama daemon timeout issues.

> [!TIP]
> **Large Models (> 17 GB) & Hugging Face Token**:
> While public models can be downloaded anonymously, Hugging Face frequently throttles or times out unauthenticated connections on large files (> 17 GB). To ensure maximum download speeds and avoid rate limiting, it is strongly recommended to configure a free Hugging Face token (`hf_...`) in **Settings > API Profiles / Vault**.

<p align="center">
  <img src="docs/screenshots/hub_showcase.webp" alt="NeuraDex Model Hub - Exploration & Download" width="100%">
</p>

<p align="center">
  <img src="docs/screenshots/hf_showcase.webp" alt="Hub Page - Hugging Face Quantization Modal" width="100%">
</p>

### 🖧 4. Service Configuration, Storage & Keys
- **Daemon Supervision**: Check service status, start, stop, or restart the Ollama systemd service.
- **Ollama Runtime Options**: Configure Flash Attention (`OLLAMA_FLASH_ATTENTION`) and memory retention policies (`OLLAMA_KEEP_ALIVE`).
- **Model Storage Directory**: Configure `OLLAMA_MODELS` path with disk space indicators and a built-in migration tool with progress tracking.
- **Network Exposure Switcher**:
  - 🔒 **Localhost Only** (`127.0.0.1`) for local-only use.
  - 🏠 **Local Area Network (LAN)** (`0.0.0.0`) to share across the local network.
  - 🌐 **Exposed / Internet** (`0.0.0.0` with CORS policy `OLLAMA_ORIGINS=*`).
- **Credential Storage**: Store API keys using the FreeDesktop Secret Service (**GNOME Keyring** & **KDE KWallet**) with a local protected file fallback (`chmod 0600`), and apply keys to the systemd drop-in configuration.
- **SSH Member Keys**: Generate Ed25519 SSH key pairs (`~/.ollama/id_ed25519_{member}`) for Ollama accounts with clipboard copy support.
- **System Logs**: View real-time service logs via `journalctl -u ollama.service` with color tags, search filter, and log-level filtering.
- **Diagnostic Report**: Generate and inspect a Markdown-formatted environment report (OS, kernel, Wayland/X11 session, CPU, GPU VRAM/temps, Ollama configuration, loaded models) to easily troubleshoot issues or attach to bug reports.
- **Language & Updates**: Bilingual interface (English and French) with automatic system locale detection, and check for new releases via the GitHub API.

<p align="center">
  <img src="docs/screenshots/settings_showcase.webp" alt="Settings Page - Daemon Control & Security" width="100%">
</p>

### 🧪 5. Chat Interface for Model Testing
> [!NOTE]
> NeuraDex is primarily designed for model management and system monitoring. A built-in chat interface is included to quickly test prompts, inspect model outputs, and measure inference performance without third-party tools.

- **Testing Chat**: Asynchronous conversation interface with session history and real-time streaming.
- **Inference Metrics**: Real-time generation speed (tokens/second), Time To First Token (TTFT), and memory tracking during inference.
- **Temporary Settings Popover**: Adjust temperature, context window, and system prompts during testing without modifying global model settings.
- **Session Titles**: Automatic session title generation from the first user prompt (with French and English support).

<!-- <p align="center">
  <img src="docs/screenshots/chat.png" alt="Chat Page - Playground & Telemetry" width="100%">
</p> -->

---

## 🧩 Hardware Compatibility

| Platform | VRAM / Memory Gauges | Compute & Load | Power Draw | Temperature | Status |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **NVIDIA** | ✅ NVML | ✅ NVML | ✅ NVML | ✅ NVML | Tested & Functional |
| **AMD Radeon** | ✅ `amdgpu` sysfs | ✅ sysfs | ✅ `hwmon` | ✅ `hwmon` | Implemented (Community testing welcome) |
| **Intel Arc / iGPU** | ✅ `xe` / `i915` sysfs | ✅ sysfs | ✅ `hwmon` | ✅ `hwmon` | Implemented (Community testing welcome) |

> [!NOTE]
> **Community Feedback Needed (AMD & Intel GPUs)** : I no longer have physical access to AMD Radeon hardware nor Intel Arc hardware. The AMD and Intel monitoring backends have been built following official Linux kernel `sysfs`/`hwmon` specifications and verified with automated test suites, but have not yet been validated on real physical devices. **User feedback, logs, and issue reports from AMD and Intel users are very welcome!**

---

## 🚀 Installation

### Quick Install (One-liner)
For Debian, Ubuntu, Fedora, Arch Linux, and derivatives:
```bash
curl -fsSL https://raw.githubusercontent.com/BazinFla/Neuradex/main/scripts/get.sh | bash
```
*(Automatically detects your distribution, downloads the latest official `.deb`, `.rpm`, or `.pkg.tar.zst` package from GitHub Releases, and installs it).*

---

### Manual Package Installation

#### Fedora & derivatives (.rpm)
Download the latest `.rpm` from [Releases](https://github.com/BazinFla/Neuradex/releases) and install:
```bash
sudo dnf install ./neuradex-*.rpm
```

#### Debian / Ubuntu & derivatives (.deb)
Download the latest `.deb` from [Releases](https://github.com/BazinFla/Neuradex/releases) and install:
```bash
sudo apt install ./neuradex_*_amd64.deb
```

#### Arch Linux & derivatives (.pkg.tar.zst)
Download the latest `.pkg.tar.zst` from [Releases](https://github.com/BazinFla/Neuradex/releases) and install:
```bash
sudo pacman -U ./neuradex-*-x86_64.pkg.tar.zst
```
Or build locally with the included `PKGBUILD`:
```bash
makepkg -si
```

<details>
<summary><b>🔨 Build from Source</b></summary>

#### Prerequisites

##### Debian / Ubuntu / Pop!_OS
```bash
sudo apt update
sudo apt install -y build-essential libgtk-4-dev libadwaita-1-dev libssl-dev pkg-config
```

##### Fedora / RHEL
```bash
sudo dnf install -y gcc gtk4-devel libadwaita-devel openssl-devel pkgconf-pkg-config
```

##### Arch Linux / Manjaro
```bash
sudo pacman -S --needed base-devel gtk4 libadwaita openssl pkgconf
```

#### Compile and Install
```bash
git clone https://github.com/BazinFla/NeuraDex.git
cd NeuraDex/neuradex
cargo build --release
./scripts/install.sh
```
</details>

---

## 📄 License

Distributed under the **GPL-3.0 License**. See `LICENSE` for more information.

---

## 💡 Troubleshooting & Reset

To reset NeuraDex configuration and restore default Ollama service settings:
```bash
rm -rf ~/.config/neuradex
sudo rm -f /etc/systemd/system/ollama.service.d/override.conf
sudo systemctl daemon-reload
sudo systemctl restart ollama
```

