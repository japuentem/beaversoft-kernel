// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Interrupt-Safe Spinlock (IrqSafeSpinLock) for Bare-Metal Concurrency
// ============================================================================

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinMutex<T> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

pub type SpinLock<T> = SpinMutex<T>;

unsafe impl<T: Send> Sync for SpinMutex<T> {}
unsafe impl<T: Send> Send for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    pub const DEADLOCK_THRESHOLD_SPINS: u64 = 25_000_000;

    pub const fn new(data: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Intenta adquirir el cerrojo sin bloquear. Devuelve None de inmediato si ya está ocupado.
    pub fn try_lock(&self) -> Option<SpinMutexGuard<'_, T>> {
        let saved_if = disable_interrupts_and_save();
        if !self.lock.swap(true, Ordering::Acquire) {
            Some(SpinMutexGuard {
                mutex: self,
                saved_interrupt_flag: saved_if,
            })
        } else {
            restore_interrupts(saved_if);
            None
        }
    }

    /// Intenta adquirir el cerrojo con límite de ciclos (Watchdog Anti-Deadlock).
    /// Si supera `max_spins`, aborta la espera y devuelve un error evitando el congelamiento de CPU.
    pub fn try_lock_timeout(&self, max_spins: u64) -> Result<SpinMutexGuard<'_, T>, &'static str> {
        let saved_if = disable_interrupts_and_save();
        let mut spins: u64 = 0;
        while self.lock.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
            spins += 1;
            if spins >= max_spins {
                restore_interrupts(saved_if);
                return Err("DEADLOCK_TIMEOUT: Límite de ciclos superado; se abortó espera para salvar CPU.");
            }
        }
        Ok(SpinMutexGuard {
            mutex: self,
            saved_interrupt_flag: saved_if,
        })
    }

    /// Adquiere el cerrojo monitoreando posibles bloqueos cíclicos con Watchdog de hardware.
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        let saved_if = disable_interrupts_and_save();
        let mut spins: u64 = 0;
        let mut warned = false;

        while self.lock.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
            spins += 1;

            if spins >= Self::DEADLOCK_THRESHOLD_SPINS && !warned {
                warned = true;
                // Emitir alerta de emergencia directa a UART COM1 (sin adquirir cerrojos para evitar deadlock recursivo)
                unsafe {
                    emergency_serial_log("\r\n[ANTI-DEADLOCK WATCHDOG] ADVERTENCIA: Posible Deadlock detectado en SpinMutex!\r\n");
                }
            }
        }

        SpinMutexGuard {
            mutex: self,
            saved_interrupt_flag: saved_if,
        }
    }
}

pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
    saved_interrupt_flag: bool,
}

impl<'a, T> Deref for SpinMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for SpinMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.lock.store(false, Ordering::Release);
        restore_interrupts(self.saved_interrupt_flag);
    }
}

#[inline]
fn disable_interrupts_and_save() -> bool {
    let rflags: u64;
    unsafe {
        core::arch::asm!(
            "pushfq",
            "pop {}",
            "cli",
            out(reg) rflags,
            options(nomem, preserves_flags)
        );
    }
    // El bit 9 en RFLAGS es el IF (Interrupt Flag)
    (rflags & (1 << 9)) != 0
}

#[inline]
fn restore_interrupts(was_enabled: bool) {
    if was_enabled {
        unsafe {
            core::arch::asm!("sti", options(nomem, nostack, preserves_flags));
        }
    }
}

/// Emite bytes directamente al puerto serie COM1 sin adquirir ningún cerrojo,
/// garantizando que el reporte de Deadlock sea 100% inmune a bloqueos recursivos.
unsafe fn emergency_serial_log(msg: &str) {
    for b in msg.bytes() {
        while (crate::io::inb(0x3FD) & 0x20) == 0 {
            core::hint::spin_loop();
        }
        crate::io::outb(0x3F8, b);
    }
}
