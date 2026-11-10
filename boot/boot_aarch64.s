/* ============================================================================
 * Project: Beaversoft Meta-Kernel
 * Author: José Alberto Puente Maldonado (japuentem)
 * Brand: Beaversoft
 * Description: AArch64 bare-metal boot stub for QEMU virt & ARM Cortex-A processors
 * ============================================================================ */

.section .text.boot
.global _start_aarch64
.type _start_aarch64, @function

_start_aarch64:
    // 1. Identificar núcleo de CPU primario (BSP) vs núcleos secundarios (AP)
    // MPIDR_EL1: Affinity Level 0 almacena el ID del núcleo (0, 1, 2, 3...)
    mrs     x1, mpidr_el1
    and     x1, x1, #0xFF
    cbz     x1, 1f

    // Núcleos secundarios esperan en bucle de bajo consumo hasta despertar por PSCI
2:  wfe
    b       2b

1:
    // 2. Habilitar coprocesadores de punto flotante y SIMD (Neon) en CPACR_EL1
    // Bits [21:20] = 0b11 (FPEN: Non-trapping FP/SIMD access en EL0 y EL1)
    mrs     x1, cpacr_el1
    orr     x1, x1, #(0x3 << 20)
    msr     cpacr_el1, x1
    isb

    // 3. Configurar puntero de pila (Stack Pointer) del kernel en EL1
    ldr     x1, =_stack_top
    mov     sp, x1

    // 4. Limpiar sección .bss con ceros
    ldr     x1, =_kernel_data_end
    ldr     x2, =_kernel_end
3:
    cmp     x1, x2
    b.ge    4f
    str     xzr, [x1], #8
    b       3b

4:
    // 5. Transferir control a la función principal de Rust (x0 contiene el puntero al DTB)
    bl      kernel_main_aarch64

    // En caso de retorno inesperado, detener CPU en bucle de bajo consumo
5:  wfi
    b       5b

.section .bss
.align 16
.global _stack_bottom
_stack_bottom:
    .skip 0x40000 // 256 KiB de pila dedicada para el kernel supervisor
.global _stack_top
_stack_top:

