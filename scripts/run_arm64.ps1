# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Description: Runner de Beaver OS para arquitectura ARM64 (aarch64) en QEMU virt
# ============================================================================

param(
    [switch]$BuildOnly
)

$qemu = "C:\Program Files\qemu\qemu-system-aarch64.exe"
$kernel = "target\aarch64-unknown-none\debug\beaver-kernel"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "  BEAVER OS - AArch64 Runner (ARM Cortex-A72 / QEMU virt)" -ForegroundColor Cyan
Write-Host "  Author: José Alberto Puente Maldonado (japuentem)" -ForegroundColor Gray
Write-Host "  Brand: Beaversoft" -ForegroundColor Gray
Write-Host "============================================================" -ForegroundColor Cyan

Write-Host "[TASK] Compilando kernel para arquitectura aarch64-unknown-none..." -ForegroundColor Yellow
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo build --target aarch64-unknown-none

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Falló la compilación de Beaver OS para ARM64." -ForegroundColor Red
    exit 1
}

Write-Host "[OK] Binario generado exitosamente: $kernel" -ForegroundColor Green

if ($BuildOnly) {
    exit 0
}

Write-Host "[RUN] Iniciando Beaver OS en emulador ARM64 (QEMU virt, 512MB RAM)..." -ForegroundColor Cyan
Write-Host "[INFO] Presiona Ctrl+A luego X para salir de QEMU." -ForegroundColor DarkGray

& $qemu -M virt -cpu cortex-a72 -m 512M -nographic -kernel $kernel
