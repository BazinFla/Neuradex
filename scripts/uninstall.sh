#!/usr/bin/env bash
# ==============================================================================
# Script : uninstall.sh
# Description : Uninstalls NeuraDex (binary, .desktop launcher, and icons)
# Location : scripts/uninstall.sh
# ==============================================================================

set -euo pipefail

# Terminal colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

APP_ID="io.github.bazinfla.NeuraDex"
OLD_APP_ID="io.github.neuradex.NeuraDex"
APP_NAME="NeuraDex"
TARGET="user"

usage() {
    echo -e "${BOLD}${APP_NAME} - Uninstallation Script${NC}"
    echo ""
    echo -e "${BLUE}Usage:${NC} $0 [options]"
    echo ""
    echo "Options:"
    echo "  --user         Uninstall from ~/.local (default)"
    echo "  --system       Uninstall from /usr/local (requires sudo)"
    echo "  -h, --help     Show this help"
    echo ""
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --user)
            TARGET="user"
            shift
            ;;
        --system)
            TARGET="system"
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

echo -e "${BOLD}${BLUE}==> Uninstalling ${APP_NAME}...${NC}"

if [ "$TARGET" = "system" ]; then
    if [ "$(id -u)" -ne 0 ]; then
        echo -e "${RED}Error: System-wide uninstallation (--system) requires root privileges (sudo).${NC}"
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

# 1. Remove binary
echo -e "${YELLOW}==> Removing binary...${NC}"
rm -f "${BIN_DIR}/neuradex"

# 2. Remove desktop launchers
echo -e "${YELLOW}==> Removing .desktop launchers...${NC}"
rm -f "${APPS_DIR}/${APP_ID}.desktop"
rm -f "${APPS_DIR}/${OLD_APP_ID}.desktop"

# 3. Remove icons
echo -e "${YELLOW}==> Removing icons...${NC}"
rm -f "${ICONS_DIR}/scalable/apps/${APP_ID}.svg"
rm -f "${ICONS_DIR}/scalable/apps/${OLD_APP_ID}.svg"

for size in 16 24 32 48 64 128 256 512; do
    rm -f "${ICONS_DIR}/${size}x${size}/apps/${APP_ID}.png"
    rm -f "${ICONS_DIR}/${size}x${size}/apps/${OLD_APP_ID}.png"
done

# 4. Refresh caches
echo -e "${YELLOW}==> Refreshing system caches...${NC}"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${APPS_DIR}" 2>/dev/null || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${ICONS_DIR}" 2>/dev/null || true
elif command -v gtk4-update-icon-cache >/dev/null 2>&1; then
    gtk4-update-icon-cache -f -t "${ICONS_DIR}" 2>/dev/null || true
fi

echo ""
echo -e "${GREEN}${BOLD}✓ ${APP_NAME} was uninstalled successfully!${NC}"
echo -e "Note: Your settings in ~/.config/neuradex have been preserved."
echo -e "To delete them as well: rm -rf ~/.config/neuradex"
