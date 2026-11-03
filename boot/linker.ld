/* ============================================================================
 * Project: Beaversoft Meta-Kernel
 * Author: José Alberto Puente Maldonado (japuentem)
 * Brand: Beaversoft
 * Description: Linker script for x86_64 bare-metal kernel
 * ============================================================================ */

ENTRY(_start)

SECTIONS
{
    . = 1M;

    _kernel_start = .;

    .multiboot : ALIGN(4K) {
        KEEP(*(.multiboot_header))
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
    }

    _kernel_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}