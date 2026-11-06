# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Product: Beaver OS
# Document: Official Release Notes - Version 0.9.0 (Release Candidate 1)
# ============================================================================

# 🦫 Beaver OS v0.9.0 — Notas Oficiales de Lanzamiento
**Versión:** 0.9.0 (Release Candidate 1)  
**Fecha de Publicación:** 5 de Noviembre de 2026  
**Arquitectura:** x86_64 Bare-Metal (`no_std`)  
**Firma / Marca:** Beaversoft  
**Autor Principal & Arquitecto de Software:** José Alberto Puente Maldonado (`japuentem`)  
**Licencia:** Source-Available con Protección Estricta de Marca y Renombrado Obligatorio ([`LICENSE`](../LICENSE))  

---

## 🌟 Resumen Ejecutivo

**Beaver OS** es un sistema operativo moderno de propósito general y diseño de microkernel resiliente, escrito desde cero en **Rust bare-metal (`no_std`)**. Desarrollado con el objetivo de erradicar los clásicos fallos catastróficos de los sistemas operativos convencionales (como pantallas azules o kernel panics por drivers defectuosos), Beaver OS introduce una arquitectura tolerante a fallos con aislamiento estricto de privilegios por hardware, almacenamiento con auto-recuperación ante cortes abruptos de energía y controladores de hardware directos sin capas de abstracción innecesarias ni telemetría opaca.

La versión **v0.9.0 (Release Candidate 1)** marca la culminación de todas las fases del ciclo de desarrollo del meta-kernel, entregando una distribución híbrida booteable lista para despliegue en entornos virtuales (QEMU/KVM, VirtualBox, VMware) y hardware físico x86_64 moderno (UEFI) y clásico (BIOS).

---

## 📦 Medios de Distribución e Integridad Criptográfica

Las imágenes oficiales han sido generadas y certificadas mediante compilación reproducible en modo release (`x86_64-unknown-none`).

| Archivo de Imagen | Tamaño | Formato | Suma de Comprobación Criptográfica (SHA-256) |
|---|---|---|---|
| `beaver-os-v0.9.0.iso` | 16,818,176 bytes (16.04 MB) | ISO 9660 / El Torito UEFI Híbrido | `1BDFBC43C7EE32B12C76EC6C179422188079767175B47777AD2C884F7E1C5AEA` |
| `beaver-os-v0.9.0.img` | 16,777,216 bytes (16.00 MB) | Imagen cruda de disco (GPT / UEFI ESP) | `9467D933660DB0E7C767276C8FB364D0C0EA0BF5A3E02DFFC02D31DEF3D4AD27` |

> **Verificación en PowerShell:**
> ```powershell
> Get-FileHash target\beaver-os-v0.9.0.iso -Algorithm SHA256
> Get-FileHash target\beaver-os-v0.9.0.img -Algorithm SHA256
> ```

---

## 🚀 Principales Características y Novedades de la Versión 0.9.0

### 1. Aislamiento Real de Procesos en Espacio de Usuario (Ring 3)
- **Modo No Privilegiado (CPL=3):** Separación total de privilegios entre el supervisor del kernel y las aplicaciones de usuario.
- **Transición MSR por Hardware:** Implementación de la instrucción rápida `syscall` y retorno `sysret` configurando el registro específico del modelo `IA32_LSTAR` (`0xC0000082`) junto con `IA32_STAR` y `IA32_FMASK`.
- **Directorio de Páginas CR3 Aislado:** Paginación de 4 niveles (`PML4`) dedicada para procesos de usuario, con mapeo en modo usuario (`UserSupervisor = 1`) y protección contra lectura/escritura en la región alta del kernel.
- **Pila de Usuario Independiente:** Pila separada de 2 MiB en espacio no privilegiado y conmutación de contexto libre de carreras.
- **Shell Nativo en Ring 3 (`sh`):** Intérprete interactivo que opera completamente en modo no privilegiado, interactuando con el hardware exclusivamente a través de la pasarela MSR Syscall. Un error o caída en el shell nunca compromete la estabilidad del kernel supervisor.
- **Pre-empaquetado de Binarios ELF64 de Usuario:** La partición de arranque y el sistema de archivos incluyen ejecutables nativos x86-64 preinstalados (`\BIN\HELLO.ELF`, `\BIN\BENCH.ELF`, `\BIN\SH.ELF` y `/disk/bin/*.elf`), compilados con `beaver-libc` y ejecutables directamente en modo no privilegiado mediante `syscall`.
- **Motor de BeaverScript (.bs) Aislado en Ring 3:** Los scripts nativos (`/disk/*.bs` como `demo.bs`, `functions.bs`, `fibonacci.bs`) se ejecutan dentro de un sandbox aislado en modo usuario (CPL=3) con directorio CR3 dedicado, canalizando todas sus operaciones de E/S (`print`, `println`, `beep`, `file_write`) a través de llamadas al sistema MSR rápidas (`SYS_WRITE`, `SYS_BEEP`, `SYS_OPEN`, `SYS_CLOSE`, `SYS_EXIT`) con protección Anti-BSOD.

### 2. Sistema de Archivos Resiliente BeaverFS
- **Arquitectura de Partición Doble A/B:** Soporte nativo para arranques redundantes A/B con espejo de paridad para recuperación inmediata ante actualizaciones fallidas.
- **Inmunidad contra Cortes de Energía:** Registro transaccional seguro y motor de auto-reparación del Sector Cero (`Zero-Sector Parity Check`) que restaura automáticamente tablas de asignación o inodos corruptos.
- **Capa VFS Unificada:** Soporte para montajes virtuales (`/dev`, `/proc`, `/sys`, `/bin`, `/disk`) y abstracción POSIX (`open`, `read`, `write`, `close`, `stat`).

### 3. Controladores de Hardware Bare-Metal Reales (Zero Mocks)
- **Almacenamiento SATA AHCI:**
  - Controlador nativo AHCI (PCI ID clase `0x01`, subclase `0x06`).
  - Transferencias DMA en memoria física utilizando listas PRDT (*Physical Region Descriptor Tables*) y puertos HBAMemory.
- **Red Gigabit Intel 82540EM (e1000):**
  - Configuración directa de registros MMIO del controlador Intel Gigabit.
  - Anillos circulares de descriptores de transmisión (Tx) y recepción (Rx).
  - Manejo de interrupciones deterministas en bus PCI.
- **Subsistema de Video VBE / Framebuffer TrueColor:**
  - Soporte de resolución 1024x768 a 32 bits de profundidad por pixel (TrueColor BGR/RGBA).
  - Compositor de ventanas de doble búfer (*double-buffering*) con refresco sin parpadeo (*tear-free rendering*).

### 4. Pila de Red Completa BeaverNet (TCP/IP)
- **Protocolos Soportados:** Ethernet II, ARP con resolución de direcciones físicas por broadcast, IPv4 con fragmentación/ensamblado e ICMP Echo (*ping* de baja latencia).
- **Socket TCP Confiable:** Handshake de tres vías (`SYN` -> `SYN-ACK` -> `ACK`), control de flujo, ventanas de recepción y desconexión limpia (`FIN`/`RST`).
- **Cliente HTTP/1.1 Nativo (`curl`):** Capacidad de ejecutar peticiones HTTP directas hacia servidores LAN e Internet y procesar cabeceras y payloads JSON.
- **Malla BeaverSync:** Protocolo descentralizado entre nodos Beaver OS para sincronización de telemetría y estado distribuido sin servidor central.

### 5. Entorno Gráfico y Aplicaciones Nativas: Beaver Desktop
- **Escritorio Multiventana:** Gestor de ventanas con arrastre dinámico, minimización, barras de título y cursor de ratón PS/2 acelerado.
- **Suite de Aplicaciones Oficiales:**
  - **Terminal Interactivo:** Consola gráfica integrada con canalización interactiva de teclado PS/2, buffer de 128 caracteres, historial de comandos accesible con Flechas Arriba/Abajo, autocompletado mediante tecla TAB y ejecución directa de binarios en Ring 3 (`exec`, `sh`).
  - **BeaverFiles:** Explorador visual de carpetas y archivos en particiones BeaverFS.
  - **BeaverNote:** Bloc de notas de texto plano para edición rápida.
  - **BeaverAudio:** Monitor de reproducción de sonido y sintetizador de ondas.
  - **BeaverSync:** Panel de control de nodos y tráfico de red en malla.
  - **BeaverPhotos:** Visor de imágenes en formato mapa de bits nativo.
  - **BeaverCalendar:** Calendario y cronómetro de sistema sincronizado por RTC.

### 6. Beaver Shell y Telemetría en Tiempo Real
- **Banner Tipográfico de Inicio:** Nuevo banner Figlet estandarizado a 64 columnas con los colores institucionales (Cian Beaversoft y Amarillo Ámbar).
- **Consola Interactiva:** Historial de comandos, autocompletado, ejecución de scripts BeaverScript (`.bs`), monitores de memoria, procesos (`ps`), red (`net`, `ping`, `curl`) y diagnóstico de hardware (`sys`, `lspci`, `lscpu`).

---

## 🧪 Certificación y Resultados de la Suite de Pruebas

El sistema operativo incluye una suite automatizada de pruebas end-to-end ejecutadas en QEMU mediante emulación de hardware completa (`powershell -ExecutionPolicy Bypass -File run.ps1 -Test`):

```text
============================================================
BEAVER OS v0.9.0 - SUITE AUTOMATIZADA DE PRUEBAS END-TO-END
============================================================
[1/16] GDT & IDT Ring 0/Ring 3 Segments ............ [PASS]
[2/16] Interrupt Controller (PIC/APIC) ............. [PASS]
[3/16] Paging & Virtual Memory Manager ............. [PASS]
[4/16] Dynamic Heap Allocator (Buddy/Slab) ......... [PASS]
[5/16] Task Scheduler & Context Switching .......... [PASS]
[6/16] Ring 3 Userland & MSR Syscall Gateway ....... [PASS]
[7/16] POSIX Syscall Bridge (Read/Write/Exit) ...... [PASS]
[8/16] PCI Bus Enumeration ......................... [PASS]
[9/16] AHCI SATA Controller & DMA Transfers ........ [PASS]
[10/16] BeaverFS Dual-Slot & Inode Operations ...... [PASS]
[11/16] Zero-Sector Parity Auto-Recovery ........... [PASS]
[12/16] Intel E1000 NIC MMIO & Descriptors ......... [PASS]
[13/16] BeaverNet TCP/IP Handshake & ICMP .......... [PASS]
[14/16] VBE Framebuffer Compositor & Desktop ....... [PASS]
[15/16] BeaverScript Engine & Interpreter .......... [PASS]
[16/16] Multi-Tasking SMP Core Telemetry ........... [PASS]
------------------------------------------------------------
RESULTADO FINAL: 16 de 16 Pruebas Superadas (100% de Éxito)
Código de Salida: 33 (Éxito limpio en modo de pruebas)
============================================================
```

---

## 🛠 Requisitos Mínimos y Recomendados

| Componente | Requisito Mínimo | Requisito Recomendado |
|---|---|---|
| **Procesador** | CPU x86_64 compatible con AMD64 / Intel 64 (1 núcleo) | CPU x86_64 Multinúcleo (2+ núcleos con soporte APIC) |
| **Memoria RAM** | 128 MiB RAM | 512 MiB RAM o superior |
| **Firmware** | BIOS tradicional o UEFI v2.0+ (Secure Boot desactivado) | UEFI x86_64 nativo |
| **Almacenamiento** | Medio USB de 64 MiB o disco SATA AHCI | Disco SATA SSD o unidad flash USB 3.0 |
| **Gráficos** | Adaptador VGA estándar (80x25 modo texto) | Adaptador VBE / VESA compatible con 1024x768 a 32bpp |
| **Red** | Opcional | Intel 82540EM / 82545EM / 82574L Gigabit Ethernet |

---

## 👥 Reconocimientos y Autoría

- **Desarrollador y Arquitecto Principal:** José Alberto Puente Maldonado (`japuentem`)
- **Marca y Firma Creadora:** Beaversoft
- **Comunidad y Soporte:** [GitHub Repository / Beaversoft Meta-Kernel](https://github.com/japuentem/beaversoft-kernel)

*Beaver OS v0.9.0 — Construyendo sistemas operativos seguros, deterministas y libres de caídas.*

