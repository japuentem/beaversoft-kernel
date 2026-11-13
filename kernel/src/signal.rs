// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: POSIX Asynchronous Signal Management (kill, sigaction, sigreturn)
// ============================================================================

extern crate alloc;

use alloc::collections::BTreeMap;
use crate::sync::SpinLock;

// Constantes estándar de señales POSIX
pub const SIGHUP:  i32 = 1;
pub const SIGINT:  i32 = 2;
pub const SIGQUIT: i32 = 3;
pub const SIGILL:  i32 = 4;
pub const SIGTRAP: i32 = 5;
pub const SIGABRT: i32 = 6;
pub const SIGFPE:  i32 = 8;
pub const SIGKILL: i32 = 9;   // No interceptable ni ignorable
pub const SIGUSR1: i32 = 10;
pub const SIGSEGV: i32 = 11;
pub const SIGUSR2: i32 = 12;
pub const SIGPIPE: i32 = 13;
pub const SIGALRM: i32 = 14;
pub const SIGTERM: i32 = 15;
pub const SIGCHLD: i32 = 17;
pub const SIGCONT: i32 = 18;
pub const SIGSTOP: i32 = 19;  // No interceptable ni ignorable

pub const SIG_DFL: u64 = 0;
pub const SIG_IGN: u64 = 1;

// Modos para sigprocmask
pub const SIG_BLOCK:   i32 = 0;
pub const SIG_UNBLOCK: i32 = 1;
pub const SIG_SETMASK: i32 = 2;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SigAction {
    pub sa_handler: u64,
    pub sa_mask: u64,
    pub sa_flags: u64,
    pub sa_restorer: u64,
}

#[derive(Clone, Debug)]
pub struct ProcessSignalState {
    pub handlers: [SigAction; 32],
    pub pending: u32,
    pub blocked: u32,
    pub is_alive: bool,
}

impl ProcessSignalState {
    pub fn new() -> Self {
        Self {
            handlers: [SigAction::default(); 32],
            pending: 0,
            blocked: 0,
            is_alive: true,
        }
    }
}

pub struct SignalManager {
    processes: BTreeMap<usize, ProcessSignalState>,
}

impl SignalManager {
    pub const fn new() -> Self {
        Self {
            processes: BTreeMap::new(),
        }
    }

    pub fn ensure_process(&mut self, pid: usize) -> &mut ProcessSignalState {
        self.processes.entry(pid).or_insert_with(ProcessSignalState::new)
    }

    pub fn sigaction(&mut self, pid: usize, sig: i32, new_act: Option<SigAction>) -> Result<SigAction, i64> {
        if sig <= 0 || sig >= 32 {
            return Err(-22); // -EINVAL
        }
        // POSIX prohíbe explícitamente capturar o ignorar SIGKILL y SIGSTOP
        if sig == SIGKILL || sig == SIGSTOP {
            return Err(-22); // -EINVAL
        }

        let state = self.ensure_process(pid);
        let old = state.handlers[sig as usize];

        if let Some(act) = new_act {
            state.handlers[sig as usize] = act;
        }

        Ok(old)
    }

    pub fn kill(&mut self, target_pid: usize, sig: i32) -> Result<(), i64> {
        if sig < 0 || sig >= 32 {
            return Err(-22); // -EINVAL
        }

        // Si el proceso no existe en la tabla y es mayor que 0, verificar existencia en el planificador
        if !self.processes.contains_key(&target_pid) && target_pid > 0 {
            let task_exists = crate::task::scheduler::SCHEDULER.lock().task_state(target_pid as u64).is_some();
            if !task_exists {
                return Err(-3); // -ESRCH (No such process)
            }
        }

        let state = self.ensure_process(target_pid);
        if !state.is_alive {
            return Err(-3); // -ESRCH
        }

        // sig == 0 es verificación de existencia POSIX sin enviar señal
        if sig == 0 {
            return Ok(());
        }

        // SIGKILL es definitivo e ineludible
        if sig == SIGKILL {
            state.is_alive = false;
            state.pending |= 1 << SIGKILL;
            let _ = crate::task::scheduler::SCHEDULER.lock().kill_task(target_pid as u64);
            return Ok(());
        }

        let handler = state.handlers[sig as usize].sa_handler;
        if handler == SIG_IGN {
            // Señal ignorada
            return Ok(());
        }

        if handler == SIG_DFL {
            match sig {
                SIGTERM | SIGINT | SIGSEGV | SIGQUIT | SIGILL | SIGABRT | SIGFPE => {
                    // Acción por defecto: terminar el proceso
                    state.is_alive = false;
                }
                _ => {}
            }
        }

        // Encolar señal pendiente si no está bloqueada
        state.pending |= 1 << sig;
        Ok(())
    }

    pub fn sigprocmask(&mut self, pid: usize, how: i32, new_mask: Option<u64>) -> Result<u64, i64> {
        let state = self.ensure_process(pid);
        let old_mask = state.blocked as u64;

        if let Some(mask) = new_mask {
            let mask32 = (mask & 0xFFFF_FFFF) as u32;
            // POSIX: SIGKILL y SIGSTOP nunca pueden ser bloqueados
            let sanitized_mask = mask32 & !((1 << SIGKILL) | (1 << SIGSTOP));

            match how {
                SIG_BLOCK => state.blocked |= sanitized_mask,
                SIG_UNBLOCK => state.blocked &= !sanitized_mask,
                SIG_SETMASK => state.blocked = sanitized_mask,
                _ => return Err(-22), // -EINVAL
            }
        }

        Ok(old_mask)
    }

    pub fn pop_pending(&mut self, pid: usize) -> Option<i32> {
        if let Some(state) = self.processes.get_mut(&pid) {
            let deliverable = state.pending & !state.blocked;
            if deliverable == 0 {
                return None;
            }
            // Encontrar la señal con el bit más bajo
            let sig = deliverable.trailing_zeros() as i32;
            state.pending &= !(1 << sig);
            Some(sig)
        } else {
            None
        }
    }

    pub fn is_alive(&self, pid: usize) -> bool {
        self.processes.get(&pid).map_or(false, |s| s.is_alive)
    }
}

pub static SIGNAL_MANAGER: SpinLock<SignalManager> = SpinLock::new(SignalManager::new());
