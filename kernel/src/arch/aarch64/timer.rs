// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: ARM Generic Timer Driver for AArch64 (Deterministic PIT equivalent)
// ============================================================================

use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

pub static TICKS: AtomicU64 = AtomicU64::new(0);

pub struct ArmGenericTimer;

impl ArmGenericTimer {
    /// Inicializa el temporizador genérico de ARM para disparar a 100 Hz (cada 10 ms)
    pub fn init() {
        let freq = Self::frequency();
        let interval = freq / 100; // 100 Hz

        unsafe {
            // Cargar valor de cuenta regresiva en CNTP_TVAL_EL0
            asm!("msr cntp_tval_el0, {}", in(reg) interval, options(nomem, nostack));
            // Habilitar temporizador sin enmascarar interrupción: ENABLE=1, IMASK=0
            asm!("msr cntp_ctl_el0, {}", in(reg) 1u64, options(nomem, nostack));
        }

        // En QEMU virt, el temporizador físico CNTP usa la IRQ 30 (PPI 14)
        super::gic::GicV2::enable_irq(30);
    }

    /// Lee la frecuencia del contador del sistema en Hz (CNTFRQ_EL0)
    #[inline(always)]
    pub fn frequency() -> u64 {
        let freq: u64;
        unsafe {
            asm!("mrs {}, cntfrq_el0", out(reg) freq, options(nomem, nostack));
        }
        freq
    }

    /// Lee el valor actual del contador físico (CNTPCT_EL0)
    #[inline(always)]
    pub fn current_ticks() -> u64 {
        let ticks: u64;
        unsafe {
            asm!("mrs {}, cntpct_el0", out(reg) ticks, options(nomem, nostack));
        }
        ticks
    }

    /// Manejador de interrupción del temporizador: recarga el intervalo y actualiza ticks
    pub fn on_irq() {
        let freq = Self::frequency();
        let interval = freq / 100;
        unsafe {
            asm!("msr cntp_tval_el0, {}", in(reg) interval, options(nomem, nostack));
        }
        TICKS.fetch_add(1, Ordering::Relaxed);
    }

    /// Retorna los segundos transcurridos desde el arranque
    pub fn uptime_seconds() -> u64 {
        TICKS.load(Ordering::Relaxed) / 100
    }
}

pub fn timer_frequency() -> u64 {
    ArmGenericTimer::frequency()
}
