// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Syscall/Sysret MSR Setup & Ring 3 → Ring 0 System Call Gate
//
// Syscall calling convention (64-bit):
//   RAX = syscall number
//   RDI = arg0, RSI = arg1, RDX = arg2
//   Return value in RAX
//
// Syscall numbers:
//   0 = SYS_WRITE   – imprime mensaje en VGA
//   1 = SYS_GETPID  – devuelve el TID de la tarea actual
//   2 = SYS_SLEEP   – duerme N ticks
//   3 = SYS_EXIT    – termina la tarea usermode actual
// ============================================================================

use core::arch::asm;
use crate::vga::VgaColor;
use crate::kprint_color;

// ──────────────────────────────────────────────────────────────────────────────
// Constantes de MSR
// ──────────────────────────────────────────────────────────────────────────────
const MSR_EFER:  u32 = 0xC000_0080;
const MSR_STAR:  u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_FMASK: u32 = 0xC000_0084;

// ──────────────────────────────────────────────────────────────────────────────
// Números de syscall soportados
// ──────────────────────────────────────────────────────────────────────────────
pub const SYS_WRITE:    u64 = 0;
pub const SYS_GETPID:   u64 = 1;
pub const SYS_SLEEP:    u64 = 2;
pub const SYS_EXIT:     u64 = 3;
pub const SYS_IPC_SEND: u64 = 4;
pub const SYS_IPC_RECV: u64 = 5;
pub const SYS_FUTEX_WAIT:  u64 = 6;
pub const SYS_FUTEX_WAKE:  u64 = 7;
pub const SYS_MUTEX_LOCK:  u64 = 8;
pub const SYS_MUTEX_UNLOCK:u64 = 9;
pub const SYS_SEM_WAIT:    u64 = 10;
pub const SYS_SEM_POST:    u64 = 11;

// ──────────────────────────────────────────────────────────────────────────────
// Helpers de lectura/escritura de MSR
// ──────────────────────────────────────────────────────────────────────────────
#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") lo,
        out("edx") hi,
        options(nostack, preserves_flags)
    );
    (hi as u64) << 32 | lo as u64
}

#[inline]
unsafe fn wrmsr(msr: u32, value: u64) {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") lo,
        in("edx") hi,
        options(nostack, preserves_flags)
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Pila del kernel para syscalls (8 KiB, alineada a 16 bytes)
// ──────────────────────────────────────────────────────────────────────────────
const SYSCALL_STACK_SIZE: usize = 8 * 1024;

#[repr(C, align(16))]
struct SyscallStack([u8; SYSCALL_STACK_SIZE]);

static mut SYSCALL_KERNEL_STACK: SyscallStack = SyscallStack([0u8; SYSCALL_STACK_SIZE]);

/// Cima de la pila de syscall, referenciada desde el stub de assembly
#[no_mangle]
pub static mut SYSCALL_KERNEL_STACK_TOP: u64 = 0;

// ──────────────────────────────────────────────────────────────────────────────
// Inicialización del subsistema de syscalls
// ──────────────────────────────────────────────────────────────────────────────
pub fn init_syscalls() {
    unsafe {
        // Inicializar puntero de cima de pila
        let base = core::ptr::addr_of_mut!(SYSCALL_KERNEL_STACK) as u64;
        SYSCALL_KERNEL_STACK_TOP = base + SYSCALL_STACK_SIZE as u64;

        // 1. Habilitar bit SCE (System Call Enable) en IA32_EFER
        let efer = rdmsr(MSR_EFER);
        wrmsr(MSR_EFER, efer | 0x1);

        // 2. Configurar IA32_STAR:
        //    bits 63:48 = base de selectores User  → 0x18 (User Code = 0x18|3 = 0x1B)
        //    bits 47:32 = selector Kernel Code      → 0x08
        //
        //    GDT extendida (gdt.rs):
        //      0x00 Null | 0x08 KernelCode | 0x10 KernelData
        //      0x18 UserCode (Ring3) | 0x20 UserData (Ring3)
        //      0x28 TSS-low | 0x30 TSS-high
        let star: u64 = (0x0018_u64 << 48) | (0x0008_u64 << 32);
        wrmsr(MSR_STAR, star);

        // 3. IA32_LSTAR = dirección del handler en Ring 0
        let handler_addr = syscall_handler_asm as *const () as u64;
        wrmsr(MSR_LSTAR, handler_addr);

        // 4. IA32_FMASK: enmascarar IF (bit 9) y DF (bit 10) al entrar al kernel
        wrmsr(MSR_FMASK, 0x0000_0000_0000_0600);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Stub de assembly: puente Ring 3 → Ring 0
//
// Al entrar por SYSCALL:
//   CS/SS → kernel  (via STAR)
//   RIP   → esta función  (via LSTAR)
//   RCX   = RIP de retorno al userland
//   R11   = RFLAGS guardado
//   RSP   = pila de usuario  ← la intercambiamos inmediatamente
// ──────────────────────────────────────────────────────────────────────────────
core::arch::global_asm!(r#"
.section .text
.global syscall_handler_asm
syscall_handler_asm:
    /* 1. Guardar RSP usuario y saltar a pila kernel */
    mov r10, rsp
    lea rsp, [rip + SYSCALL_KERNEL_STACK_TOP]
    mov rsp, [rsp]

    /* 2. Preservar contexto de usuario */
    push rcx        /* RIP retorno (sysretq lo necesita en RCX) */
    push r11        /* RFLAGS guardado */
    push r10        /* RSP usuario */
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15

    /* 3. Llamar al dispatcher Rust
          syscall_dispatch(nr=rdi, a0=rsi, a1=rdx, a2=r8)
          RAX = syscall number → mover a RDI */
    mov rdi, rax
    /* rsi = arg0 ya está en RSI */
    /* rdx = arg1 ya está en RDX */
    /* r8  = arg2 (RCX original ya fue sobreescrito por SYSCALL) */
    call syscall_dispatch

    /* 4. Restaurar contexto de usuario */
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    pop r10         /* RSP usuario */
    pop r11         /* RFLAGS */
    pop rcx         /* RIP retorno */

    /* 5. Restaurar RSP usuario y volver a Ring 3 */
    mov rsp, r10
    sysretq
"#);

// Declaración extern para que Rust pueda tomar su dirección
extern "C" {
    fn syscall_handler_asm();
}

// ──────────────────────────────────────────────────────────────────────────────
// Despachador Rust de syscalls (Ring 0, llamado desde el stub de assembly)
// ──────────────────────────────────────────────────────────────────────────────
static USERLAND_MESSAGES: &[&str] = &[
    "Hello from Ring 3 userland!",
    "User process running in privilege isolation.",
    "Syscall bridge: Ring3 -> Ring0 -> Ring3 completed.",
    "Kernel memory protected by CPU privilege rings.",
];

// Mutexes y Semáforos para primitivas de sincronización de usuario en Ring 3
static USER_MUTEXES: [core::sync::atomic::AtomicU32; 16] = [
    core::sync::atomic::AtomicU32::new(0), core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0), core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0), core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0), core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0), core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0), core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0), core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0), core::sync::atomic::AtomicU32::new(0),
];

static USER_SEMAPHORES: [core::sync::atomic::AtomicU32; 16] = [
    core::sync::atomic::AtomicU32::new(1), core::sync::atomic::AtomicU32::new(1),
    core::sync::atomic::AtomicU32::new(1), core::sync::atomic::AtomicU32::new(1),
    core::sync::atomic::AtomicU32::new(1), core::sync::atomic::AtomicU32::new(1),
    core::sync::atomic::AtomicU32::new(1), core::sync::atomic::AtomicU32::new(1),
    core::sync::atomic::AtomicU32::new(1), core::sync::atomic::AtomicU32::new(1),
    core::sync::atomic::AtomicU32::new(1), core::sync::atomic::AtomicU32::new(1),
    core::sync::atomic::AtomicU32::new(1), core::sync::atomic::AtomicU32::new(1),
    core::sync::atomic::AtomicU32::new(1), core::sync::atomic::AtomicU32::new(1),
];

#[no_mangle]
pub extern "C" fn syscall_dispatch(nr: u64, arg0: u64, arg1: u64, _arg2: u64) -> u64 {
    match nr {
        SYS_WRITE => {
            let idx = (arg0 as usize) % USERLAND_MESSAGES.len();
            kprint_color!(
                VgaColor::LightGreen, VgaColor::Black,
                "  [SYSCALL] SYS_WRITE: {}\n",
                USERLAND_MESSAGES[idx]
            );
            0
        }

        SYS_GETPID => {
            let tid = crate::task::scheduler::SCHEDULER.lock().current_task_id();
            kprint_color!(
                VgaColor::LightCyan, VgaColor::Black,
                "  [SYSCALL] SYS_GETPID -> TID={}\n",
                tid
            );
            tid
        }

        SYS_SLEEP => {
            crate::task::scheduler::sleep(arg0);
            0
        }

        SYS_EXIT => {
            kprint_color!(
                VgaColor::LightCyan, VgaColor::Black,
                "  [SYSCALL] SYS_EXIT(code={}) -> terminando proceso usermode\n",
                arg0
            );
            crate::task::scheduler::mark_finished_and_yield();
            0
        }

        SYS_IPC_SEND => {
            let channel = match arg0 {
                0 => "system",
                1 => "telemetry",
                _ => "events",
            };
            let tid = crate::task::scheduler::SCHEDULER.lock().current_task_id();
            let (uid, _, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
            let mut ipc = crate::ipc::IPC_MANAGER.lock();
            match ipc.send_message(channel, tid, uid, "Mensaje de Ring 3 via SYS_IPC_SEND") {
                Ok(msg_id) => {
                    kprint_color!(
                        VgaColor::LightGreen, VgaColor::Black,
                        "  [SYSCALL] SYS_IPC_SEND -> Msg ID #{} en canal '{}'\n",
                        msg_id, channel
                    );
                    msg_id
                }
                Err(_) => u64::MAX,
            }
        }

        SYS_IPC_RECV => {
            let channel = match arg0 {
                0 => "system",
                1 => "telemetry",
                _ => "events",
            };
            let mut ipc = crate::ipc::IPC_MANAGER.lock();
            match ipc.receive_message(channel) {
                Ok(msg) => {
                    kprint_color!(
                        VgaColor::LightGreen, VgaColor::Black,
                        "  [SYSCALL] SYS_IPC_RECV -> Msg ID #{}: '{}'\n",
                        msg.id, msg.payload
                    );
                    msg.id
                }
                Err(_) => u64::MAX,
            }
        }

        SYS_FUTEX_WAIT => {
            // arg0: dirección u32 en memoria de usuario, arg1: valor esperado
            let user_ptr = arg0 as *const u32;
            let expected = arg1 as u32;
            let current = unsafe { *user_ptr };
            if current == expected {
                crate::task::scheduler::sleep(1);
            }
            0
        }

        SYS_FUTEX_WAKE => {
            // arg0: dirección u32 en memoria de usuario, arg1: número de hilos a despertar
            // Despertamos hilos cediendo el quantum al scheduler
            crate::task::scheduler::yield_now();
            arg1.min(1)
        }

        SYS_MUTEX_LOCK => {
            let idx = (arg0 as usize) % 16;
            while USER_MUTEXES[idx].compare_exchange(0, 1, core::sync::atomic::Ordering::Acquire, core::sync::atomic::Ordering::Relaxed).is_err() {
                crate::task::scheduler::yield_now();
            }
            0
        }

        SYS_MUTEX_UNLOCK => {
            let idx = (arg0 as usize) % 16;
            USER_MUTEXES[idx].store(0, core::sync::atomic::Ordering::Release);
            0
        }

        SYS_SEM_WAIT => {
            let idx = (arg0 as usize) % 16;
            loop {
                let current = USER_SEMAPHORES[idx].load(core::sync::atomic::Ordering::Relaxed);
                if current > 0 {
                    if USER_SEMAPHORES[idx].compare_exchange(current, current - 1, core::sync::atomic::Ordering::Acquire, core::sync::atomic::Ordering::Relaxed).is_ok() {
                        break;
                    }
                } else {
                    crate::task::scheduler::yield_now();
                }
            }
            0
        }

        SYS_SEM_POST => {
            let idx = (arg0 as usize) % 16;
            USER_SEMAPHORES[idx].fetch_add(1, core::sync::atomic::Ordering::Release);
            0
        }

        _ => {
            kprint_color!(
                VgaColor::LightRed, VgaColor::Black,
                "  [SYSCALL] DESCONOCIDA nr={}\n",
                nr
            );
            u64::MAX
        }
    }
}
