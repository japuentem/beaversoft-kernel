# Guía de Arranque UEFI en Hardware Real — Beaver OS
**Proyecto:** Beaversoft Meta-Kernel  
**Autor:** José Alberto Puente Maldonado (japuentem)  
**Marca:** Beaversoft  
**Arquitectura:** x86_64 UEFI Native (GOP Framebuffer + 64-bit Long Mode)

---

## 1. Visión General y Arquitectura

Beaver OS cuenta con un cargador de arranque universal nativo para firmware **UEFI x86_64** (`BOOTX64.EFI`). 

A diferencia de los sistemas operativos convencionales que requieren cargadores de múltiples etapas (Stage 1, Stage 2, GRUB2, Shim, etc.), Beaver OS implementa un cargador monolítico autónomo de alto rendimiento:

1. **Protocolo GOP (Graphics Output Protocol):** Detecta la resolución nativa de la pantalla de la computadora o laptop (por ejemplo 1024x768 o superior en 32 bits BGR/RGB) y pasa la dirección física del Linear Framebuffer (`fb_base`, `fb_size`, `fb_pitch`) al kernel mediante la estructura `UefiBootInfo` en la dirección física fija `0x8000`.
2. **Carga y Desempaquetado ELF64:** Asigna páginas de memoria convencional física en `0x100000` (1 MB) y mapea los segmentos `PT_LOAD` del kernel Beaver compilado en Rust (`beaversoft-kernel`).
3. **Firma y Handshake `BEAVUEFI`:** Valida la firma de 64 bits en el código del kernel y extrae el punto de entrada directo de 64 bits (`uefi_entry`).
4. **Cierre de Servicios de Firmware (`ExitBootServices`):** Finaliza la capa de firmware de la placa madre y transfiere el control absoluto de la CPU y la memoria al kernel en **Modo Largo de 64 bits (Ring 0)**.

---

## 2. Compatibilidad de Hardware

| Componente | Requisito Mínimo | Estado |
| :--- | :--- | :--- |
| **Procesador** | Intel (Core 2 Duo en adelante, Core i3/i5/i7/i9, Xeon) o AMD (Athlon 64, Phenom, Ryzen, EPYC) de 64 bits. | ✅ 100% Compatible |
| **Firmware** | UEFI 2.0 o superior (prácticamente cualquier computadora fabricada desde 2011 a la fecha). | ✅ Estándar UEFI x86_64 |
| **Memoria RAM** | 512 MB o más. | ✅ 2 MB Kernel Heap + Paginación 4-Niveles |
| **Gráficos** | Cualquier adaptador Intel HD/UHD/Iris, AMD Radeon o NVIDIA con soporte UEFI GOP. | ✅ Linear Framebuffer |
| **Almacenamiento** | Controlador SATA/IDE en modo Compatible/IDE o AHCI básico para BeaverFS. | ✅ Bus ATA Primario LBA 28/48 |
| **Entrada** | Teclado PS/2 o Teclado USB con emulación de BIOS ("Legacy USB Support"). Ratón PS/2 o touchpad compatible PS/2. | ✅ IRQ 1 (Teclado) + IRQ 12 (Ratón) |

---

## 3. Preparación de la Memoria USB

### Método 1: Automatizado con PowerShell (Recomendado)
Abre una terminal de PowerShell en Windows como Administrador dentro de la carpeta del proyecto y ejecuta:

```powershell
.\create_usb.ps1
```

El script realizará automáticamente:
1. Compilación del kernel en modo `release`.
2. Generación del binario UEFI `BOOTX64.EFI`.
3. Detección y lista interactiva de memorias USB conectadas (con protecciones de seguridad para no tocar discos del sistema).
4. Formateo opcional en FAT32 con etiqueta `BEAVER_OS`.
5. Copia de la estructura estándar de arranque: `X:\EFI\BOOT\BOOTX64.EFI`.

### Método 2: Preparación Manual
Si prefieres hacerlo manualmente en cualquier sistema:
1. Conecta tu memoria USB y formatéala en formato **FAT32** (o FAT16).
2. Crea la siguiente estructura de carpetas:
   ```
   E:\
   └── EFI\
       └── BOOT\
           └── BOOTX64.EFI
   ```
3. Copia el archivo `target\BOOTX64.EFI` a `E:\EFI\BOOT\BOOTX64.EFI`.

---

## 4. Configuración del BIOS / UEFI en la Computadora Real

Para garantizar que la placa madre permita arrancar sistemas operativos independientes, asegúrate de configurar los siguientes parámetros en la pantalla de configuración del BIOS/UEFI (presionando `F2`, `Supr` o `F10` al encender):

1. **Secure Boot (Arranque Seguro):** **DESACTIVAR (Disabled)**.  
   *Motivo:* Secure Boot bloquea cualquier binario que no contenga la firma criptográfica comercial de Microsoft Corporation. Beaver OS es un sistema de código abierto independiente.
2. **Boot Mode:** **UEFI** (o *UEFI Native* / *UEFI without CSM*).
3. **Fast Boot (Arranque Rápido):** **Desactivar (Disabled)** (permite que los puertos USB se inicialicen a tiempo antes de arrancar).
4. **Legacy USB Support:** **Habilitar (Enabled)** (permite que el teclado USB responda inmediatamente).

---

## 5. Teclas de Acceso al Boot Menu según Fabricante

Al encender el equipo o laptop, presiona repetidamente la tecla correspondiente para abrir la lista de dispositivos de arranque:

| Fabricante | Tecla de Boot Menu | Tecla de Entrada a BIOS Setup |
| :--- | :--- | :--- |
| **Dell / Alienware** | `F12` | `F2` |
| **HP (Hewlett-Packard)** | `F9` o `Esc` | `F10` |
| **Lenovo / ThinkPad** | `F12` o `Fn + F12` | `F1` o `Enter` |
| **ASUS** | `F8` o `Esc` | `Supr` (Delete) o `F2` |
| **Acer** | `F12` *(Habilitar en BIOS primero)* | `F2` |
| **Toshiba / Dynabook** | `F12` | `F2` |
| **MSI** | `F11` | `Supr` (Delete) |
| **Gigabyte / AORUS** | `F12` | `Supr` (Delete) |
| **Intel NUC** | `F10` | `F2` |
| **Microsoft Surface** | Mantener `Vol -` y pulsar `Power` | Mantener `Vol +` y pulsar `Power` |

En la lista desplegada, selecciona la opción que dice **"UEFI: Beaver OS"** o el nombre de tu memoria USB bajo la sección UEFI.

---

## 6. Verificación Local en QEMU con Firmware UEFI OVMF

Puedes probar exactamente el mismo flujo de arranque UEFI que ocurrirá en una computadora real sin salir de tu entorno de desarrollo ejecutando:

```powershell
.\run.ps1 -Uefi
```

Este comando inicia QEMU cargando el firmware de código abierto **OVMF (EDK2 x86_64)**, monta el directorio virtual FAT con `BOOTX64.EFI`, e inicializa Beaver OS en modo nativo UEFI.

