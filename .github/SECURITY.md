# Security Policy

NeuraDex takes the security of its users and systems seriously. Because NeuraDex interacts with system daemons (`systemd`), manages cloud API tokens, handles SSH keys, and configures network exposure, we place high importance on safe defaults and prompt resolution of vulnerabilities.

---

## 🛡️ Supported Versions

We provide security updates and patches for the following versions:

| Version | Supported          | Status |
| ------- | ------------------ | ------ |
| `0.1.x` | :white_check_mark: | Current active release branch |
| `< 0.1` | :x:                | Unsupported |

---

## 🚨 Reporting a Vulnerability

**Please DO NOT report security vulnerabilities through public GitHub issues or public discussions.**

If you discover or suspect a security vulnerability in NeuraDex, please report it through one of the following private channels:

1. **GitHub Private Vulnerability Reporting (Recommended)**:
   Navigate to the **Security** tab of the repository and click **[Report a vulnerability](https://github.com/BazinFla/NeuraDex/security/advisories/new)** to open a private advisory draft.
2. **Direct Contact**:
   If you cannot use GitHub Advisories, you can contact the project maintainer directly via GitHub profile contact ([@BazinFla](https://github.com/BazinFla)).

### What to Include in Your Report

To help us triage and resolve the issue quickly, please include:
- A clear, descriptive summary of the vulnerability.
- Steps to reproduce the issue (including sample payloads, mock environments, or reproduction scripts where applicable).
- The version of NeuraDex, Linux distribution, and desktop environment (GNOME, KDE, etc.) where the issue was observed.
- The potential security impact (e.g., local privilege escalation, credential exfiltration, path traversal).
- Any proposed mitigations or patch suggestions if available.

### Response Timeline & Coordinated Disclosure

- **Acknowledgment**: We aim to acknowledge receipt of your report within **48 to 72 hours**.
- **Assessment**: We will investigate, reproduce the issue, and provide an initial assessment with an estimated timeline for a fix.
- **Fix & Release**: A security release will be prepared and tested.
- **Coordinated Disclosure**: We request that you allow reasonable time for a fix to be published before disclosing any details publicly. Once a patch is released, credit will be given in the release notes and advisory (unless you wish to remain anonymous).

---

## 🔍 Security Architecture & Threat Model

When auditing NeuraDex, please keep the following operational context in mind:

### 1. API Token & SSH Key Storage
- **Current State (v0.1.x)**: API tokens (Ollama Cloud, Hugging Face, OpenRouter) and SSH keys are stored locally in the user's XDG configuration directory (`~/.config/neuradex/`) with strict file permissions (`0600`).
- **Roadmap (v0.2.0+)**: Transitioning to native FreeDesktop Secret Service integration (`oo7` / GNOME Keyring & KDE KWallet) for hardware-backed/OS-level secret encryption.
- *High Priority*: Any vulnerability allowing unauthorized local or remote access, leakage in log files (`journalctl`), or insecure temporary file creation.

### 2. Systemd Service Control & System Configuration
- NeuraDex manages the Ollama system service (`systemctl start/stop/restart`) and generates systemd environment override drop-ins (e.g., `OLLAMA_MODELS`, `OLLAMA_HOST`).
- *High Priority*: Any command injection, argument injection, or path traversal vulnerability in systemd command generation or configuration writers.

### 3. Network Exposure & Daemon Binding
- NeuraDex provides a 1-click switcher between Localhost (`127.0.0.1`), Local Network (`0.0.0.0`), and Exposed / Internet (`0.0.0.0` with `OLLAMA_ORIGINS="*"`).
- Because Ollama does not provide built-in authentication, NeuraDex enforces explicit user confirmation warnings before binding to public interfaces. Insecure default bindings or bypasses of user confirmation are considered critical UX security bugs.

### 4. Remote Model Downloads (Hugging Face & GGUF)
- NeuraDex queries Hugging Face APIs and downloads GGUF model files to the local Ollama storage directory.
- *High Priority*: Path traversal in model names/filenames (`../../`), Server-Side Request Forgery (SSRF), or arbitrary file write vulnerabilities during download or quantization selection.

### 5. Dependency Supply Chain
- NeuraDex strictly audits its Rust crate dependencies using `cargo audit` (targeting 0 known CVEs across all dependencies). Automated CI pipelines enforce this check on all pull requests.
