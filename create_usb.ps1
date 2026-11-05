<#
 ============================================================================
 Project: Beaversoft Meta-Kernel
 Author: José Alberto Puente Maldonado (japuentem)
 Brand: Beaversoft
 Description: Beaver OS - Creador Automatizado de USB Booteable UEFI Real
 ============================================================================
#>

param(
    [string]$DriveLetter,
    [switch]$SkipFormat,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

# UTF-8 Encoding
$null = & chcp 65001 2>&1
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

Write-Host "========================================================================" -ForegroundColor Cyan
Write-Host "   BEAVER OS - Creador de Memoria USB Booteable UEFI (Hardware Real)" -ForegroundColor Yellow
Write-Host "   Autor: José Alberto Puente Maldonado (japuentem)" -ForegroundColor Green
Write-Host "   Arquitectura: x86_64 UEFI Native (GOP Framebuffer + Long Mode)" -ForegroundColor DarkGray
Write-Host "========================================================================" -ForegroundColor Cyan
Write-Host ""

# 1. Verificar herramientas de compilación
$CargoBin = "$env:USERPROFILE\.cargo\bin"
if ((Test-Path $CargoBin) -and -not ($env:PATH -split ';' -contains $CargoBin)) {
    $env:PATH = "$CargoBin;$env:PATH"
}

$LLD = "C:\Users\japue\.rustup\toolchains\nightly-x86_64-pc-windows-msvc\lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe"
if (-not (Test-Path $LLD)) {
    Write-Error "[ERROR] No se localizó el enlazador rust-lld en: $LLD"
    exit 1
}

# 2. Compilar Kernel Beaver y Cargador UEFI BOOTX64.EFI
Write-Host "[1/4] Compilando Beaver Kernel (Release x86_64)..." -ForegroundColor Yellow
& cargo build --release --target x86_64-unknown-none
if ($LASTEXITCODE -ne 0) {
    Write-Error "[ERROR] Falló la compilación del kernel Beaver."
    exit $LASTEXITCODE
}

Write-Host "[2/4] Generando binario de arranque UEFI BOOTX64.EFI..." -ForegroundColor Yellow
& rustc +nightly --target x86_64-pc-windows-msvc -C panic=abort -O --emit=obj "boot\uefi\src\main.rs" -o "target\bootx64.obj"
if ($LASTEXITCODE -ne 0) {
    Write-Error "[ERROR] Falló la compilación del objeto UEFI bootx64.obj."
    exit $LASTEXITCODE
}

& $LLD -flavor link /NODEFAULTLIB /ENTRY:efi_main /SUBSYSTEM:EFI_APPLICATION "target\bootx64.obj" /OUT:"target\BOOTX64.EFI"
if ($LASTEXITCODE -ne 0) {
    Write-Error "[ERROR] Falló el enlace de BOOTX64.EFI."
    exit $LASTEXITCODE
}

$EfiSize = (Get-Item "target\BOOTX64.EFI").Length
Write-Host "[OK] BOOTX64.EFI generado exitosamente ($([math]::Round($EfiSize / 1KB, 2)) KB)." -ForegroundColor Green
Write-Host ""

# 3. Detectar unidades USB disponibles
Write-Host "[3/4] Escaneando dispositivos de almacenamiento USB conectados..." -ForegroundColor Yellow
$UsbDisks = Get-Disk | Where-Object { $_.BusType -eq 'USB' }

if (-not $UsbDisks) {
    Write-Host "[AVISO] No se detectaron memorias USB conectadas a este equipo." -ForegroundColor Red
    Write-Host "        Por favor conecta una memoria USB (Pendrive) y vuelve a ejecutar este script." -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "        Si prefieres probar el booteo UEFI en este momento de forma virtual, ejecuta:" -ForegroundColor Cyan
    Write-Host "        .\run.ps1 -Uefi" -ForegroundColor White
    exit 0
}

Write-Host "Dispositivos USB detectados:" -ForegroundColor Cyan
$Index = 1
$DriveChoices = @()

foreach ($disk in $UsbDisks) {
    $partitions = Get-Partition -DiskNumber $disk.Number -ErrorAction SilentlyContinue
    $letters = ($partitions | Where-Object { $_.DriveLetter } | ForEach-Object { "$($_.DriveLetter):" }) -join ", "
    if (-not $letters) { $letters = "(Sin asignar)" }
    $sizeGb = [math]::Round($disk.Size / 1GB, 2)
    Write-Host "  [$Index] Disco $($disk.Number): $($disk.FriendlyName) - $sizeGb GB - Letra: $letters" -ForegroundColor White
    $DriveChoices += [PSCustomObject]@{
        Index = $Index
        DiskNumber = $disk.Number
        Name = $disk.FriendlyName
        SizeGb = $sizeGb
        Letters = $letters
        Partitions = $partitions
    }
    $Index++
}
Write-Host ""

$SelectedDrive = $null
if ($DriveLetter) {
    $TargetLetter = $DriveLetter.TrimEnd(':').ToUpper()
    foreach ($choice in $DriveChoices) {
        if ($choice.Letters -match "$TargetLetter:") {
            $SelectedDrive = $choice
            break
        }
    }
    if (-not $SelectedDrive) {
        Write-Error "[ERROR] La letra especificada '$DriveLetter' no coincide con ninguna unidad USB conectada."
        exit 1
    }
} else {
    $Selection = Read-Host "Ingresa el número de la memoria USB que deseas preparar (o 'q' para salir)"
    if ($Selection -eq 'q') {
        Write-Host "Operación cancelada por el usuario." -ForegroundColor Yellow
        exit 0
    }
    $ChoiceIdx = [int]$Selection
    $SelectedDrive = $DriveChoices | Where-Object { $_.Index -eq $ChoiceIdx }
    if (-not $SelectedDrive) {
        Write-Error "[ERROR] Opción no válida."
        exit 1
    }
}

# Obtener letra de unidad activa
$TargetVolLetter = ($SelectedDrive.Partitions | Where-Object { $_.DriveLetter } | Select-Object -First 1).DriveLetter
if (-not $TargetVolLetter) {
    Write-Error "[ERROR] El disco USB seleccionado no tiene una partición con letra asignada. Por favor asígnale una letra en Administración de Discos."
    exit 1
}

$MountPath = "$($TargetVolLetter):\"

Write-Host ""
Write-Host "¡ATENCIÓN!" -ForegroundColor Red -BackgroundColor Black
Write-Host "Se preparará la unidad USB en $MountPath ($($SelectedDrive.Name), $($SelectedDrive.SizeGb) GB)" -ForegroundColor Yellow
if (-not $SkipFormat) {
    Write-Host "La unidad será formateada en FAT32 con etiqueta 'BEAVER_OS'." -ForegroundColor Red
} else {
    Write-Host "Se copiarán los archivos de arranque conservando los datos existentes." -ForegroundColor Cyan
}

if (-not $Force) {
    $Confirm = Read-Host "¿Estás 100% seguro de continuar? Escribe 'SI' para confirmar"
    if ($Confirm -ne 'SI') {
        Write-Host "Operación cancelada por seguridad." -ForegroundColor Yellow
        exit 0
    }
}

# 4. Formatear y preparar estructura UEFI
Write-Host ""
Write-Host "[4/4] Escribiendo cargador Beaver OS UEFI en la unidad USB..." -ForegroundColor Yellow

if (-not $SkipFormat) {
    Write-Host "  * Formateando $MountPath en FAT32..." -ForegroundColor DarkGray
    Format-Volume -DriveLetter $TargetVolLetter -FileSystem FAT32 -NewFileSystemLabel "BEAVER_OS" -Confirm:$false | Out-Null
}

$EfiBootDir = Join-Path $MountPath "EFI\BOOT"
if (-not (Test-Path $EfiBootDir)) {
    New-Item -ItemType Directory -Path $EfiBootDir -Force | Out-Null
}

$DestEfi = Join-Path $EfiBootDir "BOOTX64.EFI"
Copy-Item "target\BOOTX64.EFI" -Destination $DestEfi -Force
Write-Host "  * Copiado BOOTX64.EFI -> $DestEfi" -ForegroundColor Green

# Copiar kernel binario de respaldo para inspección
$BootDir = Join-Path $MountPath "boot"
if (-not (Test-Path $BootDir)) {
    New-Item -ItemType Directory -Path $BootDir -Force | Out-Null
}
Copy-Item "target\x86_64-unknown-none\release\beaver-kernel" -Destination (Join-Path $BootDir "beaver-kernel.elf") -Force

# Copiar ejecutables de usuario ELF64 precompilados
$BinDir = Join-Path $MountPath "BIN"
if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
}
if (Test-Path "target\x86_64-unknown-none\release\beaver-hello") {
    Copy-Item "target\x86_64-unknown-none\release\beaver-hello" -Destination (Join-Path $BinDir "HELLO.ELF") -Force
}
if (Test-Path "target\x86_64-unknown-none\release\beaver-bench") {
    Copy-Item "target\x86_64-unknown-none\release\beaver-bench" -Destination (Join-Path $BinDir "BENCH.ELF") -Force
}
if (Test-Path "target\x86_64-unknown-none\release\beaver-sh") {
    Copy-Item "target\x86_64-unknown-none\release\beaver-sh" -Destination (Join-Path $BinDir "SH.ELF") -Force
}

# Copiar scripts nativos de demostración
$ScriptsDir = Join-Path $MountPath "SCRIPTS"
if (-not (Test-Path $ScriptsDir)) {
    New-Item -ItemType Directory -Path $ScriptsDir -Force | Out-Null
}
$DemoScript = @'
// BeaverScript v1.0 - Script de Demostracion en Hardware Real
println "=== BEAVER OS BARE-METAL HARDWARE DEMO ==="
let a = 15
let b = 25
let total = a + b
println "Calculo matematico a + b: " + total
let mi_array = [100, 200, 300]
println "Array persistente: " + mi_array
println "Elemento indexado [1]: " + mi_array[1]
println "Generando melodia acustica en hardware..."
beep(523, 70)
beep(659, 70)
beep(784, 80)
loop 3 { println "Iteracion de bucle interactivo en Ring 3" }
println "=== SCRIPT EJECUTADO CON EXITO EN RING 3 ==="
'@
Set-Content -Path (Join-Path $ScriptsDir "demo.bs") -Value $DemoScript -Encoding UTF8

# Crear archivo README en la raíz de la USB con instrucciones claras
$ReadmeContent = @"
===============================================================================
  BEAVER OS - Memoria USB Booteable UEFI
  Autor: José Alberto Puente Maldonado (japuentem)
  Sitio/Marca: Beaversoft Meta-Kernel
===============================================================================

¡Tu memoria USB de Beaver OS está lista para arrancar en cualquier PC o Laptop moderna!

INSTRUCCIONES DE ARRANQUE EN HARDWARE REAL:
1. Conecta esta memoria USB en el puerto USB 2.0 o USB 3.0 de la computadora apagada.
2. Enciende el equipo y presiona repetidamente la tecla del 'Boot Menu' según el fabricante:
   - Dell: F12
   - HP: F9 o Esc
   - Lenovo / ThinkPad: F12 (o Fn+F12)
   - ASUS: F8 o Esc
   - Acer: F12 (si no abre, entrar a BIOS con F2 y habilitar 'F12 Boot Menu')
   - Toshiba: F12
   - MSI: F11
   - Gigabyte: F12
   - Microsoft Surface: Mantener presionado Bajar Volumen y presionar botón de Encendido.

3. CONFIGURACIÓN DE BIOS / UEFI RECOMENDADA:
   - Secure Boot: DESACTIVAR (Disabled). Como Beaver OS es un sistema operativo independiente
     y no posee firma comercial de Microsoft Corporation, Secure Boot debe estar apagado.
   - Boot Mode: UEFI Native (no requiere Legacy ni CSM).
   - Fast Boot: Recomendado Desactivar para dar tiempo de detectar el USB.

4. SELECCIÓN:
   En el menú de arranque, selecciona:
   "UEFI: Beaver OS / USB Flash Drive" (o la opción UEFI correspondiente a tu memoria USB).

¡El sistema cargará en alta resolución con el entorno visual y terminal Beaver!
===============================================================================
"@

Set-Content -Path (Join-Path $MountPath "README_BEAVER_BOOT.txt") -Value $ReadmeContent -Encoding UTF8
Write-Host "  * Creado README_BEAVER_BOOT.txt con guía paso a paso" -ForegroundColor Green

Write-Host ""
Write-Host "========================================================================" -ForegroundColor Green
Write-Host "  ¡ÉXITO! Tu memoria USB Beaver OS ha sido creada con éxito en $MountPath" -ForegroundColor Yellow
Write-Host "========================================================================" -ForegroundColor Green
Write-Host "Ya puedes expulsar con seguridad la USB y probarla en cualquier laptop o PC." -ForegroundColor White
Write-Host "Consulta 'README_BEAVER_BOOT.txt' en la USB para la guía de teclas de BIOS." -ForegroundColor Cyan

