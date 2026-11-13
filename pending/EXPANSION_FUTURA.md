# Beaver OS — Hoja de Ruta y Expansión Futura
**Marca:** Beaversoft  
**Producto:** Beaver OS (`beaver-kernel`)  
**Autor:** José Alberto Puente Maldonado (@japuentem)  
**Fecha:** 2026-11-12  
**Estado:** Planificación de Arquitectura (Post-v0.9.5)

---

## 1. Emulación y Controladores para GPU Acelerada 3D (VirtIO-GPU / DRM / KMS)

### Contexto Actual:
Beaver OS cuenta actualmente con un servidor gráfico institucional (**Desktop Compositor**) operando sobre búferes lineales de píxeles (`VBE Framebuffer` a 1024x768x32bpp) con doble búfer por software, regiones sucias (*dirty rects*) y composición por ventanas mediante llamadas al sistema IPC.

### Objetivos de la Expansión:
1. **Controlador VirtIO-GPU (Subsystem 0x1050 en Bus PCI):**
   - Implementación del protocolo de comandos VirtIO-GPU 3D (Virgl / Gallium3D pass-through).
   - Manejo de colas de comandos `controlq` y `cursorq` para actualización de puntero por hardware independiente del refresco de pantalla.
   - Creación y gestión de recursos 2D/3D en memoria host (`VIRTIO_GPU_CMD_RESOURCE_CREATE_2D`, `VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D`, `VIRTIO_GPU_CMD_RESOURCE_FLUSH`).
2. **Infraestructura DRM (Direct Rendering Manager) y KMS (Kernel Mode Setting):**
   - Abstracción de planos (*planes*), conectores (*connectors*), codificadores (*encoders*) y CRTCs para cambio dinámico de resolución nativa (Full HD 1080p, 2K y 4K UHD).
   - Gestión de búferes de memoria de video compartidos mediante pasarela **DMA-BUF / GEM** (*Graphics Execution Manager*) con soporte de páginas mapeadas por demanda en Ring 3.
   - Sincronización vertical por hardware (*VSync interrupt*) ligada a los timers de alta resolución para garantizar 60 FPS estables sin tearing ni sobrecarga de CPU.

---

## 2. Portabilidad a Arquitecturas de Hardware Adicionales (RISC-V 64 / RV64GC)

### Contexto Actual:
Beaver OS compila y ejecuta nativamente en modo bi-arquitectura verificada:
- **x86_64:** Modo largo de 64 bits con paginación de 4 niveles, SMP APIC/MADT, syscalls MSR e interrupciones IDT.
- **ARM64 (`aarch64`):** EL1/EL0 con MMU L0->L1 (TTBR0/TTBR1), GICv2, Generic Timer a 100 Hz y VirtIO.

### Objetivos de la Expansión:
1. **Target de Compilación Bare-Metal:**
   - Incorporación del target `riscv64gc-unknown-none` en el compilador de Rust.
   - Extensiones base: `RV64I` (instrucciones base 64-bit), `M` (multiplicación/división en hardware), `A` (operaciones atómicas de memoria), `F` (coma flotante simple precisión), `D` (doble precisión), `C` (instrucciones comprimidas de 16 bits).
2. **Interfaz Binaria de Supervisión (OpenSBI v0.2+):**
   - Integración con SBI para llamadas de servicio del firmware: consola de depuración temprana, temporizador de hardware (`sbi_set_timer`) y apagado/reinicio del sistema.
3. **Controlador de Interrupciones y MMU Sv39 / Sv48:**
   - Configuración del controlador de interrupciones a nivel de plataforma (**PLIC** - *Platform-Level Interrupt Controller*) y temporizador local (**CLINT**).
   - Traducción de direcciones virtuales de 3 niveles (**Sv39**, páginas de 4 KiB, gigapáginas de 1 GiB y megapáginas de 2 MiB) con registros de control `satp` (*Supervisor Address Translation and Protection*).
4. **Validación en Hardware Emulado y Real:**
   - Automatización de arranque en QEMU (`qemu-system-riscv64 -M virt`).
   - Soporte para placas monoplaca reales accesibles en el mercado (e.g. StarFive VisionFive 2, SiFive HiFive Unleashed).

