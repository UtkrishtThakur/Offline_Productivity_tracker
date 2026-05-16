#!/usr/bin/env bash

set -e

echo "== Tracker Installer (Linux) =="

# -----------------------------
# Dependency checks
# -----------------------------

check_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "[ERROR] Missing dependency: $1"
        exit 1
    fi
}

echo "[*] Checking base dependencies..."

check_command cargo
check_command rustc

echo "[OK] Base dependencies found"

# -----------------------------
# Config dirs
# -----------------------------

CONFIG_DIR="$HOME/.config/tracker"
DATA_DIR="$HOME/.local/share/tracker"

echo "[*] Creating directories..."

mkdir -p "$CONFIG_DIR"
mkdir -p "$DATA_DIR/summaries"
mkdir -p "$DATA_DIR/sessions"

echo "[OK] Directories created"

# -----------------------------
# Build Rust tracker
# -----------------------------

echo "[*] Building Rust tracker..."
cd rust-tracker
cargo build --release
cd ..
echo "[OK] Rust tracker built"

# -----------------------------
# Install binary
# -----------------------------

echo "[*] Installing tracker binary..."
sudo cp rust-tracker/target/release/rust-tracker /usr/local/bin/tracker
sudo chmod +x /usr/local/bin/tracker
echo "[OK] Installed tracker command"

# -----------------------------
# Initialization of config
# -----------------------------

echo "[*] Initializing tracker.toml..."
if [ ! -f "$CONFIG_DIR/tracker.toml" ]; then
    tracker init-config
    mv tracker.toml "$CONFIG_DIR/tracker.toml"
fi

# -----------------------------
# AI Summaries Setup
# -----------------------------

echo ""
read -p "Enable AI semantic summaries? (y/n): " ENABLE_AI

if [ "$ENABLE_AI" = "y" ]; then
    echo "[*] Setting up AI Analyzer dependencies..."

    check_command docker

    if ! command -v ollama >/dev/null 2>&1; then
        echo "[*] Installing Ollama..."
        curl -fsSL https://ollama.com/install.sh | sh
    else
        echo "[OK] Ollama found"
    fi

    read -p "Select model [qwen2.5:7b]: " MODEL_CHOICE
    MODEL=${MODEL_CHOICE:-qwen2.5:7b}

    echo "[*] Pulling model $MODEL..."
    ollama pull "$MODEL"

    echo "[*] Configuring analyzer..."
    cd py-analyzer
    cat <<EOF > .env
TRACKER_AI_ENABLED=true
TRACKER_AI_MODEL=$MODEL
TRACKER_AI_OLLAMA_HOST=http://localhost:11434
EOF
    docker compose build
    cd ..

    echo "[*] Enabling AI in tracker.toml..."
    sed -i 's/enabled = false/enabled = true/g' "$CONFIG_DIR/tracker.toml"
    sed -i "s/model = .*/model = \"$MODEL\"/g" "$CONFIG_DIR/tracker.toml"

    echo "[OK] AI semantic summaries enabled!"
else
    echo "[*] AI summaries disabled. Proceeding with deterministic tracking only."
    sed -i 's/enabled = true/enabled = false/g' "$CONFIG_DIR/tracker.toml"
fi

# -----------------------------
# Complete
# -----------------------------

echo ""
echo "=================================="
echo "Tracker installed successfully"
echo ""
echo "Run:"
echo "  tracker start"
echo "=================================="