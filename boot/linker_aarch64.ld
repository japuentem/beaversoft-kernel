/* ============================================================================
 * Project: Beaversoft Meta-Kernel
 * Author: José Alberto Puente Maldonado (japuentem)
 * Brand: Beaversoft
 * Description: Linker script for ARM64 (AArch64) bare-metal kernel on QEMU virt
 * ============================================================================ */

ENTRY(_start_aarch64)

SECTIONS
{
    . = 0x40080000;

    _kernel_start = .;

    .text.boot : ALIGN(4K) {
        KEEP(*(.text.boot))
    }

    .text : ALIGN(4K) {
        *(.text .text.*)
    }

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4K) {
        *(.data .data.*)
    }

    _kernel_data_end = .;

    .bss : ALIGN(4K) {
        *(.bss .bss.*)
        *(COMMON)
    }

    _kernel_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}

