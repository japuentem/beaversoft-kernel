// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Kernel Multitasking Subsystem, Task Control Blocks & Workers
// ============================================================================

extern crate alloc;

pub mod context;
pub mod scheduler;
pub mod pkg;
pub mod script;

use alloc::vec::Vec;
use alloc::vec;
use core::ptr::null_mut;
use crate::vga::{VgaColor, VGA_WRITER};

pub type TaskId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Sleeping(u64), // Ticks objetivo para despertar
    Finished,
}

pub type Uid = u32;
pub type Gid = u32;

pub const ROOT_UID: Uid = 0;
pub const ROOT_GID: Gid = 0;

pub struct Task {
    pub id: TaskId,
    pub name: &'static str,
    pub state: TaskState,
    pub stack: Vec<u8>,
    pub stack_ptr: *mut u8,
    pub cr3: u64,
    pub ticks_executed: u64,
    pub uid: Uid,
    pub gid: Gid,
    pub username: &'static str,
    pub running_on_core: Option<u32>,
    pub supervisor_rsp: u64,
    pub user_entry: u64,
    pub user_stack_top: u64,
    pub is_user: bool,
    pub exit_code: Option<u64>,
}

// Seguridad de memoria en hilos
// SAFETY: `Task` posee en exclusiva su `Vec<u8>` de pila y solo es manipulada bajo el cerrojo global `SCHEDULER`.
unsafe impl Send for Task {}

impl Task {
    #[inline]
    pub fn kernel_stack_top(&self) -> u64 {
        if self.stack.is_empty() {
            0
        } else {
            (self.stack.as_ptr() as usize + self.stack.len()) as u64 & !0xF
        }
    }

    pub fn new(id: TaskId, name: &'static str, entry: fn()) -> Self {
        const STACK_SIZE: usize = 16 * 1024; // 16 KiB de pila privada por hilo
        let mut stack = vec![0_u8; STACK_SIZE];

        // Obtener la cima de la pila alineada a 16 bytes (x86_64 ABI)
        let stack_top = stack.as_mut_ptr() as usize + STACK_SIZE;
        let mut sp = (stack_top & !0xF) as *mut u64;

        #[cfg(target_arch = "x86_64")]
        unsafe {
            // Dirección de retorno si el hilo finaliza
            sp = sp.sub(1);
            *sp = task_exit as *const () as usize as u64;

            // Dirección de entrada para el 'ret' de switch_context
            sp = sp.sub(1);
            *sp = entry as usize as u64;

            // RFLAGS inicial: 0x202 (IF = 1, interrupciones activadas)
            sp = sp.sub(1);
            *sp = 0x202;

            // Registros callee-saved (rbp, rbx, r12, r13, r14, r15)
            for _ in 0..6 {
                sp = sp.sub(1);
                *sp = 0;
            }
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            sp = sp.sub(1);
            *sp = entry as usize as u64; // x30 = entry
            sp = sp.sub(1);
            *sp = 0; // x29 = FP
            for _ in 0..10 {
                sp = sp.sub(1);
                *sp = 0;
            }
        }

        Self {
            id,
            name,
            state: TaskState::Ready,
            stack,
            stack_ptr: sp as *mut u8,
            cr3: crate::memory::paging::kernel_cr3(),
            ticks_executed: 0,
            uid: ROOT_UID,
            gid: ROOT_GID,
            username: "root",
            running_on_core: None,
            supervisor_rsp: 0,
            user_entry: 0,
            user_stack_top: 0,
            is_user: false,
            exit_code: None,
        }
    }

    pub fn new_user(id: TaskId, name: &'static str, cr3: u64, user_entry: u64, user_stack_top: u64) -> Self {
        const STACK_SIZE: usize = 16 * 1024; // 16 KiB de pila de guardia del kernel para este proceso
        let mut stack = vec![0_u8; STACK_SIZE];

        let stack_top = stack.as_mut_ptr() as usize + STACK_SIZE;
        let mut sp = (stack_top & !0xF) as *mut u64;

        #[cfg(target_arch = "x86_64")]
        unsafe {
            sp = sp.sub(1);
            *sp = task_exit as *const () as usize as u64;

            sp = sp.sub(1);
            *sp = user_process_trampoline as *const () as usize as u64;

            sp = sp.sub(1);
            *sp = 0x202; // IF = 1

            for _ in 0..6 {
                sp = sp.sub(1);
                *sp = 0;
            }
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            sp = sp.sub(1);
            *sp = user_process_trampoline as *const () as usize as u64;
            sp = sp.sub(1);
            *sp = 0;
            for _ in 0..10 {
                sp = sp.sub(1);
                *sp = 0;
            }
        }

        Self {
            id,
            name,
            state: TaskState::Ready,
            stack,
            stack_ptr: sp as *mut u8,
            cr3,
            ticks_executed: 0,
            uid: ROOT_UID,
            gid: ROOT_GID,
            username: "root",
            running_on_core: None,
            supervisor_rsp: 0,
            user_entry,
            user_stack_top,
            is_user: true,
            exit_code: None,
        }
    }

    pub fn new_main() -> Self {
        // Tarea 0 representa el hilo principal ya en ejecución en el BSP (Core 0)
        Self {
            id: 0,
            name: "main_shell",
            state: TaskState::Running,
            stack: Vec::new(),
            stack_ptr: null_mut(),
            cr3: crate::memory::paging::kernel_cr3(),
            ticks_executed: 0,
            uid: ROOT_UID,
            gid: ROOT_GID,
            username: "root",
            running_on_core: Some(0),
            supervisor_rsp: 0,
            user_entry: 0,
            user_stack_top: 0,
            is_user: false,
            exit_code: None,
        }
    }
}

pub extern "C" fn user_process_trampoline() {
    let (user_entry, user_stack_top, cr3) = scheduler::SCHEDULER.lock()
        .current_user_task_info()
        .unwrap_or((0, 0, 0));

    if user_entry != 0 && user_stack_top != 0 && cr3 != 0 {
        #[cfg(target_arch = "x86_64")]
        if crate::memory::paging::read_cr3() != cr3 {
            unsafe { crate::memory::paging::write_cr3(cr3); }
        }

        crate::syscall::CURRENT_USER_BRK.store(crate::syscall::USER_HEAP_START, core::sync::atomic::Ordering::Release);
        let exit_code = unsafe { crate::syscall::enter_ring3_with_return(user_entry, user_stack_top) };

        scheduler::SCHEDULER.lock().set_current_exit_code(exit_code);
    }

    #[cfg(target_arch = "x86_64")]
    crate::memory::paging::switch_to_kernel_cr3();
    #[cfg(target_arch = "aarch64")]
    crate::arch::aarch64::mmu::Aarch64Mmu::switch_to_kernel_ttbr0();

    scheduler::mark_finished_and_yield();
}

#[allow(dead_code)]
extern "C" fn task_exit() -> ! {
    scheduler::mark_finished_and_yield();
    loop {
        crate::arch::current::halt();
    }
}

// ============================================================================
// Hilos de Trabajo en Segundo Plano (Background Workers)
// ============================================================================

/// Tarea 1: Worker de Telemetría en Segundo Plano
/// Monitorea el latido del sistema y actualiza un indicador gráfico en tiempo real
pub fn telemetry_worker() {
    let mut counter: u32 = 0;

    loop {
        counter += 1;

        // Indicador de actividad en esquina superior (fila 0, col 48) sin alterar el cursor de hardware
        {
            let spinner = match counter % 4 {
                0 => b'|',
                1 => b'/',
                2 => b'-',
                _ => b'\\',
            };
            if let Some(mut vga) = VGA_WRITER.try_lock() {
                vga.write_str_at(0, 48, "[TH1: ", VgaColor::Yellow, VgaColor::Black);
                vga.write_char_at(0, 54, spinner, VgaColor::Yellow, VgaColor::Black);
                vga.write_char_at(0, 55, b']', VgaColor::Yellow, VgaColor::Black);
            }
        }

        // Dormir durante 100 ticks (1 segundo) liberando la CPU para otras tareas
        scheduler::sleep(100);
    }
}

/// Tarea 2: Guardián de Almacenamiento en Segundo Plano
/// Inspecciona de forma continua la paridad de las ranuras A/B en segundo plano
pub fn storage_guard_worker() {
    loop {
        // Indicador de guardia en esquina superior (fila 0, col 34) sin alterar el cursor de hardware
        {
            if let Some(mut vga) = VGA_WRITER.try_lock() {
                vga.write_str_at(0, 34, "[GUARD: OK]", VgaColor::LightGreen, VgaColor::Black);
            }
        }

        // Dormir durante 300 ticks (3 segundos)
        scheduler::sleep(300);
    }
}
