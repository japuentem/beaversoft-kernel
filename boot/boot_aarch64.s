/* ============================================================================
 * Project: Beaversoft Meta-Kernel
 * Author: José Alberto Puente Maldonado (japuentem)
 * Brand: Beaversoft
 * Description: AArch64 bare-metal boot stub with standard 64-byte ARM64 Image
 *              Header (Android Fastboot / Samsung Odin / U-Boot / QEMU virt)
 * ============================================================================ */

.section .text.boot
.global _start_aarch64
.type _start_aarch64, @function

_start_aarch64:
    /* ========================================================================
     * Cabecera de Imagen ARM64 Estándar (64 bytes exactos: 0x00 .. 0x3F)
     * Requerida por Android ABL/LK (fastboot boot), Samsung S-Boot, U-Boot booti
     * ======================================================================== */
    b       _actual_start                   // 0x00: code0 (Salto ejecutable al inicio real)
    .long   0                               // 0x04: code1 (Reservado)
    .quad   0x00080000                      // 0x08: text_offset (512 KiB desde base de RAM)
    .quad   _kernel_end - _start_aarch64    // 0x10: image_size (Tamaño efectivo del kernel)
    .quad   0x00000002                      // 0x18: flags (Bit 0=LE, Bits 1-2=4K pages)
    .quad   0                               // 0x20: res2
    .quad   0                               // 0x28: res3
    .quad   0                               // 0x30: res4
    .long   0x644d5241                      // 0x38: magic ("ARM\x64" en Little-Endian)
    .long   0                               // 0x3c: res5 (PE COFF offset = 0)

_actual_start:
    // 0. Preservar x0 (Dirección física del DTB / Flattened Device Tree entregada por el bootloader)
    mov     x19, x0

    // 1. Identificar núcleo de CPU primario (BSP) vs núcleos secundarios (AP)
    // MPIDR_EL1: Affinity Level 0 almacena el ID del núcleo (0, 1, 2, 3...)
    mrs     x1, mpidr_el1
    and     x1, x1, #0xFF
    cbz     x1, 1f

    // Núcleos secundarios esperan en bucle de bajo consumo hasta despertar por PSCI
2:  wfe
    b       2b

1:
    // 2. Si el bootloader del móvil arrancó en EL2 (Hypervisor), descender limpiamente a EL1h
    mrs     x1, CurrentEL
    lsr     x1, x1, #2
    cmp     x1, #2
    b.ne    _el1_entry

    // Configurar EL1 como AArch64 (HCR_EL2.RW = bit 31)
    mov     x1, #(1 << 31)
    msr     hcr_el2, x1

    // Permitir acceso al Generic Timer desde EL1
    mrs     x1, cnthctl_el2
    orr     x1, x1, #3
    msr     cnthctl_el2, x1
    msr     cntvoff_el2, xzr

    // SPSR_EL2 = EL1h (0b0101) con DAIF enmascarado (0x3C5)
    mov     x1, #0x3c5
    msr     spsr_el2, x1
    adr     x1, _el1_entry
    msr     elr_el2, x1
    eret

_el1_entry:
    // 3. Habilitar coprocesadores de punto flotante y SIMD (Neon) en CPACR_EL1
    // Bits [21:20] = 0b11 (FPEN: Non-trapping FP/SIMD access en EL0 y EL1)
    mrs     x1, cpacr_el1
    orr     x1, x1, #(0x3 << 20)
    msr     cpacr_el1, x1
    isb

    // 4. Configurar puntero de pila (Stack Pointer) del kernel en EL1
    ldr     x1, =_stack_top
    mov     sp, x1

    // 5. Limpiar sección .bss con ceros
    ldr     x1, =_kernel_data_end
    ldr     x2, =_kernel_end
3:
    cmp     x1, x2
    b.ge    4f
    str     xzr, [x1], #8
    b       3b

4:
    // 6. Restaurar puntero DTB en x0 y transferir control a Rust
    mov     x0, x19
    bl      kernel_main_aarch64

    // En caso de retorno inesperado, detener CPU en bucle de bajo consumo
5:  wfi
    b       5b

.section .bss
.align 16
.global _stack_bottom
.global _stack_top
_stack_bottom:
    .skip 0x40000 // 256 KiB de pila dedicada para el kernel supervisor
_stack_top:
