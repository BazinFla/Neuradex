# Contributing to NeuraDex

Thank you for your interest in contributing to **NeuraDex**! 🎉

NeuraDex is an open-source, native Linux desktop application built with **Rust** and **GTK4 / Libadwaita**, providing a complete control cockpit, hardware/VRAM monitor, model hub, and inference studio for local AI (Ollama).

We welcome all kinds of contributions: code improvements, bug reports, documentation, translations, packaging, and hardware telemetry testing.

---

## 🧭 Table of Contents

1. [Where We Need the Most Help](#-where-we-need-the-most-help)
2. [Development Environment Setup](#-development-environment-setup)
3. [Running and Testing Locally](#-running-and-testing-locally)
   - [Normal Execution](#1-normal-execution)
   - [Testing Without a Dedicated GPU](#2-testing-without-a-dedicated-gpu-cpu-only-mode)
   - [Testing with Offline / Custom Ollama Daemon](#3-testing-with-offline--custom-ollama-daemon)
4. [Quality Standards & Guidelines](#-quality-standards--guidelines)
   - [Code Formatting](#code-formatting)
   - [Clippy (Strict Zero-Warning Policy)](#clippy-strict-zero-warning-policy)
   - [Automated Tests](#automated-tests)
   - [Security Audit](#security-audit)
5. [Localization (i18n)](#-localization-i18n)
6. [Git Workflow & Commit Guidelines](#-git-workflow--commit-guidelines)
7. [Submitting a Pull Request](#-submitting-a-pull-request)

---

## 🎯 Where We Need the Most Help

- **Hardware Validation (AMD & Intel GPUs)**:
  The hardware telemetry backends for AMD Radeon (`amdgpu` sysfs / `rocm-smi`) and Intel Arc / iGPUs (`xe` / `i915` sysfs) have been implemented and unit-tested, but real-world feedback on diverse Linux hardware is invaluable. If you have AMD or Intel GPUs, test logs and compatibility feedback are immensely appreciated!
- **Packaging & Distribution**:
  Flatpak / Flathub manifest integration, AppStream metadata (`metainfo.xml`), Arch AUR packages, and Fedora Copr repos.
- **UI/UX Polish**:
  Adhering strictly to modern [GNOME Human Interface Guidelines (HIG)](https://developer.gnome.org/hig/) using Libadwaita patterns.
- **Translations**:
  Expanding translation coverage beyond English and French (see `locales/`).

---

## 🛠️ Development Environment Setup

### 1. System Dependencies

#### Debian / Ubuntu / Pop!_OS
```bash
sudo apt update
sudo apt install -y build-essential libgtk-4-dev libadwaita-1-dev libssl-dev pkg-config
```

#### Fedora / RHEL
```bash
sudo dnf install -y gcc gtk4-devel libadwaita-devel openssl-devel pkgconf-pkg-config
```

#### Arch Linux / Manjaro
```bash
sudo pacman -S --needed base-devel gtk4 libadwaita openssl pkgconf
```

### 2. Rust Toolchain

NeuraDex requires a modern Rust toolchain (Rust 2021 Edition, Rust >= 1.78 recommended):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
rustup component add rustfmt clippy
```

### 3. Local Ollama Runtime (Optional but Recommended)

NeuraDex interacts natively with Ollama. Make sure Ollama is installed and running locally on `http://127.0.0.1:11434`, or use the mock/offline environment variables described below.

---

## 🧪 Running and Testing Locally

### 1. Normal Execution
```bash
cargo run
```

### 2. Testing Without a Dedicated GPU (CPU-Only Mode)
You can simulate a machine without a dedicated GPU even on an NVIDIA/AMD workstation:
```bash
# Test NVIDIA GPU bypass via standard CUDA visibility
CUDA_VISIBLE_DEVICES="" cargo run

# Universal bypass for all GPU vendors (NVIDIA, AMD, Intel)
NEURADEX_DISABLE_GPU=1 cargo run
```
*Expected behavior*: NeuraDex should gracefully fallback to 100% CPU mode with 0 MB VRAM allocation without crashing or hanging.

### 3. Testing with Offline / Custom Ollama Daemon
You can test how NeuraDex handles network disconnections and errors without shutting down your actual Ollama daemon:
```bash
# Point to an unreachable mock port
OLLAMA_HOST="127.0.0.1:9999" cargo run
```
*Expected behavior*: The top header displays a red `🔴 Déconnecté` badge, network actions show clean error toasts/banners, and no panic occurs.

---

## 📏 Quality Standards & Guidelines

Before submitting any Pull Request, please ensure all checks pass cleanly.

### Code Formatting
Ensure your code matches the official Rust style:
```bash
cargo fmt --all -- --check
```
To automatically apply formatting:
```bash
cargo fmt --all
```

### Clippy (Strict Zero-Warning Policy)
NeuraDex maintains a **zero-warning policy** on Clippy. All warnings are treated as errors in CI:
```bash
cargo clippy --all-targets -- -D warnings
```
Key patterns to observe:
- Avoid unnecessary `&t!(...)` dereferences in UI labels (use `impl AsRef<str>`).
- Keep struct/function signatures clean and modular (use parameter structs like `CustomModelfileParams` or `DirtyCheckWidgets` rather than functions with 8+ arguments).
- Ensure new structs implement `Default` when providing `new()`.

### Automated Tests
Run the entire test suite (unit and integration tests):
```bash
cargo test
```
Or target specific regression suites:
```bash
cargo test --test api_and_hw_test
```

### Security Audit
Check dependencies for known CVEs:
```bash
cargo install cargo-audit --locked
cargo audit
```

---

## 🌍 Localization (i18n)

NeuraDex supports multiple languages located in the `locales/` directory:
- `locales/en.json` (English - default)
- `locales/fr.json` (French)

When adding or modifying user-facing strings in the UI:
1. Wrap string keys using the translation macro `t!("your_key")`.
2. Add the corresponding key-value pairs to **both** `locales/en.json` and `locales/fr.json`.
3. If contributing a new language, create `locales/<lang_code>.json` and ensure all existing keys are populated.

---

## 🌿 Git Workflow & Commit Guidelines

1. **Fork** the repository and clone your fork locally:
   ```bash
   git clone https://github.com/<your-username>/NeuraDex.git
   cd NeuraDex
   ```
2. **Create a topic branch**:
   ```bash
   git checkout -b feature/hardware-amd-sensor
   # or
   git checkout -b fix/ollama-timeout-handling
   ```
3. **Write Conventional Commits**:
   We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:
   - `feat: add support for ROCm 6.x memory telemetry`
   - `fix: prevent UI freeze when Ollama stream aborts unexpectedly`
   - `docs: update troubleshooting guide for Fedora 41`
   - `refactor: extract model card widget into standalone component`
   - `test: add integration test for custom Modelfile parsing`

---

## 🚀 Submitting a Pull Request

1. Push your branch to your GitHub fork:
   ```bash
   git push origin feature/your-feature-name
   ```
2. Open a Pull Request against the `main` branch of `BazinFla/NeuraDex`.
3. Fill out the PR description template clearly:
   - What problem does this PR solve?
   - How was it tested (hardware configuration, Linux distribution, manual steps)?
   - Include before/after screenshots or recordings for any UI/visual changes.
4. Ensure all CI checks (tests, fmt, clippy) pass.

Thank you for helping make NeuraDex the best native AI cockpit on Linux! 🚀
