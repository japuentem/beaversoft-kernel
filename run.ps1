<#
 ============================================================================
 Project: Beaversoft Meta-Kernel
 Author: José Alberto Puente Maldonado (japuentem)
 Brand: Beaversoft
 Description: Automated build & QEMU runner script for PowerShell (Windows)
 ============================================================================
#>

param(
    [switch]$DebugBuild,
    [switch]$NoGui,
    [switch]$Clean,
    [switch]$Uefi
)

$ErrorActionPreference = "Stop"

# Forzar UTF-8 en consola de Windows (chcp 65001) para visualización correcta
$null = & chcp 65001 2>&1
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

# Asegurar rutas estándar de Rust y QEMU en PATH
$CargoBin = "$env:USERPROFILE\.cargo\bin"
if ((Test-Path $CargoBin) -and -not ($env:PATH -split ';' -contains $CargoBin)) {
    $env:PATH = "$CargoBin;$env:PATH"
}

$QemuBin = "C:\Program Files\qemu"
if ((Test-Path $QemuBin) -and -not ($env:PATH -split ';' -contains $QemuBin)) {
    $env:PATH = "$QemuBin;$env:PATH"
}

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "  BEAVER OS - Automation Runner" -ForegroundColor Yellow
Write-Host "  Author: José Alberto Puente Maldonado (japuentem)" -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor Cyan

if ($Clean) {
    Write-Host "[TASK] Limpiando artefactos de compilación..." -ForegroundColor Yellow
    cargo clean
    Write-Host "[OK] Directorio target limpiado." -ForegroundColor Green
    exit 0
}

$Target = "x86_64-unknown-none"
$Mode = if ($DebugBuild) { "debug" } else { "release" }

Write-Host "[TASK] Compilando kernel para arquitectura $Target en modo $Mode..." -ForegroundColor Yellow

if ($DebugBuild) {
    cargo build --target $Target
} else {
    cargo build --release --target $Target
}

if ($LASTEXITCODE -ne 0) {
    Write-Error "[ERROR] Falló la compilación del kernel."
    exit $LASTEXITCODE
}

$KernelBin = "target\$Target\$Mode\beaversoft-kernel"
if (-not (Test-Path $KernelBin)) {
    Write-Error "[ERROR] No se encontró el binario generado en $KernelBin"
    exit 1
}

Write-Host "[OK] Kernel compilado exitosamente: $KernelBin" -ForegroundColor Green
$DiskImg = "target\disk.img"
if (-not (Test-Path $DiskImg)) {
    Write-Host "[INFO] Creando imagen de disco virtual ATA (10 MB)..." -ForegroundColor DarkGray
    $fs = [System.IO.File]::Create($DiskImg)
    $fs.SetLength(10 * 1024 * 1024)
    $fs.Close()
}

if ($Uefi) {
    Write-Host "[UEFI] Modo UEFI activado. Construyendo cargador BOOTX64.EFI..." -ForegroundColor Yellow
    $LLD = "C:\Users\japue\.rustup\toolchains\nightly-x86_64-pc-windows-msvc\lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe"
    & rustc +nightly --target x86_64-pc-windows-msvc -C panic=abort -O --emit=obj "boot\uefi\src\main.rs" -o "target\bootx64.obj"
    & $LLD -flavor link /NODEFAULTLIB /ENTRY:efi_main /SUBSYSTEM:EFI_APPLICATION "target\bootx64.obj" /OUT:"target\BOOTX64.EFI"

    $UefiDir = "target\uefi_test\EFI\BOOT"
    if (-not (Test-Path $UefiDir)) {
        New-Item -ItemType Directory -Path $UefiDir -Force | Out-Null
    }
    Copy-Item "target\BOOTX64.EFI" -Destination "$UefiDir\BOOTX64.EFI" -Force

    $OvmfFirmware = "C:\Program Files\qemu\share\edk2-x86_64-code.fd"
    $QemuArgs = @(
        "-drive", "if=pflash,format=raw,readonly=on,file=$OvmfFirmware",
        "-drive", "file=$DiskImg,format=raw,index=0,media=disk",
        "-drive", "file=fat:rw:target\uefi_test,format=raw,index=1,media=disk",
        "-nic", "model=e1000",
        "-smp", "4",
        "-m", "512M",
        "-serial", "stdio"
    )
    if ($NoGui) {
        $QemuArgs += @("-display", "none")
    } else {
        $QemuArgs += @("-audiodev", "dsound,id=snd0", "-machine", "pcspk-audiodev=snd0", "-device", "intel-hda", "-device", "hda-duplex,audiodev=snd0")
    }
} else {
    $QemuArgs = @(
        "-kernel", $KernelBin,
        "-drive", "file=$DiskImg,format=raw,index=0,media=disk",
        "-nic", "model=e1000",
        "-smp", "4",
        "-serial", "stdio"
    )
    if ($NoGui) {
        $QemuArgs += @("-display", "none")
    } else {
        $QemuArgs += @("-audiodev", "dsound,id=snd0", "-machine", "pcspk-audiodev=snd0", "-device", "intel-hda", "-device", "hda-duplex,audiodev=snd0")
    }
}

Write-Host "[INFO] Ejecutando: qemu-system-x86_64 $($QemuArgs -join ' ')" -ForegroundColor DarkGray
& qemu-system-x86_64 @QemuArgs
