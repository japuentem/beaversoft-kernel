/* ============================================================================
 * Project: Beaversoft Meta-Kernel
 * Author: José Alberto Puente Maldonado (japuentem)
 * Brand: Beaversoft
 * Description: Userland ELF64 Linker Script for AArch64 (EL0 TTBR0_EL1 @ 0x60000000)
 * ============================================================================ */

ENTRY(_start)

SECTIONS
{
    . = 0x60000000;

    .text : ALIGN(4K) {
        *(.text .text.*)
    }

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4K) {
        *(.data .data.*)
    }

    .bss : ALIGN(4K) {
        *(.bss .bss.*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}

