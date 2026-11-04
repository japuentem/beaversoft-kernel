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
    [switch]$Uefi,
    [switch]$Iso,
    [switch]$Test,
    # -Node A: escucha :5555  MAC ..:BE:A0:01  IP 10.0.0.1
    # -Node B: conecta :5555  MAC ..:BE:A0:02  IP 10.0.0.2
    [string]$Node = ''
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

$KernelBin = "target\$Target\$Mode\beaver-kernel"
if (-not (Test-Path $KernelBin)) {
    Write-Error "[ERROR] No se encontró el binario generado en $KernelBin"
    exit 1
}

Write-Host "[OK] Kernel compilado exitosamente: $KernelBin" -ForegroundColor Green
$DiskImg = if ($Node -eq 'A') {
    "target\disk-A.img"
} elseif ($Node -eq 'B') {
    "target\disk-B.img"
} else {
    "target\disk.img"
}
if (-not (Test-Path $DiskImg)) {
    Write-Host "[INFO] Creando imagen de disco virtual ATA ($DiskImg, 10 MB)..." -ForegroundColor DarkGray
    $fs = [System.IO.File]::Create($DiskImg)
    $fs.SetLength(10 * 1024 * 1024)
    $fs.Close()
}

if ($Iso) {
    Write-Host "[ISO] Modo ISO activado. Generando imagen híbrida target\beaver-os-v0.9.0.iso..." -ForegroundColor Yellow
    & powershell -ExecutionPolicy Bypass -File "scripts\build_iso.ps1" -SkipBuild
    if ($LASTEXITCODE -ne 0) {
        Write-Error "[ERROR] Falló la generación de la imagen ISO."
        exit $LASTEXITCODE
    }

    $IsoFile = "target\beaver-os-v0.9.0.iso"
    $OvmfFirmware = "C:\Program Files\qemu\share\edk2-x86_64-code.fd"
    $QemuArgs = @(
        "-drive", "if=pflash,format=raw,readonly=on,file=$OvmfFirmware",
        "-cdrom", $IsoFile,
        "-boot", "d",
        "-device", "ahci,id=ahci0",
        "-drive", "id=sata0,file=$DiskImg,format=raw,if=none",
        "-device", "ide-hd,drive=sata0,bus=ahci0.0",
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
} elseif ($Uefi) {
    Write-Host "[UEFI] Modo UEFI activado. Construyendo cargador BOOTX64.EFI..." -ForegroundColor Yellow
    $LLD = "C:\Users\japue\.rustup\toolchains\nightly-x86_64-pc-windows-msvc\lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe"
    $BootObj = if ($Node -ne '') { "target\bootx64-$Node.obj" } else { "target\bootx64.obj" }
    $BootEfi = if ($Node -ne '') { "target\BOOTX64-$Node.EFI" } else { "target\BOOTX64.EFI" }
    & rustc +nightly --target x86_64-pc-windows-msvc -C panic=abort -O --emit=obj "boot\uefi\src\main.rs" -o $BootObj
    & $LLD -flavor link /NODEFAULTLIB /ENTRY:efi_main /SUBSYSTEM:EFI_APPLICATION $BootObj /OUT:$BootEfi

    $UefiRoot = if ($Node -eq 'A') {
        "target\uefi_test_A"
    } elseif ($Node -eq 'B') {
        "target\uefi_test_B"
    } else {
        "target\uefi_test"
    }
    $UefiDir = "$UefiRoot\EFI\BOOT"
    if (-not (Test-Path $UefiDir)) {
        New-Item -ItemType Directory -Path $UefiDir -Force | Out-Null
    }
    Copy-Item $BootEfi -Destination "$UefiDir\BOOTX64.EFI" -Force
    Copy-Item $KernelBin -Destination "$UefiRoot\KERNEL.ELF" -Force

    $OvmfFirmware = "C:\Program Files\qemu\share\edk2-x86_64-code.fd"
    $QemuArgs = @(
        "-drive", "if=pflash,format=raw,readonly=on,file=$OvmfFirmware",
        "-device", "ahci,id=ahci0",
        "-drive", "id=sata0,file=$DiskImg,format=raw,if=none",
        "-device", "ide-hd,drive=sata0,bus=ahci0.0",
        "-drive", "file=fat:rw:$UefiRoot,format=raw,index=1,media=disk",
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
        "-device", "ahci,id=ahci0",
        "-drive", "id=sata0,file=$DiskImg,format=raw,if=none",
        "-device", "ide-hd,drive=sata0,bus=ahci0.0",
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

# Banco de pruebas: dos QEMU conectados por socket (Fase 4 - E1000 real)
if ($Node -eq 'A') {
    $QemuArgs = $QemuArgs | Where-Object { $_ -ne 'model=e1000' -and $_ -ne '-nic' }
    # Nodo A: escucha en puerto 5555, MAC fija ...:BE:A0:01
    $QemuArgs += @("-netdev", "socket,id=n0,listen=:5555")
    $QemuArgs += @("-device", "e1000,netdev=n0,mac=52:54:00:BE:A0:01")
    Write-Host "[NET] Nodo A: escuchando en :5555 (IP estatica 10.0.0.1)" -ForegroundColor Cyan
} elseif ($Node -eq 'B') {
    $QemuArgs = $QemuArgs | Where-Object { $_ -ne 'model=e1000' -and $_ -ne '-nic' }
    # Nodo B: conecta al nodo A, MAC fija ...:BE:A0:02
    $QemuArgs += @("-netdev", "socket,id=n0,connect=127.0.0.1:5555")
    $QemuArgs += @("-device", "e1000,netdev=n0,mac=52:54:00:BE:A0:02")
    Write-Host "[NET] Nodo B: conectando a 127.0.0.1:5555 (IP estatica 10.0.0.2)" -ForegroundColor Cyan
}
if ($Test) {
    Write-Host "[TEST] Modo de pruebas automaticas QEMU (isa-debug-exit 0xF4) activado..." -ForegroundColor Cyan
    $LLD = "C:\Users\japue\.rustup\toolchains\nightly-x86_64-pc-windows-msvc\lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe"
    & rustc +nightly --target x86_64-pc-windows-msvc -C panic=abort -O --emit=obj "boot\uefi\src\main.rs" -o "target\bootx64-test.obj"
    & $LLD -flavor link /NODEFAULTLIB /ENTRY:efi_main /SUBSYSTEM:EFI_APPLICATION "target\bootx64-test.obj" /OUT:"target\BOOTX64-TEST.EFI"

    $UefiRoot = "target\uefi_autotest"
    $UefiDir = "$UefiRoot\EFI\BOOT"
    New-Item -ItemType Directory -Path $UefiDir -Force | Out-Null
    Copy-Item "target\BOOTX64-TEST.EFI" -Destination "$UefiDir\BOOTX64.EFI" -Force
    Copy-Item $KernelBin -Destination "$UefiRoot\KERNEL.ELF" -Force

    $OvmfFirmware = "C:\Program Files\qemu\share\edk2-x86_64-code.fd"
    $LogFile = "target\autotest_serial.log"
    if (Test-Path $LogFile) { Remove-Item $LogFile -Force }

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = "C:\Program Files\qemu\qemu-system-x86_64.exe"
    $psi.WorkingDirectory = (Get-Location).Path
    $psi.Arguments = "-drive `"if=pflash,format=raw,readonly=on,file=$OvmfFirmware`" -device ahci,id=ahci0 -drive id=sata0,file=$DiskImg,format=raw,if=none -device ide-hd,drive=sata0,bus=ahci0.0 -drive file=fat:rw:$UefiRoot,format=raw,index=1,media=disk -nic model=e1000 -smp 4 -m 512M -device isa-debug-exit,iobase=0xf4,iosize=0x04 -chardev stdio,id=char0,logfile=$LogFile,signal=off -serial chardev:char0 -display none"
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi
    $proc.Start() | Out-Null
    $proc.BeginOutputReadLine()
    $proc.BeginErrorReadLine()

    for ($i = 0; $i -lt 50; $i++) {
        if ((Test-Path $LogFile) -and ((Get-Content $LogFile -Raw -ErrorAction SilentlyContinue) -match "root@beaver:/#")) {
            break
        }
        Start-Sleep -Milliseconds 500
    }
    $proc.StandardInput.WriteLine("autotest exit")
    $exited = $proc.WaitForExit(15000)
    if (-not $exited) {
        $proc.Kill()
        Write-Error "[FAIL] Timeout esperando salida isa-debug-exit de QEMU."
        exit 1
    }
    Get-Content $LogFile | Select-Object -Last 20
    if ($proc.ExitCode -eq 33) {
        Write-Host "[PASS] QEMU finalizo con codigo 33 (QemuExitCode::Success = 0x10 en puerto 0xF4)." -ForegroundColor Green
        exit 0
    } else {
        Write-Error "[FAIL] QEMU finalizo con codigo inesperado: $($proc.ExitCode)"
        exit 1
    }
}

Write-Host "[INFO] Ejecutando: qemu-system-x86_64 $($QemuArgs -join ' ')" -ForegroundColor DarkGray
& qemu-system-x86_64 @QemuArgs
