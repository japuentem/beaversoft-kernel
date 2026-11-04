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
    core_current_task: [Option<TaskId>; 16],
}

// Pila de reposo guardada para cada núcleo secundario (AP) cuando no tiene tareas listas asignadas
static mut AP_IDLE_STACKS: [*mut u8; 16] = [core::ptr::null_mut(); 16];

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            current_idx: 0,
            active: false,
            next_id: 1,
            core_current_task: [None; 16],
        }
    }

    pub fn init(&mut self) {
        // Registrar la tarea principal (Task 0: main_shell) asignada al BSP (Core 0)
        self.tasks.push(Task::new_main());
        self.current_idx = 0;
        self.active = true;
        self.core_current_task[0] = Some(0);
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
        kprintln!(" ID | Nombre de Tarea       | Núcleo | UID | Estado       | CPU Ticks (@100Hz)");
        kprintln!("----+-----------------------+--------+-----+--------------+-------------------");

        for (idx, t) in self.tasks.iter().enumerate() {
            let state_str = match t.state {
                TaskState::Running => "Ejecutando [RUN]",
                TaskState::Ready => "Listo     [RDY]",
                TaskState::Sleeping(_) => "Durmiendo [SLP]",
                TaskState::Finished => "Finalizado[END]",
            };

            let core_str = match t.running_on_core {
                Some(c) => alloc::format!("Core {}", c),
                None => alloc::format!("Any"),
            };

            let is_current = (crate::apic::current_apic_id() == 0 && idx == self.current_idx)
                || t.running_on_core == Some(crate::apic::current_apic_id());
            let marker = if is_current { " *" } else { "  " };

            kprint_color!(
                if is_current { VgaColor::LightGreen } else { VgaColor::White },
                VgaColor::Black,
                "{}{:02} | {:<21} | {:<6} | {:>3} | {:<12} | {} ticks\n",
                marker, t.id, t.name, core_str, t.uid, state_str, t.ticks_executed
            );
        }
        kprintln!("===================================================================================");
    }

    /// Obtiene el índice de la tarea activa para el núcleo que efectúa la llamada
    fn get_task_index_for_current_core(&self) -> Option<usize> {
        let core = crate::apic::current_apic_id();
        let core_idx = (core as usize).min(15);
        if core != 0 {
            if let Some(tid) = self.core_current_task[core_idx] {
                return self.tasks.iter().position(|t| t.id == tid);
            }
            None
        } else {
            if self.current_idx < self.tasks.len() {
                Some(self.current_idx)
            } else {
                None
            }
        }
    }

    /// Obtiene la tarea asignada actualmente a un núcleo por su APIC ID
    pub fn get_core_task(&self, core_id: usize) -> Option<TaskId> {
        if core_id < 16 {
            self.core_current_task[core_id]
        } else {
            None
        }
    }

    /// Selecciona una tarea lista para ser ejecutada concurrentemente en un Application Processor (AP)
    pub fn pick_task_for_core(&mut self, apic_id: u32) -> Option<(*mut u8, u64, TaskId)> {
        let now = get_ticks();
        // Despertar tareas durmientes
        for t in &mut self.tasks {
            if let TaskState::Sleeping(wake_tick) = t.state {
                if now >= wake_tick {
                    t.state = TaskState::Ready;
                }
            }
        }

        let core_idx = (apic_id as usize).min(15);
        for t in &mut self.tasks {
            if t.id != 0 && t.state == TaskState::Ready && t.running_on_core.is_none() {
                t.state = TaskState::Running;
                t.running_on_core = Some(apic_id);
                self.core_current_task[core_idx] = Some(t.id);
                return Some((t.stack_ptr, t.cr3, t.id));
            }
        }
        None
    }

    /// Devuelve las credenciales (UID, GID, username) de la tarea activa
    pub fn current_credentials(&self) -> (crate::task::Uid, crate::task::Gid, &'static str) {
        if let Some(idx) = self.get_task_index_for_current_core() {
            if let Some(t) = self.tasks.get(idx) {
                return (t.uid, t.gid, t.username);
            }
        }
        (crate::task::ROOT_UID, crate::task::ROOT_GID, "root")
    }

    /// Asigna las credenciales (UID, GID, username) a la tarea activa
    pub fn set_current_credentials(&mut self, uid: crate::task::Uid, gid: crate::task::Gid, username: &'static str) {
        if let Some(idx) = self.get_task_index_for_current_core() {
            if let Some(t) = self.tasks.get_mut(idx) {
                t.uid = uid;
                t.gid = gid;
                t.username = username;
            }
        }
    }

    /// Devuelve el TID de la tarea actualmente en ejecución
    pub fn current_task_id(&self) -> u64 {
        if let Some(idx) = self.get_task_index_for_current_core() {
            if let Some(t) = self.tasks.get(idx) {
                return t.id;
            }
        }
        0
    }

    /// Asigna un espacio de direcciones (CR3) dedicado a la tarea actualmente en ejecución
    pub fn set_current_cr3(&mut self, cr3: u64) {
        if let Some(idx) = self.get_task_index_for_current_core() {
            if let Some(t) = self.tasks.get_mut(idx) {
                t.cr3 = cr3;
            }
        }
    }

    /// Devuelve el CR3 asociado a la tarea actualmente en ejecución
    pub fn current_cr3(&self) -> u64 {
        if let Some(idx) = self.get_task_index_for_current_core() {
            if let Some(t) = self.tasks.get(idx) {
                return t.cr3;
            }
        }
        crate::memory::paging::kernel_cr3()
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
    let core = crate::apic::current_apic_id();
    let core_idx = (core as usize).min(15);

    if core != 0 {
        // Ejecución en un Application Processor (AP)
        let switch_args = {
            let mut sched = SCHEDULER.lock();
            if !sched.active {
                return;
            }
            let now = get_ticks();
            // Despertar tareas durmientes
            for t in &mut sched.tasks {
                if let TaskState::Sleeping(wake_tick) = t.state {
                    if now >= wake_tick {
                        t.state = TaskState::Ready;
                    }
                }
            }

            if let Some(tid) = sched.core_current_task[core_idx] {
                sched.core_current_task[core_idx] = None;
                if let Some(t) = sched.tasks.iter_mut().find(|task| task.id == tid) {
                    t.ticks_executed += 1;
                    if t.state == TaskState::Running {
                        t.state = TaskState::Ready;
                    }
                    t.running_on_core = None;
                    let prev_sp = &mut t.stack_ptr as *mut *mut u8;
                    // SAFETY: AP_IDLE_STACKS[core_idx] contiene la pila idle salvada en ap_worker_loop
                    let next_sp = unsafe { AP_IDLE_STACKS[core_idx] };
                    if !next_sp.is_null() {
                        Some((prev_sp, next_sp, crate::memory::paging::kernel_cr3()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((prev_sp, next_sp, next_cr3)) = switch_args {
            if next_cr3 != 0 && crate::memory::paging::read_cr3() != next_cr3 {
                // SAFETY: Restauración de tabla PML4 estándar del kernel
                unsafe { crate::memory::paging::write_cr3(next_cr3); }
            }
            // SAFETY: prev_sp apunta a la pila de la tarea saliente y next_sp a la pila idle del AP
            unsafe {
                switch_context(prev_sp, next_sp);
            }
        }
        return;
    }

    // BSP (Core 0): Round-Robin tradicional
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

        // 2. Buscar la siguiente tarea lista para Round-Robin en BSP
        // (Debe estar lista y no estar ejecutándose en un AP secundario)
        let total = sched.tasks.len();
        let mut next_idx = (curr + 1) % total;
        let mut found = false;

        for _ in 0..total {
            let t = &sched.tasks[next_idx];
            let can_run = (t.state == TaskState::Ready || t.state == TaskState::Running)
                && (t.running_on_core.is_none() || t.running_on_core == Some(0));
            if can_run {
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
            if curr != 0 {
                sched.tasks[curr].running_on_core = None;
            }
        }
        sched.tasks[next_idx].state = TaskState::Running;
        sched.tasks[next_idx].running_on_core = Some(0);
        sched.core_current_task[0] = Some(sched.tasks[next_idx].id);

        let prev_idx = curr;
        sched.current_idx = next_idx;

        let prev_sp = &mut sched.tasks[prev_idx].stack_ptr as *mut *mut u8;
        let next_sp = sched.tasks[next_idx].stack_ptr;
        let next_cr3 = sched.tasks[next_idx].cr3;
        Some((prev_sp, next_sp, next_cr3))
    }; // <-- ¡EL CERROJO DEL SCHEDULER SE LIBERA AQUÍ MISMO!

    if let Some((prev_sp, next_sp, next_cr3)) = context_switch_args {
        if next_cr3 != 0 && crate::memory::paging::read_cr3() != next_cr3 {
            // SAFETY: `next_cr3` es una tabla PML4 válida de tarea que incluye los mapeos supervisores del kernel.
            unsafe {
                crate::memory::paging::write_cr3(next_cr3);
            }
        }
        // SAFETY: `prev_sp` y `next_sp` apuntan a pilas de kernel válidas de tareas registradas y el cerrojo `SCHEDULER` ya fue liberado.
        unsafe {
            switch_context(prev_sp, next_sp);
        }
    }
}

pub fn sleep(ticks: u64) {
    let core = crate::apic::current_apic_id();
    let core_idx = (core as usize).min(15);
    let now = get_ticks();
    {
        let mut sched = SCHEDULER.lock();
        if core != 0 {
            if let Some(tid) = sched.core_current_task[core_idx] {
                if let Some(t) = sched.tasks.iter_mut().find(|task| task.id == tid) {
                    t.state = TaskState::Sleeping(now + ticks);
                }
            }
        } else {
            let curr = sched.current_idx;
            if curr < sched.tasks.len() {
                sched.tasks[curr].state = TaskState::Sleeping(now + ticks);
            }
        }
    }
    schedule();
}

pub fn yield_now() {
    schedule();
}

pub fn mark_finished_and_yield() {
    let core = crate::apic::current_apic_id();
    let core_idx = (core as usize).min(15);
    {
        let mut sched = SCHEDULER.lock();
        if core != 0 {
            if let Some(tid) = sched.core_current_task[core_idx] {
                if let Some(t) = sched.tasks.iter_mut().find(|task| task.id == tid) {
                    t.state = TaskState::Finished;
                    t.cr3 = crate::memory::paging::kernel_cr3();
                    t.running_on_core = None;
                }
            }
        } else {
            let curr = sched.current_idx;
            if curr < sched.tasks.len() {
                sched.tasks[curr].state = TaskState::Finished;
                sched.tasks[curr].cr3 = crate::memory::paging::kernel_cr3();
                if curr != 0 {
                    sched.tasks[curr].running_on_core = None;
                }
            }
        }
    }
    crate::memory::paging::switch_to_kernel_cr3();
    schedule();
}

pub fn schedule_tick() {
    schedule();
}

/// Bucle continuo de ejecución de tareas distribuidas en procesadores secundarios (APs)
pub fn ap_worker_loop(apic_id: u32) -> ! {
    let core_idx = (apic_id as usize).min(15);

    loop {
        let task_to_run = {
            let mut sched = SCHEDULER.lock();
            if !sched.active {
                None
            } else {
                sched.pick_task_for_core(apic_id)
            }
        };

        if let Some((task_sp, task_cr3, task_id)) = task_to_run {
            // Contabilizar ejecución en el núcleo APIC dentro de SMP
            {
                let mut smp = crate::apic::SMP.lock();
                for c in &mut smp.cores {
                    if c.apic_id == apic_id {
                        c.tasks_executed += 1;
                        break;
                    }
                }
            }

            if task_cr3 != 0 && crate::memory::paging::read_cr3() != task_cr3 {
                // SAFETY: Cambio seguro de CR3 al espacio de la tarea
                unsafe { crate::memory::paging::write_cr3(task_cr3); }
            }

            // SAFETY: Conmutación de contexto desde la pila idle del AP hacia la tarea seleccionada.
            // Cuando la tarea ceda el paso o termine, retornará exactamente a esta línea restaurando la pila idle.
            unsafe {
                switch_context(&mut AP_IDLE_STACKS[core_idx] as *mut *mut u8, task_sp);
            }

            // Restaurar CR3 base del kernel
            crate::memory::paging::switch_to_kernel_cr3();

            // Limpiar registro si la tarea finalizó
            {
                let mut sched = SCHEDULER.lock();
                if sched.core_current_task[core_idx] == Some(task_id) {
                    sched.core_current_task[core_idx] = None;
                }
            }
        } else {
            // No hay tareas listas: reposo en baja latencia con interrupciones habilitadas
            // SAFETY: 'sti; hlt' activa interrupciones y suspende la CPU hasta el próximo evento/IPI
            unsafe {
                core::arch::asm!("sti; hlt", options(nomem, nostack));
            }
        }
    }
}
