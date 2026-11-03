// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Preemptive Round-Robin Task Scheduler for Micro-Kernel Threads
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use crate::drivers::timer::get_ticks;
use crate::sync::SpinMutex;
use crate::task::context::switch_context;
use crate::task::{Task, TaskId, TaskState};
use crate::vga::VgaColor;
use crate::{kprintln, kprint_color};

pub struct Scheduler {
    tasks: Vec<Task>,
    current_idx: usize,
    active: bool,
    next_id: TaskId,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            current_idx: 0,
            active: false,
            next_id: 1,
        }
    }

    pub fn init(&mut self) {
        // Registrar la tarea principal (Task 0: main_shell)
        self.tasks.push(Task::new_main());
        self.current_idx = 0;
        self.active = true;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn spawn(&mut self, name: &'static str, entry: fn()) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        let task = Task::new(id, name, entry);
        self.tasks.push(task);
        id
    }

    pub fn print_tasks(&self) {
        kprintln!("=========================== HILOS ACTIVOS EN BEAVER ===========================");
        kprintln!(" ID | Nombre de Tarea       | Usuario   | UID | Estado       | CPU Ticks (@100Hz)");
        kprintln!("----+-----------------------+-----------+-----+--------------+-------------------");

        for (idx, t) in self.tasks.iter().enumerate() {
            let state_str = match t.state {
                TaskState::Running => "Ejecutando [RUN]",
                TaskState::Ready => "Listo     [RDY]",
                TaskState::Sleeping(_) => "Durmiendo [SLP]",
                TaskState::Finished => "Finalizado[END]",
            };

            let is_current = idx == self.current_idx;
            let marker = if is_current { " *" } else { "  " };

            kprint_color!(
                if is_current { VgaColor::LightGreen } else { VgaColor::White },
                VgaColor::Black,
                "{}{:02} | {:<21} | {:<9} | {:>3} | {:<12} | {} ticks\n",
                marker, t.id, t.name, t.username, t.uid, state_str, t.ticks_executed
            );
        }
        kprintln!("===================================================================================");
    }

    /// Devuelve las credenciales (UID, GID, username) de la tarea activa
    pub fn current_credentials(&self) -> (crate::task::Uid, crate::task::Gid, &'static str) {
        if let Some(t) = self.tasks.get(self.current_idx) {
            (t.uid, t.gid, t.username)
        } else {
            (crate::task::ROOT_UID, crate::task::ROOT_GID, "root")
        }
    }

    /// Asigna las credenciales (UID, GID, username) a la tarea activa
    pub fn set_current_credentials(&mut self, uid: crate::task::Uid, gid: crate::task::Gid, username: &'static str) {
        if let Some(t) = self.tasks.get_mut(self.current_idx) {
            t.uid = uid;
            t.gid = gid;
            t.username = username;
        }
    }

    /// Devuelve el TID de la tarea actualmente en ejecución
    pub fn current_task_id(&self) -> u64 {
        if let Some(t) = self.tasks.get(self.current_idx) {
            t.id
        } else {
            0
        }
    }

    /// Termina una tarea por su identificador de tarea (TID)
    pub fn kill_task(&mut self, id: TaskId) -> Result<(), &'static str> {
        if id == 0 {
            return Err("No se puede terminar la tarea 0 (main_shell)");
        }
        for t in &mut self.tasks {
            if t.id == id {
                if t.state == TaskState::Finished {
                    return Err("La tarea ya se encuentra finalizada");
                }
                t.state = TaskState::Finished;
                return Ok(());
            }
        }
        Err("No se encontró ninguna tarea con el ID especificado")
    }
}

pub static SCHEDULER: SpinMutex<Scheduler> = SpinMutex::new(Scheduler::new());

/// Conmuta hacia la siguiente tarea lista liberando el cerrojo del scheduler
/// ANTES de cambiar el puntero de pila para evitar deadlocks
pub fn schedule() {
    let context_switch_args = {
        let mut sched = SCHEDULER.lock();
        if !sched.active || sched.tasks.len() <= 1 {
            return;
        }

        let now = get_ticks();

        // 1. Despertar tareas durmientes
        for t in &mut sched.tasks {
            if let TaskState::Sleeping(wake_tick) = t.state {
                if now >= wake_tick {
                    t.state = TaskState::Ready;
                }
            }
        }

        let curr = sched.current_idx;
        sched.tasks[curr].ticks_executed += 1;

        // 2. Buscar la siguiente tarea lista para Round-Robin
        let total = sched.tasks.len();
        let mut next_idx = (curr + 1) % total;
        let mut found = false;

        for _ in 0..total {
            if sched.tasks[next_idx].state == TaskState::Ready || sched.tasks[next_idx].state == TaskState::Running {
                found = true;
                break;
            }
            next_idx = (next_idx + 1) % total;
        }

        if !found || next_idx == curr {
            return; // No hay otra tarea esperando
        }

        // 3. Conmutar estados lógicos
        if sched.tasks[curr].state == TaskState::Running {
            sched.tasks[curr].state = TaskState::Ready;
        }
        sched.tasks[next_idx].state = TaskState::Running;

        let prev_idx = curr;
        sched.current_idx = next_idx;

        let prev_sp = &mut sched.tasks[prev_idx].stack_ptr as *mut *mut u8;
        let next_sp = sched.tasks[next_idx].stack_ptr;
        Some((prev_sp, next_sp))
    }; // <-- ¡EL CERROJO DEL SCHEDULER SE LIBERA AQUÍ MISMO!

    if let Some((prev_sp, next_sp)) = context_switch_args {
        unsafe {
            switch_context(prev_sp, next_sp);
        }
    }
}

pub fn sleep(ticks: u64) {
    let now = get_ticks();
    {
        let mut sched = SCHEDULER.lock();
        let curr = sched.current_idx;
        if curr < sched.tasks.len() {
            sched.tasks[curr].state = TaskState::Sleeping(now + ticks);
        }
    }
    schedule();
}

pub fn yield_now() {
    schedule();
}

pub fn mark_finished_and_yield() {
    {
        let mut sched = SCHEDULER.lock();
        let curr = sched.current_idx;
        if curr < sched.tasks.len() {
            sched.tasks[curr].state = TaskState::Finished;
        }
    }
    schedule();
}

pub fn schedule_tick() {
    schedule();
}
