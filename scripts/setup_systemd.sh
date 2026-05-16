#!/usr/bin/env bash

set -e

echo "== Tracker systemd Setup =="

# --------------------------------------------------
# Ensure tracker binary exists
# --------------------------------------------------

if ! command -v tracker >/dev/null 2>&1; then
    echo "[ERROR] tracker binary not found in PATH"
    echo "Install tracker first."
    exit 1
fi

# --------------------------------------------------
# Detect current user
# --------------------------------------------------

CURRENT_USER=$(whoami)

# --------------------------------------------------
# Create service file
# --------------------------------------------------

SERVICE_FILE="/etc/systemd/system/tracker.service"

echo "[*] Creating systemd service..."

sudo tee "$SERVICE_FILE" > /dev/null <<EOF
[Unit]
Description=Tracker Activity Engine
After=network.target

[Service]
Type=simple
ExecStart=$(which tracker) start
Restart=always
RestartSec=5
User=$CURRENT_USER
WorkingDirectory=$HOME

# Optional logging
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# --------------------------------------------------
# Reload systemd
# --------------------------------------------------

echo "[*] Reloading systemd..."

sudo systemctl daemon-reload

# --------------------------------------------------
# Enable auto-start
# --------------------------------------------------

echo "[*] Enabling tracker service..."

sudo systemctl enable tracker

# --------------------------------------------------
# Start service
# --------------------------------------------------

echo "[*] Starting tracker service..."

sudo systemctl start tracker

# --------------------------------------------------
# Done
# --------------------------------------------------

echo ""
echo "=================================="
echo "Tracker systemd service installed"
echo ""
echo "Useful commands:"
echo "  systemctl status tracker"
echo "  sudo systemctl stop tracker"
echo "  sudo systemctl restart tracker"
echo "  journalctl -u tracker -f"
echo "=================================="