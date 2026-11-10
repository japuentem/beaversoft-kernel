// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Low-Level Port I/O Utilities for x86_64 Hardware Control & ARM64 HAL
// ============================================================================

#[cfg(target_arch = "x86_64")]
use core::arch::asm;

#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nostack, preserves_flags));
}
#[inline]
#[cfg(target_arch = "aarch64")]
pub unsafe fn outb(_port: u16, _val: u8) {}

#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("in al, dx", in("dx") port, out("al") val, options(nostack, preserves_flags));
    val
}
#[inline]
#[cfg(target_arch = "aarch64")]
pub unsafe fn inb(_port: u16) -> u8 {
    0
}

#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn outw(port: u16, val: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") val, options(nostack, preserves_flags));
}
#[inline]
#[cfg(target_arch = "aarch64")]
pub unsafe fn outw(_port: u16, _val: u16) {}

#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    asm!("in ax, dx", in("dx") port, out("ax") val, options(nostack, preserves_flags));
    val
}
#[inline]
#[cfg(target_arch = "aarch64")]
pub unsafe fn inw(_port: u16) -> u16 {
    0
}

#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn outl(port: u16, val: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") val, options(nostack, preserves_flags));
}
#[inline]
#[cfg(target_arch = "aarch64")]
pub unsafe fn outl(_port: u16, _val: u32) {}

#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    asm!("in eax, dx", in("dx") port, out("eax") val, options(nostack, preserves_flags));
    val
}
#[inline]
#[cfg(target_arch = "aarch64")]
pub unsafe fn inl(_port: u16) -> u32 {
    0
}

#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn io_wait() {
    outb(0x80, 0);
}
#[inline]
#[cfg(target_arch = "aarch64")]
pub unsafe fn io_wait() {}
