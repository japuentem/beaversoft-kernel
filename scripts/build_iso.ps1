<#
 ============================================================================
 Project: Beaversoft Meta-Kernel
 Author: José Alberto Puente Maldonado (japuentem)
 Brand: Beaversoft
 Description: Beaver OS - Generador de Imagen ISO Híbrida Booteable (UEFI / BIOS)
 ============================================================================
#>

param(
    [switch]$SkipBuild,
    [string]$OutputIso = "target\beaver-os-v0.9.0.iso",
    [string]$OutputImg = "target\beaver-os-v0.9.0.img"
)

$ErrorActionPreference = "Stop"

# UTF-8
$null = & chcp 65001 2>&1
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

Write-Host "========================================================================" -ForegroundColor Cyan
Write-Host "   BEAVER OS - Generador de Imagen ISO Híbrida y Medio USB (v0.9.0)" -ForegroundColor Yellow
Write-Host "   Author: José Alberto Puente Maldonado (japuentem)" -ForegroundColor Green
Write-Host "   Arquitectura: x86_64 UEFI Native + El Torito Bootable ISO9660" -ForegroundColor DarkGray
Write-Host "========================================================================" -ForegroundColor Cyan
Write-Host ""

# 1. Rutas de herramientas y binarios
$CargoBin = "$env:USERPROFILE\.cargo\bin"
if ((Test-Path $CargoBin) -and -not ($env:PATH -split ';' -contains $CargoBin)) {
    $env:PATH = "$CargoBin;$env:PATH"
}

$LLD = "C:\Users\japue\.rustup\toolchains\nightly-x86_64-pc-windows-msvc\lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe"
if (-not (Test-Path $LLD)) {
    Write-Error "[ERROR] No se localizó el enlazador rust-lld en: $LLD"
    exit 1
}

$KernelBin = "target\x86_64-unknown-none\release\beaver-kernel"
$BootEfi = "target\BOOTX64.EFI"

if (-not $SkipBuild) {
    Write-Host "[1/4] Compilando Beaver Kernel (Release x86_64)..." -ForegroundColor Yellow
    & cargo build --release --target x86_64-unknown-none
    if ($LASTEXITCODE -ne 0) {
        Write-Error "[ERROR] Falló la compilación del kernel."
        exit $LASTEXITCODE
    }

    Write-Host "[2/4] Compilando cargador de arranque UEFI BOOTX64.EFI..." -ForegroundColor Yellow
    & rustc +nightly --target x86_64-pc-windows-msvc -C panic=abort -O --emit=obj "boot\uefi\src\main.rs" -o "target\bootx64.obj"
    if ($LASTEXITCODE -ne 0) {
        Write-Error "[ERROR] Falló la compilación de bootx64.obj."
        exit $LASTEXITCODE
    }
    & $LLD -flavor link /NODEFAULTLIB /ENTRY:efi_main /SUBSYSTEM:EFI_APPLICATION "target\bootx64.obj" /OUT:$BootEfi
    if ($LASTEXITCODE -ne 0) {
        Write-Error "[ERROR] Falló el enlace de BOOTX64.EFI."
        exit $LASTEXITCODE
    }
}

if (-not (Test-Path $KernelBin) -or -not (Test-Path $BootEfi)) {
    Write-Error "[ERROR] No se encontraron los binarios requeridos ($KernelBin o $BootEfi)."
    exit 1
}

$KernelBytes = [System.IO.File]::ReadAllBytes($KernelBin)
$EfiBytes = [System.IO.File]::ReadAllBytes($BootEfi)

Write-Host "[INFO] Kernel binario: $($KernelBytes.Length) bytes ($([math]::Round($KernelBytes.Length / 1MB, 2)) MB)" -ForegroundColor DarkGray
Write-Host "[INFO] BOOTX64.EFI:    $($EfiBytes.Length) bytes ($([math]::Round($EfiBytes.Length / 1KB, 2)) KB)" -ForegroundColor DarkGray

# 2. Construcción de Sistema de Archivos FAT16 para la partición EFI en memoria
Write-Host "[3/4] Generando partición de sistema EFI (FAT16/GPT compatible)..." -ForegroundColor Yellow

$SectorSize = 512
$SectorsPerCluster = 4 # 2048 bytes por cluster
$ClusterSize = $SectorSize * $SectorsPerCluster
$ReservedSectors = 4
$NumFats = 2
$RootEntries = 512
$RootSectors = [int][math]::Ceiling(($RootEntries * 32) / $SectorSize) # 32 sectores
$SectorsPerFat = 128
$TotalSectors = 32768 # 16 MB

$FatImage = New-Object byte[] ($TotalSectors * $SectorSize)

# --- Sector 0: Boot Sector / BPB ---
$FatImage[0] = 0xEB; $FatImage[1] = 0x3C; $FatImage[2] = 0x90 # JMP short
[System.Text.Encoding]::ASCII.GetBytes("MSWIN4.1").CopyTo($FatImage, 3)
[System.BitConverter]::GetBytes([uint16]$SectorSize).CopyTo($FatImage, 11)
$FatImage[13] = [byte]$SectorsPerCluster
[System.BitConverter]::GetBytes([uint16]$ReservedSectors).CopyTo($FatImage, 14)
$FatImage[16] = [byte]$NumFats
[System.BitConverter]::GetBytes([uint16]$RootEntries).CopyTo($FatImage, 17)
[System.BitConverter]::GetBytes([uint16]$TotalSectors).CopyTo($FatImage, 19)
$FatImage[21] = 0xF8 # Media descriptor
[System.BitConverter]::GetBytes([uint16]$SectorsPerFat).CopyTo($FatImage, 22)
[System.BitConverter]::GetBytes([uint16]63).CopyTo($FatImage, 24) # Sectors per track
[System.BitConverter]::GetBytes([uint16]255).CopyTo($FatImage, 26) # Heads
[System.BitConverter]::GetBytes([uint32]0).CopyTo($FatImage, 28) # Hidden sectors
[System.BitConverter]::GetBytes([uint32]0).CopyTo($FatImage, 32) # Large sectors
$FatImage[36] = 0x80 # Drive number
$FatImage[38] = 0x29 # Extended signature
[System.BitConverter]::GetBytes([uint32]0x12345678).CopyTo($FatImage, 39) # Volume ID
[System.Text.Encoding]::ASCII.GetBytes("BEAVER_BOOT").CopyTo($FatImage, 43) # Volume Label (11 bytes)
[System.Text.Encoding]::ASCII.GetBytes("FAT16   ").CopyTo($FatImage, 54) # File system type (8 bytes)
$FatImage[510] = 0x55; $FatImage[511] = 0xAA # Magic boot signature

# --- Inicializar FAT 1 y FAT 2 ---
$Fat1Offset = $ReservedSectors * $SectorSize
$Fat2Offset = ($ReservedSectors + $SectorsPerFat) * $SectorSize

# Entradas especiales 0 y 1
$FatImage[$Fat1Offset + 0] = 0xF8; $FatImage[$Fat1Offset + 1] = 0xFF
$FatImage[$Fat1Offset + 2] = 0xFF; $FatImage[$Fat1Offset + 3] = 0xFF
$FatImage[$Fat2Offset + 0] = 0xF8; $FatImage[$Fat2Offset + 1] = 0xFF
$FatImage[$Fat2Offset + 2] = 0xFF; $FatImage[$Fat2Offset + 3] = 0xFF

$RootOffset = ($ReservedSectors + ($NumFats * $SectorsPerFat)) * $SectorSize
$DataOffset = $RootOffset + ($RootSectors * $SectorSize)

# Helper para asignar clusters en FAT1 y FAT2
function Set-FatEntry([int]$Cluster, [int]$Value) {
    $off1 = $Fat1Offset + ($Cluster * 2)
    $off2 = $Fat2Offset + ($Cluster * 2)
    $b = [System.BitConverter]::GetBytes([uint16]$Value)
    $b.CopyTo($FatImage, $off1)
    $b.CopyTo($FatImage, $off2)
}

# Helper para escribir datos en clusters
$CurrentCluster = 2

function Write-FileData([byte[]]$Data) {
    $startCluster = $script:CurrentCluster
    $bytesRemaining = $Data.Length
    $dataPos = 0

    while ($bytesRemaining -gt 0) {
        $writeLen = [math]::Min($bytesRemaining, $ClusterSize)
        $clusterByteOffset = $DataOffset + (($script:CurrentCluster - 2) * $ClusterSize)
        [System.Array]::Copy($Data, $dataPos, $FatImage, $clusterByteOffset, $writeLen)

        $dataPos += $writeLen
        $bytesRemaining -= $writeLen

        if ($bytesRemaining -gt 0) {
            Set-FatEntry $script:CurrentCluster ($script:CurrentCluster + 1)
            $script:CurrentCluster++
        } else {
            Set-FatEntry $script:CurrentCluster 0xFFFF # EOF
            $script:CurrentCluster++
        }
    }
    return $startCluster
}

# 1. Escribir KERNEL.ELF
$KernelStartCluster = Write-FileData $KernelBytes

# 2. Escribir BOOTX64.EFI
$EfiStartCluster = Write-FileData $EfiBytes

# 3. Crear cluster para directorio \EFI\BOOT
$EfiBootDirCluster = $CurrentCluster
Set-FatEntry $CurrentCluster 0xFFFF
$CurrentCluster++

$EfiBootDirBytes = New-Object byte[] $ClusterSize
# '.' entry
[System.Text.Encoding]::ASCII.GetBytes(".          ").CopyTo($EfiBootDirBytes, 0)
$EfiBootDirBytes[11] = 0x10 # Directory
[System.BitConverter]::GetBytes([uint16]$EfiBootDirCluster).CopyTo($EfiBootDirBytes, 26)

# '..' entry
[System.Text.Encoding]::ASCII.GetBytes("..         ").CopyTo($EfiBootDirBytes, 32)
$EfiBootDirBytes[43] = 0x10 # Directory

# 'BOOTX64.EFI' entry
[System.Text.Encoding]::ASCII.GetBytes("BOOTX64 EFI").CopyTo($EfiBootDirBytes, 64)
$EfiBootDirBytes[75] = 0x20 # Archive
[System.BitConverter]::GetBytes([uint16]$EfiStartCluster).CopyTo($EfiBootDirBytes, 90)
[System.BitConverter]::GetBytes([uint32]$EfiBytes.Length).CopyTo($EfiBootDirBytes, 92)

# Copiar directorio \EFI\BOOT a su cluster
$dirOffset = $DataOffset + (($EfiBootDirCluster - 2) * $ClusterSize)
[System.Array]::Copy($EfiBootDirBytes, 0, $FatImage, $dirOffset, $ClusterSize)

# 4. Crear cluster para directorio \EFI
$EfiDirCluster = $CurrentCluster
Set-FatEntry $CurrentCluster 0xFFFF
$CurrentCluster++

$EfiDirBytes = New-Object byte[] $ClusterSize
# '.' entry
[System.Text.Encoding]::ASCII.GetBytes(".          ").CopyTo($EfiDirBytes, 0)
$EfiDirBytes[11] = 0x10 # Directory
[System.BitConverter]::GetBytes([uint16]$EfiDirCluster).CopyTo($EfiDirBytes, 26)

# '..' entry
[System.Text.Encoding]::ASCII.GetBytes("..         ").CopyTo($EfiDirBytes, 32)
$EfiDirBytes[43] = 0x10 # Directory

# 'BOOT' entry
[System.Text.Encoding]::ASCII.GetBytes("BOOT       ").CopyTo($EfiDirBytes, 64)
$EfiDirBytes[75] = 0x10 # Directory
[System.BitConverter]::GetBytes([uint16]$EfiBootDirCluster).CopyTo($EfiDirBytes, 90)

$dirOffset = $DataOffset + (($EfiDirCluster - 2) * $ClusterSize)
[System.Array]::Copy($EfiDirBytes, 0, $FatImage, $dirOffset, $ClusterSize)

# 5. Escribir entradas en el Root Directory
# Entrada 0: Volume Label
[System.Text.Encoding]::ASCII.GetBytes("BEAVER_OS  ").CopyTo($FatImage, $RootOffset + 0)
$FatImage[$RootOffset + 11] = 0x08 # Volume Label

# Entrada 1: KERNEL.ELF
[System.Text.Encoding]::ASCII.GetBytes("KERNEL  ELF").CopyTo($FatImage, $RootOffset + 32)
$FatImage[$RootOffset + 43] = 0x20 # Archive
[System.BitConverter]::GetBytes([uint16]$KernelStartCluster).CopyTo($FatImage, $RootOffset + 58)
[System.BitConverter]::GetBytes([uint32]$KernelBytes.Length).CopyTo($FatImage, $RootOffset + 60)

# Entrada 2: EFI directory
[System.Text.Encoding]::ASCII.GetBytes("EFI        ").CopyTo($FatImage, $RootOffset + 64)
$FatImage[$RootOffset + 75] = 0x10 # Directory
[System.BitConverter]::GetBytes([uint16]$EfiDirCluster).CopyTo($FatImage, $RootOffset + 90)

# Guardar imagen raw de partición / disco USB ($OutputImg)
[System.IO.File]::WriteAllBytes($OutputImg, $FatImage)
$ImgSizeMb = [math]::Round((Get-Item $OutputImg).Length / 1MB, 2)
Write-Host "[OK] Imagen de disco USB generada: $OutputImg ($ImgSizeMb MB)" -ForegroundColor Green

# 3. Empaquetar ISO 9660 con Catálogo de Arranque El Torito UEFI
Write-Host "[4/4] Empaquetando imagen ISO 9660 El Torito Bootable ($OutputIso)..." -ForegroundColor Yellow

$IsoSectorSize = 2048
$FatImgIsoSectors = [int][math]::Ceiling($FatImage.Length / $IsoSectorSize)
$TotalIsoSectors = 20 + $FatImgIsoSectors
$IsoBytes = New-Object byte[] ($TotalIsoSectors * $IsoSectorSize)

# --- Sector 16: Primary Volume Descriptor (PVD) ---
$PvdOffset = 16 * $IsoSectorSize
$IsoBytes[$PvdOffset + 0] = 0x01 # Type: PVD
[System.Text.Encoding]::ASCII.GetBytes("CD001").CopyTo($IsoBytes, $PvdOffset + 1)
$IsoBytes[$PvdOffset + 6] = 0x01 # Version
[System.Text.Encoding]::ASCII.GetBytes("BEAVER_OS                      ").CopyTo($IsoBytes, $PvdOffset + 40) # Volume ID (32 chars)
[System.BitConverter]::GetBytes([uint32]$TotalIsoSectors).CopyTo($IsoBytes, $PvdOffset + 80)
# Both-endian total sectors
$IsoBytes[$PvdOffset + 84] = [byte](($TotalIsoSectors -shr 24) -band 0xFF)
$IsoBytes[$PvdOffset + 85] = [byte](($TotalIsoSectors -shr 16) -band 0xFF)
$IsoBytes[$PvdOffset + 86] = [byte](($TotalIsoSectors -shr 8) -band 0xFF)
$IsoBytes[$PvdOffset + 87] = [byte]($TotalIsoSectors -band 0xFF)
[System.BitConverter]::GetBytes([uint16]1).CopyTo($IsoBytes, $PvdOffset + 120) # Volume set size
[System.BitConverter]::GetBytes([uint16]1).CopyTo($IsoBytes, $PvdOffset + 124) # Volume sequence number
[System.BitConverter]::GetBytes([uint16]$IsoSectorSize).CopyTo($IsoBytes, $PvdOffset + 128) # Logical block size
# Root directory record (34 bytes en offset 156)
$IsoBytes[$PvdOffset + 156] = 34 # Length
$IsoBytes[$PvdOffset + 158] = 19 # Root directory location (LBA 19)
$IsoBytes[$PvdOffset + 166] = [byte]($IsoSectorSize -band 0xFF)
$IsoBytes[$PvdOffset + 167] = [byte]($IsoSectorSize -shr 8)
$IsoBytes[$PvdOffset + 181] = 0x02 # Directory flag

# --- Sector 17: Boot Record Volume Descriptor (El Torito) ---
$BootDescOffset = 17 * $IsoSectorSize
$IsoBytes[$BootDescOffset + 0] = 0x00 # Type: Boot Record
[System.Text.Encoding]::ASCII.GetBytes("CD001").CopyTo($IsoBytes, $BootDescOffset + 1)
$IsoBytes[$BootDescOffset + 6] = 0x01 # Version
[System.Text.Encoding]::ASCII.GetBytes("EL TORITO SPECIFICATION").CopyTo($IsoBytes, $BootDescOffset + 7)
# Puntero al Boot Catalog en LBA 19
[System.BitConverter]::GetBytes([uint32]19).CopyTo($IsoBytes, $BootDescOffset + 71)

# --- Sector 18: Volume Descriptor Set Terminator ---
$TermOffset = 18 * $IsoSectorSize
$IsoBytes[$TermOffset + 0] = 0xFF # Type: Terminator
[System.Text.Encoding]::ASCII.GetBytes("CD001").CopyTo($IsoBytes, $TermOffset + 1)
$IsoBytes[$TermOffset + 6] = 0x01 # Version

# --- Sector 19: Boot Catalog El Torito (32 bytes Validation + 32 bytes Initial + 32 bytes EFI Section) ---
$CatalogOffset = 19 * $IsoSectorSize

# Entrada 1: Validation Entry (32 bytes)
$IsoBytes[$CatalogOffset + 0] = 0x01 # Header ID
$IsoBytes[$CatalogOffset + 1] = 0x00 # Platform: x86
[System.Text.Encoding]::ASCII.GetBytes("Beaversoft").CopyTo($IsoBytes, $CatalogOffset + 4) # Manufacturer
$IsoBytes[$CatalogOffset + 30] = 0x55
$IsoBytes[$CatalogOffset + 31] = 0xAA
# Checksum (suma de palabras de 16 bits = 0)
$csum = 0
for ($i = 0; $i -lt 32; $i += 2) {
    if ($i -ne 28) {
        $w = [System.BitConverter]::ToUInt16($IsoBytes, $CatalogOffset + $i)
        $csum = ($csum + $w) -band 0xFFFF
    }
}
$csumVal = (0x10000 - $csum) -band 0xFFFF
[System.BitConverter]::GetBytes([uint16]$csumVal).CopyTo($IsoBytes, $CatalogOffset + 28)

# Entrada 2: Initial / Default Entry (32 bytes, offset 32)
$IsoBytes[$CatalogOffset + 32] = 0x88 # Bootable
$IsoBytes[$CatalogOffset + 33] = 0x00 # Media: No Emulation
[System.BitConverter]::GetBytes([uint16]0).CopyTo($IsoBytes, $CatalogOffset + 34) # Load segment
$IsoBytes[$CatalogOffset + 36] = 0x00 # System type: x86
$FatSectors512 = [uint16]($FatImage.Length / 512)
[System.BitConverter]::GetBytes([uint16]4).CopyTo($IsoBytes, $CatalogOffset + 38) # Sector count
[System.BitConverter]::GetBytes([uint32]20).CopyTo($IsoBytes, $CatalogOffset + 40) # Load RBA: Sector 20

# Entrada 3: Section Header for EFI (32 bytes, offset 64)
$IsoBytes[$CatalogOffset + 64] = 0x91 # Final section header for EFI
$IsoBytes[$CatalogOffset + 65] = 0xEF # Platform ID: EFI Architecture (0xEF)
[System.BitConverter]::GetBytes([uint16]1).CopyTo($IsoBytes, $CatalogOffset + 66) # 1 section entry

# Entrada 4: Section Entry for EFI (32 bytes, offset 96)
$IsoBytes[$CatalogOffset + 96] = 0x88 # Bootable
$IsoBytes[$CatalogOffset + 97] = 0x00 # Media: No Emulation
[System.BitConverter]::GetBytes([uint16]0).CopyTo($IsoBytes, $CatalogOffset + 98) # Load segment
[System.BitConverter]::GetBytes([uint16]1).CopyTo($IsoBytes, $CatalogOffset + 102) # Sector count
[System.BitConverter]::GetBytes([uint32]20).CopyTo($IsoBytes, $CatalogOffset + 104) # Load RBA: Sector 20

# --- Sector 20 en adelante: Carga útil de la Partición EFI FAT ---
$FatPayloadOffset = 20 * $IsoSectorSize
[System.Array]::Copy($FatImage, 0, $IsoBytes, $FatPayloadOffset, $FatImage.Length)

# Escribir archivo ISO final
[System.IO.File]::WriteAllBytes($OutputIso, $IsoBytes)
$IsoSizeMb = [math]::Round((Get-Item $OutputIso).Length / 1MB, 2)

Write-Host "========================================================================" -ForegroundColor Green
Write-Host " [EXITO] MEDIOS DE INSTALACION GENERADOS SATISFACTORIAMENTE" -ForegroundColor Yellow
Write-Host "   * Imagen ISO Hibrida: $OutputIso ($IsoSizeMb MB)" -ForegroundColor White
Write-Host "   * Imagen Raw USB/GPT: $OutputImg ($ImgSizeMb MB)" -ForegroundColor White
Write-Host ""
Write-Host " Puedes probar la ISO inmediatamente en QEMU ejecutando:" -ForegroundColor Cyan
Write-Host "   .\run.ps1 -Iso" -ForegroundColor White
Write-Host ""
Write-Host " Para grabar en una memoria USB real (Pendrive):" -ForegroundColor Cyan
Write-Host "   - Con Rufus: Seleccionar '$OutputIso' (Modo GPT / UEFI)" -ForegroundColor DarkGray
Write-Host "   - Con BalenaEtcher: Seleccionar '$OutputIso' o '$OutputImg'" -ForegroundColor DarkGray
Write-Host "   - O ejecutar: .\create_usb.ps1" -ForegroundColor DarkGray
Write-Host "========================================================================" -ForegroundColor Green
