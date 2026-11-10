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
use crate::memory::frame_allocator::{FRAME_ALLOCATOR, PAGE_SIZE};
use crate::memory::paging::{
    self, USER_CODE_VADDR, USER_STACK_PAGES, USER_STACK_VADDR,
};
use crate::syscall::{
    SYS_READ, SYS_WRITE, SYS_OPEN, SYS_CLOSE, SYS_GETPID, SYS_SLEEP, SYS_EXIT, SYS_UPTIME,
};
use crate::task::scheduler::SCHEDULER;
use crate::vga::VgaColor;
use crate::kprint_color;

// Pila de kernel que el procesador usa cuando la tarea Ring 3 recibe una
// interrupción (se actualiza en el TSS.RSP0 antes de cada IRETQ)
pub const KERNEL_GUARD_STACK_SIZE: usize = 4096 * 4;
pub static mut KERNEL_GUARD_STACK: [u8; KERNEL_GUARD_STACK_SIZE] = [0u8; KERNEL_GUARD_STACK_SIZE];

// ──────────────────────────────────────────────────────────────────────────────
// Lanzador genérico a Ring 3 via IRETQ
//
// Construye en la pila actual el frame:
//   [ss, rsp_user, rflags, cs, rip]  (orden inverso porque PUSH baja RSP)
// Luego ejecuta IRETQ para saltar con privilegio Ring 3.
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "aarch64")]
pub fn jump_to_ring3(_entry: u64, _user_stack_top: u64) -> ! {
    loop {
        crate::arch::aarch64::cpu::wfi();
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Macro helper de syscall inline (equivalente a "int 0x80" pero con SYSCALL / SVC)
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(target_arch = "x86_64")]
macro_rules! syscall {
    ($nr:expr) => {{
        let ret: u64;
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
    ($nr:expr, $a0:expr, $a1:expr, $a2:expr) => {{
        let ret: u64;
        unsafe {
            asm!(
                "syscall",
                inout("rax") $nr => ret,
                in("rdi") $a0 as u64,
                in("rsi") $a1 as u64,
                in("rdx") $a2 as u64,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
        }
        ret
    }};
}

#[cfg(target_arch = "aarch64")]
macro_rules! syscall {
    ($nr:expr) => {{
        let ret: u64;
        unsafe {
            core::arch::asm!(
                "svc #0",
                inout("x8") $nr => ret,
                options(nostack)
            );
        }
        ret
    }};
    ($nr:expr, $a0:expr) => {{
        let ret: u64;
        let mut x0 = $a0 as u64;
        unsafe {
            core::arch::asm!(
                "svc #0",
                in("x8") $nr,
                inout("x0") x0 => ret,
                options(nostack)
            );
        }
        ret
    }};
    ($nr:expr, $a0:expr, $a1:expr) => {{
        let ret: u64;
        let mut x0 = $a0 as u64;
        unsafe {
            core::arch::asm!(
                "svc #0",
                in("x8") $nr,
                inout("x0") x0 => ret,
                in("x1") $a1 as u64,
                options(nostack)
            );
        }
        ret
    }};
    ($nr:expr, $a0:expr, $a1:expr, $a2:expr) => {{
        let ret: u64;
        let mut x0 = $a0 as u64;
        unsafe {
            core::arch::asm!(
                "svc #0",
                in("x8") $nr,
                inout("x0") x0 => ret,
                in("x1") $a1 as u64,
                in("x2") $a2 as u64,
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
    // Toda interacción con el sistema pasa por syscalls estándar.
    let m1 = b"[RING3] Hola Mundo desde proceso aislado de usuario (Ring 3)\n";
    syscall!(SYS_WRITE, 1u64, m1.as_ptr() as u64, m1.len() as u64);

    let m2 = b"[RING3] Memoria de kernel protegida: llamadas al sistema via MSR Syscall Gate\n";
    syscall!(SYS_WRITE, 1u64, m2.as_ptr() as u64, m2.len() as u64);

    let _pid = syscall!(SYS_GETPID);
    let _up = syscall!(SYS_UPTIME);

    syscall!(SYS_SLEEP, 50u64); // Dormir 500 ms

    let m3 = b"[RING3] Proceso de usuario completado exitosamente. Llamando SYS_EXIT(0)...\n";
    syscall!(SYS_WRITE, 1u64, m3.as_ptr() as u64, m3.len() as u64);

    syscall!(SYS_EXIT, 0u64);

    // Nunca debería llegar aquí
    loop {}
}

// ──────────────────────────────────────────────────────────────────────────────
// Tarea Ring 3 de crash: intenta acceder a memoria privilegiada del kernel.
// Demuestra que el kernel NO crashea (Anti-BSOD): solo muere este proceso.
// ──────────────────────────────────────────────────────────────────────────────
#[no_mangle]
extern "C" fn userland_crash_task() {
    let m = b"[RING3-CRASH] Iniciando prueba de aislamiento: intento de lectura nula en Ring 3...\n";
    syscall!(SYS_WRITE, 1u64, m.as_ptr() as u64, m.len() as u64);

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

/// Prepara un espacio de direcciones (`CR3`) dedicado para la tarea de usuario actual,
/// mapeando su código en `USER_CODE_VADDR` (solo lectura/ejecución `PAGE_USER`) y su pila
/// privada de 16 KiB en `USER_STACK_VADDR` (`PAGE_USER | PAGE_WRITABLE`).
fn prepare_isolated_user_space(fn_ptr: *const u8) -> (u64, u64, u64) {
    let proc_cr3 = paging::create_process_address_space()
        .expect("[RING3] Sin memoria para crear tabla CR3 de proceso");

    let code_frame = FRAME_ALLOCATOR
        .allocate_frame()
        .expect("[RING3] Sin marcos fisicos para codigo de usuario");

    // SAFETY: Se copia una página de 4 KiB de instrucciones máquina de la función Ring 3 al marco físico privado.
    unsafe {
        core::ptr::copy_nonoverlapping(fn_ptr, code_frame as *mut u8, 4096);
    }

    paging::map_user_page(proc_cr3, USER_CODE_VADDR, code_frame as u64, false)
        .expect("[RING3] Error mapeando pagina de codigo USER_CODE_VADDR");

    for i in 0..USER_STACK_PAGES {
        let stack_frame = FRAME_ALLOCATOR
            .allocate_frame()
            .expect("[RING3] Sin marcos fisicos para pila de usuario");
        let vaddr = USER_STACK_VADDR + (i * PAGE_SIZE) as u64;
        paging::map_user_page(proc_cr3, vaddr, stack_frame as u64, true)
            .expect("[RING3] Error mapeando pagina de pila USER_STACK_VADDR");
    }

    let user_stack_top = USER_STACK_VADDR + (USER_STACK_PAGES * PAGE_SIZE) as u64;
    SCHEDULER.lock().set_current_cr3(proc_cr3);

    // SAFETY: `proc_cr3` contiene los mapeos supervisores del kernel (`PAGE_USER = 0`) y las páginas de usuario aisladas (`PAGE_USER = 1`).
    unsafe {
        paging::write_cr3(proc_cr3);
    }

    (proc_cr3, USER_CODE_VADDR, user_stack_top)
}

/// Worker que corre la tarea normal de userland en Ring 3
pub fn spawn_normal_user_task() {
    let (proc_cr3, entry, user_stack_top) =
        prepare_isolated_user_space(userland_normal_task as *const u8);

    let guard_top = core::ptr::addr_of!(KERNEL_GUARD_STACK) as u64 + KERNEL_GUARD_STACK_SIZE as u64;
    set_tss_rsp0(guard_top);

    kprint_color!(
        VgaColor::Yellow, VgaColor::Black,
        "\n[RING3] Lanzando proceso normal en Ring 3 via IRETQ (CR3 aislado)...\n"
    );
    kprint_color!(
        VgaColor::LightCyan, VgaColor::Black,
        "  CR3:    {:#x} (PAGE_USER solo en proceso)\n  Entry:  {:#x}\n  Stack:  {:#x}\n  CPL:    Ring 3 (User Mode)\n",
        proc_cr3, entry, user_stack_top
    );

    let exit_code = unsafe { crate::syscall::enter_ring3_with_return(entry, user_stack_top) };
    kprint_color!(
        VgaColor::LightGreen, VgaColor::Black,
        "[RING3] Proceso finalizado con código {}.\n",
        exit_code
    );
}

/// Worker que corre la tarea de crash de userland en Ring 3 (demuestra aislamiento)
pub fn spawn_crash_user_task() {
    let (proc_cr3, entry, user_stack_top) =
        prepare_isolated_user_space(userland_crash_task as *const u8);

    let guard_top = core::ptr::addr_of!(KERNEL_GUARD_STACK) as u64 + KERNEL_GUARD_STACK_SIZE as u64;
    set_tss_rsp0(guard_top);

    kprint_color!(
        VgaColor::LightRed, VgaColor::Black,
        "\n[RING3-CRASH] Lanzando proceso crasheador en Ring 3 (CR3={:#x}, Anti-BSOD test)...\n",
        proc_cr3
    );
    kprint_color!(
        VgaColor::Yellow, VgaColor::Black,
        "  El proceso intentara leer 0x0 (Supervisor PAGE_USER=0) -> #PF desde Ring 3\n  El kernel deberia matar solo este proceso.\n"
    );

    let exit_code = unsafe { crate::syscall::enter_ring3_with_return(entry, user_stack_top) };
    kprint_color!(
        VgaColor::LightGreen, VgaColor::Black,
        "[RING3-CRASH] Sandbox contenido con éxito. Código de salida: {}. Kernel intacto.\n",
        exit_code
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Shell nativo interactivo en Ring 3
// ──────────────────────────────────────────────────────────────────────────────
#[no_mangle]
extern "C" fn userland_shell_task() {
    let banner = b"\n============================================================\n  BEAVER OS - Native Userland Shell (Ring 3)\n  Modo no privilegiado (CPL=3) | Syscalls MSR / POSIX\n  Escribe 'help' para ver comandos, o 'exit' para salir.\n============================================================\n";
    syscall!(SYS_WRITE, 1u64, banner.as_ptr() as u64, banner.len() as u64);

    let prompt = b"beaver-sh (Ring 3)$ ";
    let mut line_buf = [0u8; 128];

    loop {
        syscall!(SYS_WRITE, 1u64, prompt.as_ptr() as u64, prompt.len() as u64);

        for b in line_buf.iter_mut() {
            *b = 0;
        }

        let n = syscall!(SYS_READ, 0u64, line_buf.as_mut_ptr() as u64, 128u64) as usize;
        if n == 0 {
            continue;
        }

        let raw_line = &line_buf[..n];
        let line_len = raw_line.iter().position(|&c| c == b'\n' || c == b'\r').unwrap_or(n);
        let cmd_bytes = &raw_line[..line_len];
        if cmd_bytes.is_empty() {
            continue;
        }

        if cmd_bytes == b"help" {
            let help_msg = b"Comandos disponibles en Ring 3 Shell:\n  help        - Lista de comandos usermode\n  uptime      - Tiempo de actividad del kernel (SYS_UPTIME)\n  whoami      - Tarea y privilegio actual (SYS_GETPID)\n  echo <txt>  - Imprime texto en pantalla (SYS_WRITE)\n  cat <ruta>  - Lee archivo VFS (SYS_OPEN, SYS_READ, SYS_CLOSE)\n  exit        - Regresa a la consola del supervisor (SYS_EXIT)\n";
            syscall!(SYS_WRITE, 1u64, help_msg.as_ptr() as u64, help_msg.len() as u64);
        } else if cmd_bytes == b"uptime" {
            let up = syscall!(SYS_UPTIME);
            let mut up_msg = [0u8; 64];
            let prefix = b"Uptime del sistema: ";
            let suffix = b" segundos (obtenido via SYS_UPTIME)\n";
            let mut pos = 0;
            for &b in prefix { up_msg[pos] = b; pos += 1; }
            let mut digits = [0u8; 10];
            let mut dcount = 0;
            let mut val = up;
            if val == 0 { digits[0] = b'0'; dcount = 1; }
            else {
                while val > 0 && dcount < 10 {
                    digits[dcount] = b'0' + (val % 10) as u8;
                    val /= 10;
                    dcount += 1;
                }
            }
            while dcount > 0 {
                dcount -= 1;
                up_msg[pos] = digits[dcount];
                pos += 1;
            }
            for &b in suffix { up_msg[pos] = b; pos += 1; }
            syscall!(SYS_WRITE, 1u64, up_msg.as_ptr() as u64, pos as u64);
        } else if cmd_bytes == b"whoami" {
            let pid = syscall!(SYS_GETPID);
            let mut pid_msg = [0u8; 64];
            let prefix = b"Usuario: ring3-user (TID: ";
            let suffix = b", Privilegio: Ring 3 / CPL=3)\n";
            let mut pos = 0;
            for &b in prefix { pid_msg[pos] = b; pos += 1; }
            let mut digits = [0u8; 10];
            let mut dcount = 0;
            let mut val = pid;
            if val == 0 { digits[0] = b'0'; dcount = 1; }
            else {
                while val > 0 && dcount < 10 {
                    digits[dcount] = b'0' + (val % 10) as u8;
                    val /= 10;
                    dcount += 1;
                }
            }
            while dcount > 0 {
                dcount -= 1;
                pid_msg[pos] = digits[dcount];
                pos += 1;
            }
            for &b in suffix { pid_msg[pos] = b; pos += 1; }
            syscall!(SYS_WRITE, 1u64, pid_msg.as_ptr() as u64, pos as u64);
        } else if cmd_bytes.starts_with(b"echo ") {
            let text = &cmd_bytes[5..];
            syscall!(SYS_WRITE, 1u64, text.as_ptr() as u64, text.len() as u64);
            syscall!(SYS_WRITE, 1u64, b"\n".as_ptr() as u64, 1u64);
        } else if cmd_bytes.starts_with(b"cat ") {
            let file_name = &cmd_bytes[4..];
            let mut path_buf = [0u8; 64];
            if file_name.len() < 60 {
                let mut ppos = 0;
                for &b in file_name { path_buf[ppos] = b; ppos += 1; }
                path_buf[ppos] = 0;

                let fd = syscall!(SYS_OPEN, path_buf.as_ptr() as u64, 0u64);
                if (fd as i64) < 0 {
                    let err = b"Error: No se pudo abrir el archivo en VFS\n";
                    syscall!(SYS_WRITE, 2u64, err.as_ptr() as u64, err.len() as u64);
                } else {
                    let mut file_buf = [0u8; 256];
                    loop {
                        let read_n = syscall!(SYS_READ, fd, file_buf.as_mut_ptr() as u64, 256u64) as usize;
                        if read_n == 0 { break; }
                        syscall!(SYS_WRITE, 1u64, file_buf.as_ptr() as u64, read_n as u64);
                    }
                    syscall!(SYS_CLOSE, fd);
                    syscall!(SYS_WRITE, 1u64, b"\n".as_ptr() as u64, 1u64);
                }
            }
        } else if cmd_bytes == b"exit" {
            let exit_msg = b"Saliendo del shell de usuario Ring 3. Retornando al supervisor...\n";
            syscall!(SYS_WRITE, 1u64, exit_msg.as_ptr() as u64, exit_msg.len() as u64);
            syscall!(SYS_EXIT, 0u64);
            break;
        } else {
            let unknown = b"Comando no reconocido en Ring 3. Escribe 'help'.\n";
            syscall!(SYS_WRITE, 2u64, unknown.as_ptr() as u64, unknown.len() as u64);
        }
    }
}

/// Worker que lanza el Shell interactivo en Ring 3
#[cfg(target_arch = "x86_64")]
pub fn spawn_userland_shell() {
    let (proc_cr3, entry, user_stack_top) =
        prepare_isolated_user_space(userland_shell_task as *const u8);

    let guard_top = core::ptr::addr_of!(KERNEL_GUARD_STACK) as u64 + KERNEL_GUARD_STACK_SIZE as u64;
    set_tss_rsp0(guard_top);

    kprint_color!(
        VgaColor::LightGreen, VgaColor::Black,
        "\n[RING3-SHELL] Conmutando a espacio de usuario aislado (CR3={:#x}, CPL=3)...\n",
        proc_cr3
    );

    let exit_code = unsafe { crate::syscall::enter_ring3_with_return(entry, user_stack_top) };

    kprint_color!(
        VgaColor::LightGreen, VgaColor::Black,
        "\n[RING3-SHELL] Sesión de usuario finalizada (código {}). De vuelta en el supervisor.\n",
        exit_code
    );
}

#[cfg(target_arch = "aarch64")]
pub fn spawn_userland_shell() {
    crate::kprintln!("\n[AArch64] Shell interactivo EL0 iniciado.");
}

