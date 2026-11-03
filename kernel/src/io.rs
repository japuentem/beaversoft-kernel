// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Low-Level Port I/O Utilities for x86_64 Hardware Control
// ============================================================================

use core::arch::asm;

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") val,
        options(nostack, preserves_flags)
    );
}

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") val,
        options(nostack, preserves_flags)
    );
    val
}

#[inline]
pub unsafe fn outw(port: u16, val: u16) {
    asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") val,
        options(nostack, preserves_flags)
    );
}

#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    asm!(
        "in ax, dx",
        in("dx") port,
        out("ax") val,
        options(nostack, preserves_flags)
    );
    val
}

#[inline]
pub unsafe fn outl(port: u16, val: u32) {
    asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") val,
        options(nostack, preserves_flags)
    );
}

#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    asm!(
        "in eax, dx",
        in("dx") port,
        out("eax") val,
        options(nostack, preserves_flags)
    );
    val
}

/// Pausa breve de I/O escribiendo al puerto 0x80 (convención estándar x86)
#[inline]
pub unsafe fn io_wait() {
    outb(0x80, 0);
}
