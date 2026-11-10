// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: AArch64 CPU Registers, Execution Levels & Control Primitives
// ============================================================================

use core::arch::asm;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExceptionLevel {
    EL0, // Ring 3 User Mode
    EL1, // Ring 0 Kernel Mode
    EL2, // Hypervisor Mode
    EL3, // Secure Monitor Mode
}

impl core::fmt::Display for ExceptionLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ExceptionLevel::EL0 => write!(f, "0"),
            ExceptionLevel::EL1 => write!(f, "1"),
            ExceptionLevel::EL2 => write!(f, "2"),
            ExceptionLevel::EL3 => write!(f, "3"),
        }
    }
}

/// Obtiene el nivel de excepción actual (CurrentEL)
#[inline(always)]
pub fn current_el() -> ExceptionLevel {
    let el: u64;
    unsafe {
        asm!("mrs {}, CurrentEL", out(reg) el, options(nomem, nostack));
    }
    match (el >> 2) & 0x3 {
        0 => ExceptionLevel::EL0,
        1 => ExceptionLevel::EL1,
        2 => ExceptionLevel::EL2,
        _ => ExceptionLevel::EL3,
    }
}

/// Habilita interrupciones en AArch64 (limpia bit IRQ en DAIF)
#[inline(always)]
pub fn enable_interrupts() {
    unsafe {
        asm!("msr daifclr, #2", options(nomem, nostack));
    }
}

/// Deshabilita interrupciones en AArch64 (fija bit IRQ en DAIF)
#[inline(always)]
pub fn disable_interrupts() {
    unsafe {
        asm!("msr daifset, #2", options(nomem, nostack));
    }
}

/// Instrucción de bajo consumo Wait For Interrupt (wfi) equivalente a hlt
#[inline(always)]
pub fn halt() {
    unsafe {
        asm!("wfi", options(nomem, nostack));
    }
}

/// Alias para `halt()` en arquitectura ARM
#[inline(always)]
pub fn wfi() {
    halt();
}

/// Instrucción Wait For Event (wfe) para esperas activas de bajo consumo
#[inline(always)]
pub fn cpu_relax() {
    unsafe {
        asm!("wfe", options(nomem, nostack));
    }
}

/// Barrera de sincronización de instrucciones (ISB)
#[inline(always)]
pub fn isb() {
    unsafe {
        asm!("isb", options(nomem, nostack));
    }
}

/// Barrera de sincronización de datos completa (DSB SY)
#[inline(always)]
pub fn dsb() {
    unsafe {
        asm!("dsb sy", options(nomem, nostack));
    }
}
