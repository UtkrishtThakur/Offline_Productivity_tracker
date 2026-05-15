Write-Host "== Tracker Installer (Windows) =="

function Check-Command($cmd) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Host "[ERROR] Missing dependency: $cmd"
        exit 1
    }
}

Write-Host "[*] Checking dependencies..."

Check-Command cargo
Check-Command docker
Check-Command ollama

Write-Host "[OK] Dependencies found"

# -----------------------------
# Create directories
# -----------------------------

$configDir = "$env:APPDATA\\tracker"
$dataDir = "$env:LOCALAPPDATA\\tracker"

New-Item -ItemType Directory -Force -Path $configDir | Out-Null
New-Item -ItemType Directory -Force -Path "$dataDir\\sessions" | Out-Null
New-Item -ItemType Directory -Force -Path "$dataDir\\summaries" | Out-Null

Write-Host "[OK] Directories created"

# -----------------------------
# Build Rust tracker
# -----------------------------

Write-Host "[*] Building Rust tracker..."

Set-Location rust-tracker

cargo build --release

Set-Location ..

Write-Host "[OK] Rust tracker built"

# -----------------------------
# Install binary
# -----------------------------

$installDir = "$env:LOCALAPPDATA\\tracker\\bin"

New-Item -ItemType Directory -Force -Path $installDir | Out-Null

Copy-Item "rust-tracker\\target\\release\\tracker.exe" "$installDir\\tracker.exe" -Force

Write-Host "[OK] tracker.exe installed"

# -----------------------------
# Setup analyzer
# -----------------------------

Set-Location py-analyzer

Copy-Item ".env.example" ".env" -Force

docker compose build

Set-Location ..

Write-Host "[OK] Analyzer configured"

# -----------------------------
# Optional model pull
# -----------------------------

$modelChoice = Read-Host "Pull qwen2.5:7b model? (y/n)"

if ($modelChoice -eq "y") {
    ollama pull qwen2.5:7b
}

Write-Host ""
Write-Host "=================================="
Write-Host "Tracker installed successfully"
Write-Host ""
Write-Host "Run:"
Write-Host "  tracker.exe start"
Write-Host "=================================="