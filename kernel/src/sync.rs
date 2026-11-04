// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Interrupt-Safe Spinlock (IrqSafeSpinLock) for Bare-Metal Concurrency
// ============================================================================

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::panic::Location;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// Identificador del dueño nulo (ningún núcleo posee el candado)
const NO_OWNER_CPU: u32 = u32::MAX;

pub struct SpinMutex<T> {
    lock: AtomicBool,
    /// ID del núcleo (APIC/CPU) que adquirió el candado, o `NO_OWNER_CPU` si está libre.
    owner_cpu: AtomicU32,
    /// Puntero a `&'static Location<'static>` de quien adquirió el candado (0 si no disponible).
    owner_loc: AtomicUsize,
    data: UnsafeCell<T>,
}

pub type SpinLock<T> = SpinMutex<T>;

unsafe impl<T: Send> Sync for SpinMutex<T> {}
unsafe impl<T: Send> Send for SpinMutex<T> {}

/// Obtiene un identificador ligero del núcleo actual (0 en modo uniprocesador BSP).
#[inline(always)]
fn current_cpu_id() -> u32 {
    0
}

impl<T> SpinMutex<T> {
    pub const DEADLOCK_THRESHOLD_SPINS: u64 = 25_000_000;

    pub const fn new(data: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            owner_cpu: AtomicU32::new(NO_OWNER_CPU),
            owner_loc: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Devuelve la ubicación (`file:line`) donde se adquirió actualmente el candado, si está bloqueado.
    pub fn owner_location(&self) -> Option<&'static Location<'static>> {
        let ptr = self.owner_loc.load(Ordering::Relaxed);
        if ptr == 0 {
            None
        } else {
            // SAFETY: `owner_loc` solo almacena punteros provenientes de `Location::caller()`, que son `'static`.
            Some(unsafe { &*(ptr as *const Location<'static>) })
        }
    }

    /// Verifica si el candado ya está adquirido por el mismo núcleo actual.
    #[inline(always)]
    pub fn is_locked_by_current_cpu(&self) -> bool {
        self.lock.load(Ordering::Relaxed) && self.owner_cpu.load(Ordering::Relaxed) == current_cpu_id()
    }

    /// Intenta adquirir el cerrojo sin bloquear. Devuelve None de inmediato si ya está ocupado.
    #[track_caller]
    pub fn try_lock(&self) -> Option<SpinMutexGuard<'_, T>> {
        let caller = Location::caller();
        let saved_if = disable_interrupts_and_save();
        if !self.lock.swap(true, Ordering::Acquire) {
            self.owner_cpu.store(current_cpu_id(), Ordering::Relaxed);
            self.owner_loc.store(caller as *const Location<'static> as usize, Ordering::Relaxed);
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
    #[track_caller]
    pub fn try_lock_timeout(&self, max_spins: u64) -> Result<SpinMutexGuard<'_, T>, &'static str> {
        let caller = Location::caller();
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
        self.owner_cpu.store(current_cpu_id(), Ordering::Relaxed);
        self.owner_loc.store(caller as *const Location<'static> as usize, Ordering::Relaxed);
        Ok(SpinMutexGuard {
            mutex: self,
            saved_interrupt_flag: saved_if,
        })
    }

    /// Adquiere el cerrojo con detección inmediata de reentrada en el mismo núcleo.
    /// Si el mismo CPU intenta volver a adquirir un candado que ya posee (con IF=0),
    /// dispara `panic!` con diagnóstico exacto (ubicación del dueño original y de la reentrada)
    /// en lugar de girar para siempre.
    #[track_caller]
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        let caller = Location::caller();
        let saved_if = disable_interrupts_and_save();
        let cpu = current_cpu_id();
        let mut spins: u64 = 0;

        while self.lock.swap(true, Ordering::Acquire) {
            // Detección determinista de reentrada: si este mismo núcleo ya es dueño del candado,
            // jamás podrá liberarse porque las interrupciones están deshabilitadas en este núcleo.
            if self.owner_cpu.load(Ordering::Relaxed) == cpu {
                let prev_loc = self.owner_location();
                // Liberar el candado antes de invocar panic! para que el manejador de pánico pueda imprimir
                self.owner_cpu.store(NO_OWNER_CPU, Ordering::Relaxed);
                self.owner_loc.store(0, Ordering::Relaxed);
                self.lock.store(false, Ordering::Release);
                unsafe {
                    emergency_serial_log("\r\n[SPINLOCK REENTRANCY DETECTED] Reentrada ilegal en SpinMutex!\r\n");
                }
                if let Some(prev) = prev_loc {
                    panic!(
                        "Reentrada detectada en SpinMutex<{}> en {}:{}:{} (adquirido previamente en {}:{}:{})",
                        core::any::type_name::<T>(),
                        caller.file(), caller.line(), caller.column(),
                        prev.file(), prev.line(), prev.column()
                    );
                } else {
                    panic!(
                        "Reentrada detectada en SpinMutex<{}> en {}:{}:{}",
                        core::any::type_name::<T>(),
                        caller.file(), caller.line(), caller.column()
                    );
                }
            }

            core::hint::spin_loop();
            spins += 1;

            if spins >= Self::DEADLOCK_THRESHOLD_SPINS {
                let prev_loc = self.owner_location();
                unsafe {
                    emergency_serial_log("\r\n[ANTI-DEADLOCK WATCHDOG] Contencion prolongada en SpinMutex!\r\n");
                }
                if let Some(prev) = prev_loc {
                    panic!(
                        "Deadlock por timeout en SpinMutex<{}> solicitado en {}:{} (retenido por {}:{})",
                        core::any::type_name::<T>(),
                        caller.file(), caller.line(),
                        prev.file(), prev.line()
                    );
                } else {
                    panic!(
                        "Deadlock por timeout en SpinMutex<{}> solicitado en {}:{}",
                        core::any::type_name::<T>(),
                        caller.file(), caller.line()
                    );
                }
            }
        }

        self.owner_cpu.store(cpu, Ordering::Relaxed);
        self.owner_loc.store(caller as *const Location<'static> as usize, Ordering::Relaxed);

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
        self.mutex.owner_cpu.store(NO_OWNER_CPU, Ordering::Relaxed);
        self.mutex.owner_loc.store(0, Ordering::Relaxed);
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
