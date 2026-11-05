// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Syscall/Sysret MSR Setup & Ring 3 → Ring 0 System Call Gate
//
// Syscall calling convention (64-bit AMD64 System V):
//   RAX = syscall number
//   RDI = arg0, RSI = arg1, RDX = arg2
//   Return value in RAX
//
// Syscall numbers (POSIX compliant):
//   0 = SYS_READ    – lee de fd (0=stdin / teclado, >=3=archivo VFS)
//   1 = SYS_WRITE   – escribe en fd (1=stdout, 2=stderr, >=3=archivo VFS)
//   2 = SYS_OPEN    – abre archivo en VFS devolviendo fd
//   3 = SYS_CLOSE   – cierra descriptor fd
//   4 = SYS_STAT    – consulta metadatos de archivo
//   8 = SYS_LSEEK   – desplaza cursor de lectura/escritura en fd
//  24 = SYS_YIELD   – cede voluntariamente el quantum del scheduler
//  39 = SYS_GETPID  – devuelve el TID/PID del proceso actual
//  59 = SYS_EXECVE  – ejecuta binario ELF64 en Ring 3
//  60 = SYS_EXIT    – termina el proceso de usuario actual
//  99 = SYS_UPTIME  – devuelve el tiempo de actividad del sistema en segundos
// 102 = SYS_SLEEP   – suspende la ejecución por N ticks
// ============================================================================

extern crate alloc;

use core::arch::asm;
use alloc::string::String;
use alloc::vec::Vec;
use crate::vga::VgaColor;
use crate::{kprint, kprint_color};
use crate::sync::SpinMutex;
use crate::fs::vfs::VFS;

// ──────────────────────────────────────────────────────────────────────────────
// Constantes de MSR
// ──────────────────────────────────────────────────────────────────────────────
const MSR_EFER:  u32 = 0xC000_0080;
const MSR_STAR:  u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_FMASK: u32 = 0xC000_0084;

// ──────────────────────────────────────────────────────────────────────────────
// Números de syscall estándar POSIX y extensiones Beaver OS
// ──────────────────────────────────────────────────────────────────────────────
pub const SYS_READ:     u64 = 0;
pub const SYS_WRITE:    u64 = 1;
pub const SYS_OPEN:     u64 = 2;
pub const SYS_CLOSE:    u64 = 3;
pub const SYS_STAT:     u64 = 4;
pub const SYS_LSEEK:    u64 = 8;
pub const SYS_YIELD:    u64 = 24;
pub const SYS_GETPID:   u64 = 39;
pub const SYS_EXECVE:   u64 = 59;
pub const SYS_EXIT:     u64 = 60;
pub const SYS_UPTIME:   u64 = 99;
pub const SYS_SLEEP:    u64 = 102;

// IPC y sincronización en Ring 3
pub const SYS_IPC_SEND:     u64 = 110;
pub const SYS_IPC_RECV:     u64 = 111;
pub const SYS_FUTEX_WAIT:   u64 = 112;
pub const SYS_FUTEX_WAKE:   u64 = 113;
pub const SYS_MUTEX_LOCK:   u64 = 114;
pub const SYS_MUTEX_UNLOCK: u64 = 115;
pub const SYS_SEM_WAIT:     u64 = 116;
pub const SYS_SEM_POST:     u64 = 117;

// ──────────────────────────────────────────────────────────────────────────────
// Tabla de Descriptores de Archivos Abiertos (File Descriptor Table)
// ──────────────────────────────────────────────────────────────────────────────
#[derive(Clone)]
pub struct OpenFile {
    pub path: String,
    pub offset: usize,
    pub flags: u32,
}

pub static OPEN_FILES: SpinMutex<[Option<OpenFile>; 32]> = SpinMutex::new([
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
]);

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
        //    bits 63:48 = base de selectores User: 0x0010
        //      sysretq carga SS = (STAR[63:48] + 8) | 3  = 0x18 | 3 = 0x1B (User Data)
        //      sysretq carga CS = (STAR[63:48] + 16) | 3 = 0x20 | 3 = 0x23 (User Code)
        //    bits 47:32 = selector Kernel Code: 0x0008
        //      syscall carga CS = STAR[47:32]      = 0x08 (Kernel Code)
        //      syscall carga SS = STAR[47:32] + 8  = 0x10 (Kernel Data)
        let star: u64 = (0x0010_u64 << 48) | (0x0008_u64 << 32);
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

    /* 3. Reacomodar registros a la ABI de llamada en C (System V AMD64):
          Rust ABI espera:
            RDI = arg0 (nr)
            RSI = arg1 (a0)
            RDX = arg2 (a1)
            RCX = arg3 (a2)
          En llamada syscall de usuario:
            RAX = syscall number
            RDI = a0
            RSI = a1
            RDX = a2
    */
    mov rcx, rdx    /* a2 */
    mov rdx, rsi    /* a1 */
    mov rsi, rdi    /* a0 */
    mov rdi, rax    /* syscall nr */
    call syscall_dispatch

    /* 4. Restaurar contexto de usuario (RAX contiene el retorno) */
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    pop r10         /* RSP usuario */
    pop r11         /* RFLAGS */
    pop rcx         /* RIP retorno */

    /* 5. Restaurar RSP usuario y volver a Ring 3 con sysretq */
    mov rsp, r10
    sysretq

.global enter_ring3_with_return
enter_ring3_with_return:
    /* rdi = entry (u64), rsi = user_stack_top (u64) */
    /* Preservar registros callee-saved según System V ABI */
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15

    /* Guardar RSP del supervisor para que SYS_EXIT o Anti-BSOD pueda retornar */
    lea rax, [rip + SUPERVISOR_RSP]
    mov [rax], rsp

    /* Construir marco de IRETQ hacia Ring 3 */
    mov rax, 0x1b          /* SELECTOR_USER_DATA (0x18 | 3) */
    push rax               /* SS */
    push rsi               /* RSP (user_stack_top) */
    push 0x202             /* RFLAGS (IF=1) */
    mov rax, 0x23          /* SELECTOR_USER_CODE (0x20 | 3) */
    push rax               /* CS */
    push rdi               /* RIP (entry) */

    iretq

.global return_to_supervisor
return_to_supervisor:
    /* rdi = exit code */
    /* Restaurar RSP del supervisor */
    lea rdx, [rip + SUPERVISOR_RSP]
    mov rsp, [rdx]
    mov qword ptr [rdx], 0

    /* Restaurar registros callee-saved */
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp

    /* Retornar código de salida en RAX */
    mov rax, rdi
    ret
"#);

#[no_mangle]
pub static mut SUPERVISOR_RSP: u64 = 0;

extern "C" {
    fn syscall_handler_asm();
    pub fn enter_ring3_with_return(entry: u64, user_stack_top: u64) -> u64;
    pub fn return_to_supervisor(code: u64) -> !;
}

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

/// Valida si un puntero y rango de longitud pertenecen a memoria de usuario válida
fn is_valid_user_ptr(ptr: u64, len: usize) -> bool {
    if len == 0 {
        return true;
    }
    match ptr.checked_add(len as u64) {
        Some(end) => ptr > 0 && end <= 0x0000_7FFF_FFFF_FFFF,
        None => false,
    }
}

/// Lee una cadena terminada en nulo desde el espacio de usuario (hasta max_len bytes)
fn read_user_string(ptr: u64, max_len: usize) -> Option<String> {
    if !is_valid_user_ptr(ptr, 1) {
        return None;
    }
    let mut bytes = Vec::new();
    let raw = ptr as *const u8;
    for i in 0..max_len {
        let b = unsafe { core::ptr::read(raw.add(i)) };
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).ok()
}

// ──────────────────────────────────────────────────────────────────────────────
// Despachador Rust de syscalls (Ring 0)
// ──────────────────────────────────────────────────────────────────────────────
#[no_mangle]
pub extern "C" fn syscall_dispatch(nr: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    match nr {
        // SYS_READ: fd=arg0, buf=arg1, count=arg2
        SYS_READ => {
            let fd = arg0;
            let buf_ptr = arg1;
            let count = arg2 as usize;

            if !is_valid_user_ptr(buf_ptr, count) {
                return (-14_isize) as u64; // -EFAULT
            }

            if count == 0 {
                return 0;
            }

            // Lectura de teclado interactivo (stdin) - Bloqueante estándar POSIX
            if fd == 0 {
                let user_buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, count) };
                let mut read_bytes = 0;

                // Habilitar interrupciones para permitir que el controlador de teclado PS/2 (IRQ 1),
                // el puerto serie COM1 y el temporizador alimenten la cola de entrada.
                unsafe {
                    asm!("sti");
                }

                loop {
                    if let Some(ch) = crate::interrupts::pop_input_char() {
                        if ch == '\n' || ch == '\r' {
                            kprint!("\n");
                            if read_bytes < count {
                                user_buf[read_bytes] = b'\n';
                                read_bytes += 1;
                            }
                            break;
                        } else if ch == '\x08' {
                            // Tecla Backspace (borrar carácter previo)
                            if read_bytes > 0 {
                                read_bytes -= 1;
                                kprint!("\x08 \x08");
                            }
                        } else if ch == '\x03' {
                            // Ctrl+C: Cancelar entrada
                            kprint!("^C\n");
                            read_bytes = 0;
                            break;
                        } else if (ch as u8) >= 0x20 && (ch as u8) <= 0x7E {
                            // Carácter imprimible estándar
                            if read_bytes + 1 < count {
                                user_buf[read_bytes] = ch as u8;
                                read_bytes += 1;
                                kprint!("{}", ch);
                            }
                        }
                    } else {
                        // Esperar a la siguiente interrupción de hardware de forma segura y eficiente
                        unsafe {
                            asm!("hlt");
                        }
                    }
                }

                // Deshabilitar interrupciones antes de retornar al stub de syscall
                unsafe {
                    asm!("cli");
                }

                return read_bytes as u64;
            }

            // Lectura de archivo abierto en VFS (fd >= 3)
            if fd >= 3 && (fd as usize - 3) < 32 {
                let slot = fd as usize - 3;
                let mut table = OPEN_FILES.lock();
                if let Some(ref mut file) = table[slot] {
                    let vfs = VFS.lock();
                    if let Ok(file_bytes) = vfs.read_file_bytes(&file.path, 0, 0) {
                        if file.offset >= file_bytes.len() {
                            return 0; // EOF
                        }
                        let available = file_bytes.len() - file.offset;
                        let to_copy = count.min(available);
                        let slice = &file_bytes[file.offset..file.offset + to_copy];
                        unsafe {
                            core::ptr::copy_nonoverlapping(slice.as_ptr(), buf_ptr as *mut u8, to_copy);
                        }
                        file.offset += to_copy;
                        return to_copy as u64;
                    }
                }
            }

            (-9_isize) as u64 // -EBADF
        }

        // SYS_WRITE: fd=arg0, buf=arg1, count=arg2
        SYS_WRITE => {
            let fd = arg0;
            let buf_ptr = arg1;
            let count = arg2 as usize;

            if !is_valid_user_ptr(buf_ptr, count) {
                return (-14_isize) as u64; // -EFAULT
            }

            if count == 0 {
                return 0;
            }

            // Escritura en pantalla / consola estándar (stdout=1, stderr=2)
            if fd == 1 || fd == 2 {
                let to_write = count.min(4096);
                let slice = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, to_write) };
                if let Ok(s) = core::str::from_utf8(slice) {
                    if fd == 2 {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "{}", s);
                    } else {
                        kprint!("{}", s);
                    }
                } else {
                    for &b in slice {
                        if b.is_ascii() {
                            kprint!("{}", b as char);
                        } else {
                            kprint!(".");
                        }
                    }
                }
                return to_write as u64;
            }

            // Escritura en archivo abierto en VFS (fd >= 3)
            if fd >= 3 && (fd as usize - 3) < 32 {
                let slot = fd as usize - 3;
                let path = {
                    let table = OPEN_FILES.lock();
                    table[slot].as_ref().map(|f| f.path.clone())
                };

                if let Some(file_path) = path {
                    let to_write = count.min(8192);
                    let slice = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, to_write) };
                    let mut vfs = VFS.lock();
                    if vfs.create_file(&file_path, slice, 0, 0).is_ok() {
                        let mut table = OPEN_FILES.lock();
                        if let Some(ref mut f) = table[slot] {
                            f.offset += to_write;
                        }
                        return to_write as u64;
                    }
                }
            }

            (-9_isize) as u64 // -EBADF
        }

        // SYS_OPEN: path=arg0, flags=arg1
        SYS_OPEN => {
            let path_opt = read_user_string(arg0, 256);
            if path_opt.is_none() {
                return (-14_isize) as u64; // -EFAULT
            }
            let path = path_opt.unwrap();
            let flags = arg1 as u32;

            let exists = VFS.lock().exists(&path);
            if !exists && (flags & 64) == 0 {
                return (-2_isize) as u64; // -ENOENT
            }

            let mut table = OPEN_FILES.lock();
            for (i, slot) in table.iter_mut().enumerate() {
                if slot.is_none() {
                    *slot = Some(OpenFile {
                        path,
                        offset: 0,
                        flags,
                    });
                    return (i as u64) + 3;
                }
            }

            (-24_isize) as u64 // -EMFILE (Demasiados archivos abiertos)
        }

        // SYS_CLOSE: fd=arg0
        SYS_CLOSE => {
            let fd = arg0;
            if fd < 3 {
                return 0; // stdin, stdout, stderr siempre permanecen disponibles
            }
            if fd >= 3 && (fd as usize - 3) < 32 {
                let slot = fd as usize - 3;
                let mut table = OPEN_FILES.lock();
                table[slot] = None;
                return 0;
            }
            (-9_isize) as u64 // -EBADF
        }

        // SYS_LSEEK: fd=arg0, offset=arg1, whence=arg2
        SYS_LSEEK => {
            let fd = arg0;
            let offset = arg1 as usize;
            let whence = arg2;

            if fd >= 3 && (fd as usize - 3) < 32 {
                let slot = fd as usize - 3;
                let mut table = OPEN_FILES.lock();
                if let Some(ref mut f) = table[slot] {
                    match whence {
                        0 => f.offset = offset, // SEEK_SET
                        1 => f.offset = f.offset.saturating_add(offset), // SEEK_CUR
                        _ => {}
                    }
                    return f.offset as u64;
                }
            }
            (-9_isize) as u64
        }

        // SYS_YIELD: voluntariamente ceder tiempo de CPU
        SYS_YIELD => {
            crate::task::scheduler::yield_now();
            0
        }

        // SYS_GETPID (39)
        SYS_GETPID => {
            let tid = crate::task::scheduler::SCHEDULER.lock().current_task_id();
            tid
        }

        // SYS_EXECVE: path=arg0
        SYS_EXECVE => {
            let path_opt = read_user_string(arg0, 256);
            if let Some(path) = path_opt {
                let resolved_path = if !path.starts_with('/') {
                    alloc::format!("/disk/bin/{}.elf", path)
                } else {
                    path
                };
                match crate::task::pkg::execute_elf(&resolved_path) {
                    Ok(_) => 0,
                    Err(_) => (-2_isize) as u64, // -ENOENT
                }
            } else {
                (-14_isize) as u64 // -EFAULT
            }
        }

        // SYS_EXIT (60)
        SYS_EXIT => {
            kprint_color!(
                VgaColor::LightCyan, VgaColor::Black,
                "  [SYSCALL] Proceso Ring 3 finalizado exitosamente (SYS_EXIT, codigo={})\n",
                arg0
            );
            if unsafe { SUPERVISOR_RSP != 0 } {
                crate::memory::paging::switch_to_kernel_cr3();
                unsafe {
                    return_to_supervisor(arg0);
                }
            } else {
                crate::task::scheduler::mark_finished_and_yield();
                0
            }
        }

        // SYS_UPTIME: segundos de actividad
        SYS_UPTIME => {
            crate::drivers::timer::get_uptime_seconds()
        }

        // SYS_SLEEP (102)
        SYS_SLEEP => {
            crate::task::scheduler::sleep(arg0);
            0
        }

        // Primitivas IPC y sincronización
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
                Ok(msg_id) => msg_id,
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
                Ok(msg) => msg.id,
                Err(_) => u64::MAX,
            }
        }

        SYS_FUTEX_WAIT => {
            let user_ptr = arg0 as *const u32;
            let expected = arg1 as u32;
            if is_valid_user_ptr(arg0, 4) {
                let current = unsafe { *user_ptr };
                if current == expected {
                    crate::task::scheduler::sleep(1);
                }
            }
            0
        }

        SYS_FUTEX_WAKE => {
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
