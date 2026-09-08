#!/usr/bin/env bash
# ==============================================================================
# Script : install.sh
# Description : Installs NeuraDex (binary, .desktop launcher, and icons)
# Location : scripts/install.sh
# ==============================================================================

set -euo pipefail

# Terminal colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
APP_DIR="${ROOT_DIR}"

APP_ID="io.github.bazinfla.NeuraDex"
APP_NAME="NeuraDex"
BUILD_MODE="release" # "release", "debug"
INSTALL_TARGET="user" # "user" (~/.local) or "system" (/usr/local)
DEV_MODE=false
SKIP_BUILD=false

usage() {
    echo -e "${BOLD}${APP_NAME} - Installation Script${NC}"
    echo ""
    echo -e "${BLUE}Usage:${NC} $0 [options]"
    echo ""
    echo "Options:"
    echo "  --user         Install for current user in ~/.local (default, no sudo required)"
    echo "  --system       Install system-wide in /usr/local (requires root/sudo privileges)"
    echo "  --dev          Developer mode: links .desktop directly to repository binary (debug/release)"
    echo "  --debug        Build in debug mode (instead of --release)"
    echo "  --no-build     Do not build with Cargo (use existing binary)"
    echo "  -h, --help     Show this help"
    echo ""
    echo "Examples:"
    echo "  $0             # Standard user installation (~/.local/bin)"
    echo "  $0 --dev       # Configure desktop launcher to point to development binary"
    echo "  sudo $0 --system # Global installation for all users"
    echo ""
}

# Argument parsing
while [[ $# -gt 0 ]]; do
    case "$1" in
        --user)
            INSTALL_TARGET="user"
            shift
            ;;
        --system)
            INSTALL_TARGET="system"
            shift
            ;;
        --dev)
            DEV_MODE=true
            BUILD_MODE="debug"
            shift
            ;;
        --debug)
            BUILD_MODE="debug"
            shift
            ;;
        --no-build)
            SKIP_BUILD=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown option '$1'${NC}"
            usage
            exit 1
            ;;
    esac
done

echo -e "${BOLD}${BLUE}==> Starting ${APP_NAME} installation...${NC}"

# Define destination paths
if [ "$INSTALL_TARGET" = "system" ]; then
    if [ "$(id -u)" -ne 0 ]; then
        echo -e "${RED}Error: System-wide installation (--system) requires root privileges (run with sudo).${NC}"
        exit 1
    fi
    BIN_DIR="/usr/local/bin"
    APPS_DIR="/usr/local/share/applications"
    ICONS_DIR="/usr/local/share/icons/hicolor"
else
    BIN_DIR="${HOME}/.local/bin"
    APPS_DIR="${HOME}/.local/share/applications"
    ICONS_DIR="${HOME}/.local/share/icons/hicolor"
fi

# 1. Build or locate binary
BINARY_SRC=""
if [ "$SKIP_BUILD" = false ]; then
    if command -v cargo >/dev/null 2>&1; then
        echo -e "${YELLOW}==> Compiling with Cargo (${BUILD_MODE})...${NC}"
        if [ "$BUILD_MODE" = "release" ]; then
            (cd "${APP_DIR}" && cargo build --release)
            BINARY_SRC="${APP_DIR}/target/release/neuradex"
        else
            (cd "${APP_DIR}" && cargo build)
            BINARY_SRC="${APP_DIR}/target/debug/neuradex"
        fi
    else
        echo -e "${YELLOW}Warning: Cargo is not installed.${NC}"
    fi
fi

if [ -z "$BINARY_SRC" ] || [ ! -f "$BINARY_SRC" ]; then
    # Search for an existing binary (e.g. extracted portable archive)
    if [ -f "${APP_DIR}/target/release/neuradex" ]; then
        BINARY_SRC="${APP_DIR}/target/release/neuradex"
    elif [ -f "${APP_DIR}/target/debug/neuradex" ]; then
        BINARY_SRC="${APP_DIR}/target/debug/neuradex"
    elif [ -f "${ROOT_DIR}/neuradex" ]; then
        BINARY_SRC="${ROOT_DIR}/neuradex"
    elif [ -f "${SCRIPT_DIR}/neuradex" ]; then
        BINARY_SRC="${SCRIPT_DIR}/neuradex"
    else
        echo -e "${RED}Error: Unable to find executable 'neuradex'.${NC}"
        echo -e "Please install Cargo and recompile, or provide the binary."
        exit 1
    fi
fi

echo -e "    Source executable: ${BINARY_SRC}"

# 2. Install binary
mkdir -p "${BIN_DIR}" "${APPS_DIR}"
if [ "$DEV_MODE" = true ]; then
    echo -e "${BLUE}==> [Dev Mode] Launcher will point directly to: ${BINARY_SRC}${NC}"
    TARGET_EXEC="${BINARY_SRC}"
else
    echo -e "${YELLOW}==> Copying binary to ${BIN_DIR}/neuradex...${NC}"
    cp -f "${BINARY_SRC}" "${BIN_DIR}/neuradex"
    chmod +x "${BIN_DIR}/neuradex"
    TARGET_EXEC="${BIN_DIR}/neuradex"
fi

# 3. Install icons
echo -e "${YELLOW}==> Installing icons...${NC}"
SVG_ICON_SRC="${APP_DIR}/data/icons/${APP_ID}.svg"
if [ ! -f "$SVG_ICON_SRC" ] && [ -f "${ROOT_DIR}/dev/branding/${APP_ID}.svg" ]; then
    SVG_ICON_SRC="${ROOT_DIR}/dev/branding/${APP_ID}.svg"
fi

if [ -f "$SVG_ICON_SRC" ]; then
    mkdir -p "${ICONS_DIR}/scalable/apps"
    cp -f "$SVG_ICON_SRC" "${ICONS_DIR}/scalable/apps/${APP_ID}.svg"
    echo -e "    ✓ SVG icon installed in ${ICONS_DIR}/scalable/apps/${APP_ID}.svg"
fi

# Install AI model icons
MODELS_ICON_SRC="${APP_DIR}/data/icons/models"
if [ -d "$MODELS_ICON_SRC" ]; then
    mkdir -p "${ICONS_DIR}/scalable/apps"
    cp -f "${MODELS_ICON_SRC}"/*.svg "${ICONS_DIR}/scalable/apps/" 2>/dev/null || true
    echo -e "    ✓ AI model icons installed in ${ICONS_DIR}/scalable/apps/"
fi

# Install dev PNG variants if present
for size in 16 24 32 48 64 128 256 512; do
    PNG_DEV="${ROOT_DIR}/dev/branding/concept_a_vector_${size}.png"
    [ ! -f "$PNG_DEV" ] && PNG_DEV="${ROOT_DIR}/dev/branding/neura_dex_icon_${size}.png"
    if [ -f "$PNG_DEV" ]; then
        mkdir -p "${ICONS_DIR}/${size}x${size}/apps"
        cp -f "$PNG_DEV" "${ICONS_DIR}/${size}x${size}/apps/${APP_ID}.png"
    fi
done

# 4. Install .desktop file
echo -e "${YELLOW}==> Generating Desktop file...${NC}"
DESKTOP_TARGET="${APPS_DIR}/${APP_ID}.desktop"

cat <<EOF > "${DESKTOP_TARGET}"
[Desktop Entry]
Name=NeuraDex
GenericName=LLM & VRAM Control Center
GenericName[fr]=Centre de contrôle LLM & VRAM
Comment=Linux native control center, VRAM supervisor and inference studio for local AI
Comment[fr]=Centre de contrôle, supervision VRAM et studio d'inférence natif Linux pour l'IA locale
Exec=${TARGET_EXEC}
Icon=${APP_ID}
Terminal=false
Type=Application
Categories=Utility;Development;ArtificialIntelligence;
Keywords=AI;LLM;Ollama;VRAM;GPU;Inference;MachineLearning;
Keywords[fr]=IA;LLM;Ollama;VRAM;GPU;Inférence;Apprentissage;
StartupNotify=true
StartupWMClass=${APP_ID}
EOF

chmod 644 "${DESKTOP_TARGET}"
echo -e "    ✓ Launcher created in ${DESKTOP_TARGET}"

# 5. Update GNOME / XDG databases and caches
echo -e "${YELLOW}==> Refreshing system caches...${NC}"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${APPS_DIR}" 2>/dev/null || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${ICONS_DIR}" 2>/dev/null || true
elif command -v gtk4-update-icon-cache >/dev/null 2>&1; then
    gtk4-update-icon-cache -f -t "${ICONS_DIR}" 2>/dev/null || true
fi

# 6. Check user PATH
if [ "$INSTALL_TARGET" = "user" ] && [ "$DEV_MODE" = false ]; then
    if [[ ":$PATH:" != *":${HOME}/.local/bin:"* ]]; then
        echo -e "${YELLOW}Note: '~/.local/bin' is not in your PATH.${NC}"
        echo -e "To launch 'neuradex' directly from any terminal, add this to your ~/.bashrc or ~/.zshrc:"
        echo -e "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    fi
fi

echo ""
echo -e "${GREEN}${BOLD}✓ ${APP_NAME} installation completed successfully!${NC}"
echo -e "The application is now available in your GNOME application menu/grid with its logo."
