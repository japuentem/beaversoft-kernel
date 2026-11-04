// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Ring 3 User Mode Launcher & Isolation Sandbox
//
// Cómo funciona el salto a Ring 3 (IRETQ trick):
//   Para ejecutar código en Ring 3, el kernel construye en la pila de kernel
//   un frame de interrupción falso y ejecuta IRETQ. El procesador:
//     1. Restaura CS  = User Code selector (0x1B = 0x18 | RPL=3)
//     2. Restaura SS  = User Data selector (0x23 = 0x20 | RPL=3)
//     3. Cambia CPL   → Ring 3
//     4. Salta a RIP  = función de usuario
//     5. Activa RSP   = pila de usuario asignada por el kernel
//
// Aislamiento Anti-BSOD:
//   Si el código de Ring 3 hace algo ilegal (acceso a memoria privilegiada,
//   instrucción no permitida, divide by zero), el CPU lanza #GP/#PF con el
//   bit de usuario en el error code. El handler del kernel detecta esto,
//   MATA sólo esa tarea, y devuelve control al scheduler – el sistema sigue.
// ============================================================================

use core::arch::asm;
use crate::gdt::{SELECTOR_USER_CODE, SELECTOR_USER_DATA, set_tss_rsp0};
use crate::syscall::{SYS_WRITE, SYS_GETPID, SYS_SLEEP, SYS_EXIT};
use crate::vga::VgaColor;
use crate::kprint_color;

// ──────────────────────────────────────────────────────────────────────────────
// Pila de usuario (privada por tarea Ring 3)
// ──────────────────────────────────────────────────────────────────────────────
const USER_STACK_SIZE: usize = 4096 * 4; // 16 KiB
static mut USER_STACK_A: [u8; USER_STACK_SIZE] = [0u8; USER_STACK_SIZE];
static mut USER_STACK_B: [u8; USER_STACK_SIZE] = [0u8; USER_STACK_SIZE];

// Pila de kernel que el procesador usa cuando la tarea Ring 3 recibe una
// interrupción (se actualiza en el TSS.RSP0 antes de cada IRETQ)
const KERNEL_GUARD_STACK_SIZE: usize = 4096 * 4;
static mut KERNEL_GUARD_STACK: [u8; KERNEL_GUARD_STACK_SIZE] = [0u8; KERNEL_GUARD_STACK_SIZE];

// ──────────────────────────────────────────────────────────────────────────────
// Lanzador genérico a Ring 3 via IRETQ
//
// Construye en la pila actual el frame:
//   [ss, rsp_user, rflags, cs, rip]  (orden inverso porque PUSH baja RSP)
// Luego ejecuta IRETQ para saltar con privilegio Ring 3.
// ──────────────────────────────────────────────────────────────────────────────
pub fn jump_to_ring3(entry: u64, user_stack_top: u64) -> ! {
    // Actualizar RSP0 en el TSS para que las interrupciones del proceso Ring 3
    // usen nuestra pila de guardia de kernel
    let guard_top = core::ptr::addr_of!(KERNEL_GUARD_STACK) as u64 + KERNEL_GUARD_STACK_SIZE as u64;
    set_tss_rsp0(guard_top);

    let cs = SELECTOR_USER_CODE as u64;
    let ss = SELECTOR_USER_DATA as u64;
    let rflags: u64 = 0x202; // IF=1, reserved bit

    // SAFETY: Se construye un marco de interrupción válido en la pila con selectores RPL=3 (`cs`, `ss`) y se ejecuta `iretq` hacia `entry`.
    unsafe {
        asm!(
            // Construir frame de IRETQ en la pila (de abajo hacia arriba):
            //   push SS
            //   push RSP  (pila de usuario)
            //   push RFLAGS
            //   push CS
            //   push RIP  (entry point)
            "push {ss}",
            "push {rsp_user}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",
            "iretq",
            ss       = in(reg) ss,
            rsp_user = in(reg) user_stack_top,
            rflags   = in(reg) rflags,
            cs       = in(reg) cs,
            rip      = in(reg) entry,
            options(noreturn)
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Macro helper de syscall inline (equivalente a "int 0x80" pero con SYSCALL)
// ──────────────────────────────────────────────────────────────────────────────
macro_rules! syscall {
    ($nr:expr) => {{
        let ret: u64;
        // SAFETY: Invocación de la instrucción `syscall` con la convención de registros de BeaverOS y MSRs previamente configurados.
        unsafe {
            asm!(
                "syscall",
                inout("rax") $nr => ret,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
        }
        ret
    }};
    ($nr:expr, $a0:expr) => {{
        let ret: u64;
        // SAFETY: Invocación de `syscall` pasando 1 argumento en `rdi` y preservando la pila de usuario.
        unsafe {
            asm!(
                "syscall",
                inout("rax") $nr => ret,
                in("rdi") $a0 as u64,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
        }
        ret
    }};
    ($nr:expr, $a0:expr, $a1:expr) => {{
        let ret: u64;
        // SAFETY: Invocación de `syscall` pasando 2 argumentos en `rdi` y `rsi` según la ABI del kernel.
        unsafe {
            asm!(
                "syscall",
                inout("rax") $nr => ret,
                in("rdi") $a0 as u64,
                in("rsi") $a1 as u64,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
        }
        ret
    }};
}

// ──────────────────────────────────────────────────────────────────────────────
// Tarea Ring 3 normal: demuestra syscalls exitosas desde Ring 3
// ──────────────────────────────────────────────────────────────────────────────
#[no_mangle]
extern "C" fn userland_normal_task() {
    // Esta función corre en Ring 3 (CPL=3).
    // No puede acceder a puertos I/O ni a memoria del kernel directamente.
    // Toda interacción con el sistema pasa por syscalls.

    syscall!(SYS_WRITE, 0u64); // "Hello from Ring 3 userland!"
    syscall!(SYS_WRITE, 1u64); // "User process running in isolation."
    syscall!(SYS_GETPID);
    syscall!(SYS_WRITE, 2u64); // "Syscall bridge completed."
    syscall!(SYS_SLEEP, 200u64); // Dormir 2 segundos
    syscall!(SYS_WRITE, 3u64); // "Kernel memory protected."
    syscall!(SYS_EXIT, 0u64);   // Terminar limpiamente

    // Nunca debería llegar aquí
    loop {}
}

// ──────────────────────────────────────────────────────────────────────────────
// Tarea Ring 3 de crash: intenta acceder a memoria privilegiada del kernel.
// Demuestra que el kernel NO crashea (Anti-BSOD): solo muere este proceso.
// ──────────────────────────────────────────────────────────────────────────────
#[no_mangle]
extern "C" fn userland_crash_task() {
    syscall!(SYS_WRITE, 1u64); // "User process running in isolation."

    // Intentar leer desde la dirección 0x0 (NULL deref en Ring 3)
    // Esto generará un #PF con CPL=3. El kernel detectará que es un fallo
    // de usuario y matará sólo este proceso (no habrá BSOD).
    // SAFETY: Desreferencia nula deliberada en Ring 3 (CPL=3) para verificar que la excepción `#PF` aísla y termina el proceso sin afectar al kernel.
    unsafe {
        let null_ptr: *const u64 = 0x0 as *const u64;
        let _val = core::ptr::read_volatile(null_ptr); // → #PF desde Ring 3
    }

    // Nunca debería llegar aquí
    syscall!(SYS_EXIT, 1u64);
    loop {}
}

// ──────────────────────────────────────────────────────────────────────────────
// Funciones de entrada al scheduler (Ring 0) que lanzan tareas Ring 3
// Estas son las que se pasan a task::spawn() o se llaman directamente.
// ──────────────────────────────────────────────────────────────────────────────

/// Worker que corre la tarea normal de userland en Ring 3
pub fn spawn_normal_user_task() {
    let entry = userland_normal_task as *const () as u64;
    let user_stack_top = core::ptr::addr_of!(USER_STACK_A) as u64 + USER_STACK_SIZE as u64;

    kprint_color!(
        VgaColor::Yellow, VgaColor::Black,
        "\n[RING3] Lanzando proceso normal en Ring 3 via IRETQ...\n"
    );
    kprint_color!(
        VgaColor::LightCyan, VgaColor::Black,
        "  Entry:  {:#x}\n  Stack:  {:#x}\n  CPL:    Ring 3 (User Mode)\n",
        entry, user_stack_top
    );

    jump_to_ring3(entry, user_stack_top);
}

/// Worker que corre la tarea de crash de userland en Ring 3 (demuestra aislamiento)
pub fn spawn_crash_user_task() {
    let entry = userland_crash_task as *const () as u64;
    let user_stack_top = core::ptr::addr_of!(USER_STACK_B) as u64 + USER_STACK_SIZE as u64;

    kprint_color!(
        VgaColor::LightRed, VgaColor::Black,
        "\n[RING3-CRASH] Lanzando proceso crasheador en Ring 3 (Anti-BSOD test)...\n"
    );
    kprint_color!(
        VgaColor::Yellow, VgaColor::Black,
        "  El proceso intentara una NULL deref -> #PF desde Ring 3\n  El kernel deberia matar solo este proceso.\n"
    );

    jump_to_ring3(entry, user_stack_top);
}
