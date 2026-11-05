# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Description: Guía Definitiva de Creación de Medios e Instalación en Hardware Real (Bare-Metal)
# ============================================================================

# Beaver OS — Guía de Instalación en Hardware Real (Bare-Metal)
**Versión:** 0.9.0 (Production Hardening)  
**Autor:** José Alberto Puente Maldonado (`japuentem`)  
**Marca:** Beaversoft  
**Arquitectura Objetivo:** x86_64 Long Mode (UEFI Native + GOP Framebuffer + AHCI SATA DMA)

---

## 1. Resumen Ejecutivo

**Beaver OS** es un micro-kernel de 64 bits diseñado desde cero en Rust (`no_std`) para ejecutarse directamente sobre el hardware físico (*bare-metal*) sin capas de emulación intermedias. 

A partir de la versión **0.9.0**, el sistema incorpora un cargador UEFI nativo de 64 bits ([`BOOTX64.EFI`](file:///d:/proyectos_personales/beaversoft-kernel/target/BOOTX64.EFI)), controlador de almacenamiento **AHCI SATA DMA**, pila de red **TCP/IP** con servidor HTTP en Ring 0, y un **Espacio de Usuario aislado en Ring 3** con biblioteca estándar [`beaver-libc`](file:///d:/proyectos_personales/beaversoft-kernel/user/beaver-libc/) y pasarela de syscalls POSIX.

Esta guía detalla los pasos para generar la imagen ISO/USB híbrida, transferirla a un pendrive físico, configurar la BIOS/UEFI de cualquier PC o laptop x86_64 moderna e iniciar el sistema.

---

## 2. Generación de Medios de Instalación

El repositorio incluye herramientas automatizadas para generar los binarios de instalación listos para hardware físico:

### A. Compilación y Generación Automática
Ejecuta en una terminal PowerShell con privilegios de usuario estándar:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\build_iso.ps1
```

Este script compila el kernel en modo *release* y produce dos archivos en el directorio `target\`:
1. **`target\beaver-os-v0.9.0.iso`**: Imagen ISO 9660 híbrida con Catálogo de Arranque El Torito UEFI (Plataforma `0xEF`). Compatible con grabadores de CD/DVD y herramientas de USB booteable.
2. **`target\beaver-os-v0.9.0.img`**: Imagen de disco en bloque sin procesar (*raw disk image*) con sistema de archivos FAT16/GPT, conteniendo la estructura `\EFI\BOOT\BOOTX64.EFI` y `\KERNEL.ELF`.

---

## 3. Grabación de la Memoria USB (Pendrive)

Cualquier memoria USB de **64 MB o superior** es adecuada. Dispones de cuatro métodos según tu preferencia:

### Método 1: Script Automatizado de Beaver OS (Recomendado en Windows)
Conecta tu memoria USB a la computadora y ejecuta:
```powershell
powershell -ExecutionPolicy Bypass -File create_usb.ps1
```
El script detectará automáticamente los dispositivos USB conectados, solicitará confirmación del número de unidad y desplegará la estructura UEFI de arranque de forma directa.

### Método 2: Rufus (Windows)
1. Conecta tu memoria USB y abre **Rufus** (versión 3.15 o posterior).
2. En **Dispositivo**, selecciona tu pendrive USB.
3. En **Elección de arranque**, pulsa *Seleccionar* y elige `target\beaver-os-v0.9.0.iso`.
4. Configura:
   - **Esquema de partición:** `GPT`.
   - **Sistema de destino:** `UEFI (no CSM)`.
   - **Sistema de archivos:** `FAT32` o `FAT16`.
5. Pulsa **Empezar** en modo de escritura recomendado.

### Método 3: BalenaEtcher (Windows, macOS o Linux)
1. Abre BalenaEtcher.
2. Haz clic en **Flash from file** y selecciona `target\beaver-os-v0.9.0.iso` o `target\beaver-os-v0.9.0.img`.
3. Selecciona tu unidad USB como destino.
4. Haz clic en **Flash!**.

### Método 4: Comando `dd` (Linux o macOS)
```bash
sudo dd if=target/beaver-os-v0.9.0.img of=/dev/sdX bs=4M status=progress conv=fsync
```
*(Reemplaza `/dev/sdX` con el identificador exacto de tu memoria USB verificado con `lsblk`)*.

---

## 4. Configuración del Firmware de la Computadora (BIOS/UEFI)

Para arrancar Beaver OS en una PC o laptop física, reinicia el equipo e ingresa al menú de configuración del firmware (usualmente presionando `F2`, `F12`, `Del` o `Esc` durante el encendido):

1. **Modo de Arranque (Boot Mode):**
   - Configura en **UEFI Native** (desactiva *Legacy BIOS* o *CSM - Compatibility Support Module*).
2. **Arranque Seguro (Secure Boot):**
   - Configura **Secure Boot** en **Disabled** (desactivado) o modo *Setup/Custom*. Dado que Beaver OS cuenta con su propio cargador bare-metal sin firma comercial de Microsoft, requiere que el Secure Boot esté desactivado durante las pruebas.
3. **Modo de Controlador SATA / Almacenamiento (SATA Operation):**
   - Asegúrate de que el controlador de disco esté en modo **AHCI** (no *RAID On* ni *IDE*). Esto permite que el controlador DMA de Beaver OS acceda directamente a los puertos SATA y discos NVMe/SSD con soporte SATA.
4. **Orden de Arranque (Boot Priority):**
   - Selecciona la memoria USB como primera opción de arranque (e.g. *UEFI: USB Flash Drive*).
5. Guarda los cambios (`F10`) y reinicia.

---

## 5. Proceso de Arranque y Verificación de Hardware en Vivo

Al arrancar desde el medio USB, el cargador [`boot\uefi\src\main.rs`](file:///d:/proyectos_personales/beaversoft-kernel/boot/uefi/src/main.rs):
1. Negocia con el firmware UEFI la resolución óptima de video mediante el protocolo **GOP** (Graphics Output Protocol), obteniendo el framebuffer lineal en memoria física.
2. Recupera el mapa de memoria del sistema (`GetMemoryMap`), registrando las regiones de RAM utilizables.
3. Carga el ejecutable `KERNEL.ELF`, establece la paginación de 4 niveles en modo largo x86_64 y transfiere el control a `kernel_main`.

### Verificaciones en la Consola Interactiva (Beaver Shell):

Una vez iniciado el sistema en la pantalla de bienvenida con el prompt `root@beaver:/#`:

1. **Verificar Memoria RAM Real y Topología SMP:**
   ```text
   root@beaver:/# memmap
   root@beaver:/# cpuinfo
   ```
   *Medición:* Muestra todos los núcleos de CPU descubiertos en la tabla ACPI MADT y los marcos físicos administrados por el `FRAME_ALLOCATOR`.

2. **Verificar Almacenamiento SATA AHCI:**
   ```text
   root@beaver:/# storage
   root@beaver:/# disks
   ```
   *Medición:* Muestra los puertos SATA AHCI activos, la velocidad de enlace (SATA 3.0 / 6 Gbps) y los discos duros o unidades SSD conectadas.

3. **Verificar Red y Conectividad Local:**
   ```text
   root@beaver:/# net
   root@beaver:/# curl http://beaver.lan/info.json
   ```
   *Medición:* Consulta la tarjeta de red (E1000 o compatible), IP asignada, y efectúa la petición TCP HTTP de telemetría a través del loopback del kernel.

4. **Entrar al Espacio de Usuario Aislado en Ring 3:**
   ```text
   root@beaver:/# sh
   ```
   *Medición:* Conmuta el procesador a nivel de privilegio `CPL=3` con espacio de direcciones `CR3` aislado, presentando el prompt `beaver-sh (Ring 3)$ `. En este entorno puedes ejecutar:
   - `uptime` (Syscall `SYS_UPTIME`).
   - `whoami` (Syscall `SYS_GETPID`).
   - `echo <mensaje>` (Syscall `SYS_WRITE`).
   - `cat /system/motd` (Syscalls `SYS_OPEN`, `SYS_READ`, `SYS_CLOSE`).
   - `exit` (Termina el shell de usuario y regresa al supervisor).

---

## 6. Instalador Universal Bare-Metal (`install`)

Para instalar Beaver OS de manera permanente en el disco interno de la computadora:

1. Ejecuta en Beaver Shell:
   ```text
   root@beaver:/# install
   ```
2. El instalador universal (`kernel/src/storage/installer.rs`):
   - Escaneará la unidad de disco primaria detectada por el controlador AHCI DMA.
   - Creará la tabla de particiones **GPT** con esquema de alta resiliencia **A/B**:
     - Ranura A: Sistema Activo.
     - Ranura B: Sistema Espejo / Recuperación.
   - Formateará el volumen con **BeaverFS**, aplicando el árbol de directorios `/system`, `/disk`, `/net`, `/proc` y `/home`.
   - Registrará la firma anti-brickeo en la partición supervisor.
3. Al finalizar, retira la memoria USB y reinicia el equipo: tu computadora arrancará directamente Beaver OS desde el almacenamiento interno.

