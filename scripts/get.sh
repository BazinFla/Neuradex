#!/usr/bin/env bash
# ==============================================================================
# Script      : get.sh
# Description : One-line web installer for NeuraDex
# Usage       : curl -fsSL https://raw.githubusercontent.com/BazinFla/Neuradex/main/scripts/get.sh | bash
# Repository  : https://github.com/BazinFla/Neuradex
# ==============================================================================

set -euo pipefail

# Terminal colors
BOLD='\033[1m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

REPO="BazinFla/Neuradex"
APP_NAME="NeuraDex"

# Privilege elevation helper
run_privileged() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        echo -e "${YELLOW}==> Administrative privileges required to install package:${NC}"
        sudo "$@"
    else
        echo -e "${RED}Error: Neither root privileges nor 'sudo' command are available.${NC}"
        exit 1
    fi
}

main() {
    echo -e "${BOLD}${BLUE}"
    echo "  _   _                      ____            "
    echo " | \ | | ___ _   _ _ __ __ _|  _ \  _____  __"
    echo " |  \| |/ _ \ | | | '__/ _\` | | | |/ _ \ \/ /"
    echo " | |\  |  __/ |_| | | | (_| | |_| |  __/>  < "
    echo " |_| \_|\___|\__,_|_|  \__,_|____/ \___/_/\_\\"
    echo -e "${NC}"
    echo -e "${BOLD}${APP_NAME} — Native Linux Control Center for Local AI${NC}"
    echo "-------------------------------------------------------------"

    # 1. Platform & Architecture check
    OS="$(uname -s)"
    if [ "$OS" != "Linux" ]; then
        echo -e "${RED}Error: NeuraDex is built for Linux (detected: ${OS}).${NC}"
        exit 1
    fi

    ARCH="$(uname -m)"
    if [ "$ARCH" != "x86_64" ]; then
        echo -e "${RED}Error: Pre-compiled binary packages currently only support x86_64 (detected: ${ARCH}).${NC}"
        echo -e "You can compile NeuraDex from source for your architecture using Cargo:"
        echo -e "  https://github.com/${REPO}#build-from-source"
        exit 1
    fi

    # 2. Linux Distribution detection
    DISTRO_ID=""
    DISTRO_LIKE=""
    if [ -f /etc/os-release ]; then
        # shellcheck source=/dev/null
        . /etc/os-release
        DISTRO_ID="${ID:-}"
        DISTRO_LIKE="${ID_LIKE:-}"
    else
        echo -e "${RED}Error: Unable to detect Linux distribution (/etc/os-release missing).${NC}"
        exit 1
    fi

    echo -e "${BLUE}==>${NC} Detected OS: ${BOLD}${PRETTY_NAME:-Linux}${NC} (${ARCH})"

    # 3. Determine package format (DEB, RPM, or Arch/Source)
    PKG_FORMAT=""
    if [[ "$DISTRO_ID" =~ ^(debian|ubuntu|pop|linuxmint|elementary|zorin|kali|tuxedo)$ ]] || [[ "$DISTRO_LIKE" =~ (debian|ubuntu) ]]; then
        PKG_FORMAT="deb"
    elif [[ "$DISTRO_ID" =~ ^(fedora|rhel|centos|almalinux|rocky|nobara)$ ]] || [[ "$DISTRO_LIKE" =~ (fedora|rhel) ]]; then
        PKG_FORMAT="rpm"
    elif [[ "$DISTRO_ID" =~ ^(arch|manjaro|endeavouros|garuda)$ ]] || [[ "$DISTRO_LIKE" =~ arch ]]; then
        PKG_FORMAT="arch"
    else
        # Fallback checks by package manager presence
        if command -v apt >/dev/null 2>&1; then
            PKG_FORMAT="deb"
        elif command -v dnf >/dev/null 2>&1; then
            PKG_FORMAT="rpm"
        elif command -v pacman >/dev/null 2>&1; then
            PKG_FORMAT="arch"
        fi
    fi

    if [ -z "$PKG_FORMAT" ]; then
        echo -e "${RED}Error: Unsupported distribution or package manager.${NC}"
        echo -e "Please install the prerequisites and compile from source:"
        echo -e "  https://github.com/${REPO}#build-from-source"
        exit 1
    fi

    # 4. Fetch latest release information from GitHub
    echo -e "${BLUE}==>${NC} Fetching latest release information from GitHub..."

    RELEASE_JSON="$(curl -fsSL --connect-timeout 8 --max-time 20 "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)"

    DOWNLOAD_URL=""
    LATEST_TAG=""

    if [ -n "$RELEASE_JSON" ]; then
        LATEST_TAG="$(echo "$RELEASE_JSON" | grep -m1 '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)"
        if [ "$PKG_FORMAT" = "deb" ]; then
            DOWNLOAD_URL="$(echo "$RELEASE_JSON" | grep -o 'https://[^"]*amd64\.deb' | head -n 1 || true)"
        elif [ "$PKG_FORMAT" = "rpm" ]; then
            DOWNLOAD_URL="$(echo "$RELEASE_JSON" | grep -o 'https://[^"]*x86_64\.rpm' | head -n 1 || true)"
        elif [ "$PKG_FORMAT" = "arch" ]; then
            DOWNLOAD_URL="$(echo "$RELEASE_JSON" | grep -o 'https://[^"]*pkg\.tar\.zst' | head -n 1 || true)"
        fi
    fi

    # Fallback if GitHub API is rate-limited: parse redirect header from latest release page
    if [ -z "$LATEST_TAG" ] || [ -z "$DOWNLOAD_URL" ]; then
        REDIRECT_HEADER="$(curl -sI --connect-timeout 8 --max-time 15 "https://github.com/${REPO}/releases/latest" | grep -i '^location:' || true)"
        LATEST_TAG="$(echo "$REDIRECT_HEADER" | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' | head -n 1 || true)"
        if [ -n "$LATEST_TAG" ]; then
            VERSION="${LATEST_TAG#v}"
            if [ "$PKG_FORMAT" = "deb" ]; then
                DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/neuradex_${VERSION}-1_amd64.deb"
            elif [ "$PKG_FORMAT" = "rpm" ]; then
                DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/neuradex-${VERSION}-1.x86_64.rpm"
            elif [ "$PKG_FORMAT" = "arch" ]; then
                DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/neuradex-${VERSION}-1-x86_64.pkg.tar.zst"
            fi
        fi
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        echo -e "${RED}Error: Unable to locate a downloadable .${PKG_FORMAT} package on GitHub Releases.${NC}"
        echo -e "Please visit the releases page manually: https://github.com/${REPO}/releases"
        exit 1
    fi

    FILE_NAME="$(basename "$DOWNLOAD_URL")"
    echo -e "${BLUE}==>${NC} Found release: ${BOLD}${LATEST_TAG}${NC} (${FILE_NAME})"

    # 5. Download package to a secure temporary directory
    TMP_DIR="$(mktemp -d -t neuradex-install-XXXXXX)"
    trap 'rm -rf "${TMP_DIR}"' EXIT

    PKG_PATH="${TMP_DIR}/${FILE_NAME}"
    echo -e "${BLUE}==>${NC} Downloading ${APP_NAME}..."
    curl -fL --progress-bar "$DOWNLOAD_URL" -o "$PKG_PATH"

    # 6. Install package with dependency resolution
    echo -e "${BLUE}==>${NC} Installing ${APP_NAME}..."
    if [ "$PKG_FORMAT" = "deb" ]; then
        run_privileged apt update -qq || true
        run_privileged apt install -y "$PKG_PATH"
    elif [ "$PKG_FORMAT" = "rpm" ]; then
        run_privileged dnf install -y "$PKG_PATH"
    elif [ "$PKG_FORMAT" = "arch" ]; then
        run_privileged pacman -U --noconfirm "$PKG_PATH"
    fi

    echo ""
    echo -e "${GREEN}${BOLD}✓ ${APP_NAME} installed successfully!${NC}"
    echo "-------------------------------------------------------------"
    echo -e "You can now launch ${APP_NAME} from:"
    echo -e "  1. Your application launcher / desktop menu (search for '${APP_NAME}')"
    echo -e "  2. Terminal by simply typing: ${BOLD}neuradex${NC}"
    echo ""
}

main "$@"
