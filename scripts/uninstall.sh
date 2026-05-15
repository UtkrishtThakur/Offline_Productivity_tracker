#!/usr/bin/env bash

set -e

echo "== Tracker Uninstaller =="

read -p "Remove summaries and session data too? (y/n): " REMOVE_DATA

# -----------------------------
# Stop Docker analyzer
# -----------------------------

if [ -d "py-analyzer" ]; then
    echo "[*] Stopping analyzer..."

    cd py-analyzer

    docker compose down || true

    cd ..
fi

# -----------------------------
# Remove binary
# -----------------------------

echo "[*] Removing tracker binary..."

sudo rm -f /usr/local/bin/tracker

# -----------------------------
# Remove systemd service
# -----------------------------

if [ -f "/etc/systemd/system/tracker.service" ]; then
    echo "[*] Removing systemd service..."

    sudo systemctl stop tracker || true
    sudo systemctl disable tracker || true

    sudo rm -f /etc/systemd/system/tracker.service

    sudo systemctl daemon-reload
fi

# -----------------------------
# Remove config
# -----------------------------

echo "[*] Removing config..."

rm -rf "$HOME/.config/tracker"

# -----------------------------
# Optional data removal
# -----------------------------

if [ "$REMOVE_DATA" = "y" ]; then
    echo "[*] Removing session data..."

    rm -rf "$HOME/.local/share/tracker"
fi

echo ""
echo "=================================="
echo "Tracker uninstalled successfully"
echo "=================================="