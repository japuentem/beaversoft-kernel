// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Native Rust userland bindings for beaver-libc
// ============================================================================

#![no_std]

use core::arch::asm;

pub const SYS_READ:     u64 = 0;
pub const SYS_WRITE:    u64 = 1;
pub const SYS_OPEN:     u64 = 2;
pub const SYS_CLOSE:    u64 = 3;
pub const SYS_STAT:     u64 = 4;
pub const SYS_LSEEK:    u64 = 8;
pub const SYS_BRK:      u64 = 12;
pub const SYS_YIELD:    u64 = 24;
pub const SYS_GETPID:   u64 = 39;
pub const SYS_EXECVE:   u64 = 59;
pub const SYS_EXIT:     u64 = 60;
pub const SYS_WAITPID:  u64 = 61;
pub const SYS_GETTIMEOFDAY:  u64 = 96;
pub const SYS_UPTIME:         u64 = 99;
pub const SYS_SLEEP:          u64 = 102;
pub const SYS_BEEP:           u64 = 103;
pub const SYS_TIME:           u64 = 201;
pub const SYS_CLOCK_GETTIME:  u64 = 228;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TimeVal {
    pub tv_sec: u64,
    pub tv_usec: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TimeZone {
    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TimeSpec {
    pub tv_sec: u64,
    pub tv_nsec: u64,
}

pub const CLOCK_REALTIME: u64 = 0;
pub const CLOCK_MONOTONIC: u64 = 1;

// Protocolo de Ventanas de Usuario vía IPC (Desktop Compositor IPC)
pub const SYS_GUI_CREATE_WINDOW:   u64 = 120;
pub const SYS_GUI_DESTROY_WINDOW:  u64 = 121;
pub const SYS_GUI_DRAW_RECT:       u64 = 122;
pub const SYS_GUI_DRAW_TEXT:       u64 = 123;
pub const SYS_GUI_BLIT:            u64 = 124;
pub const SYS_GUI_GET_EVENT:       u64 = 125;
pub const SYS_GUI_INVALIDATE:      u64 = 126;

// Llamadas al sistema de Audio PCM y Hardware DMA
pub const SYS_AUDIO_PLAY:          u64 = 130;
pub const SYS_AUDIO_STATUS:        u64 = 131;
pub const SYS_AUDIO_STOP:          u64 = 132;

// Sockets TCP/IP
pub const SYS_SOCKET:              u64 = 41;
pub const SYS_CONNECT:             u64 = 42;
pub const SYS_ACCEPT:              u64 = 43;
pub const SYS_SEND:                u64 = 44;
pub const SYS_RECV:                u64 = 45;
pub const SYS_BIND:                u64 = 49;
pub const SYS_LISTEN:              u64 = 50;

pub const AF_INET: i32     = 2;
pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32  = 2;

// Señales asíncronas POSIX
pub const SYS_KILL:                u64 = 62;
pub const SYS_SIGACTION:           u64 = 13;
pub const SYS_SIGPROCMASK:         u64 = 14;
pub const SYS_SIGRETURN:           u64 = 15;

// Criptografía y entropía
pub const SYS_GETRANDOM:           u64 = 318;

pub const SIGHUP:  i32 = 1;
pub const SIGINT:  i32 = 2;
pub const SIGQUIT: i32 = 3;
pub const SIGILL:  i32 = 4;
pub const SIGTRAP: i32 = 5;
pub const SIGABRT: i32 = 6;
pub const SIGFPE:  i32 = 8;
pub const SIGKILL: i32 = 9;
pub const SIGUSR1: i32 = 10;
pub const SIGSEGV: i32 = 11;
pub const SIGUSR2: i32 = 12;
pub const SIGPIPE: i32 = 13;
pub const SIGALRM: i32 = 14;
pub const SIGTERM: i32 = 15;
pub const SIGCHLD: i32 = 17;
pub const SIGCONT: i32 = 18;
pub const SIGSTOP: i32 = 19;

pub const SIG_DFL: u64 = 0;
pub const SIG_IGN: u64 = 1;

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

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserAudioStatus {
    pub controller_type: u32,
    pub pci_audio_found: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub total_pcm_bytes: u64,
    pub dma_buffer_addr: u64,
}

pub const GUI_EVENT_CLICK: u32 = 1;
pub const GUI_EVENT_KEY: u32 = 2;
pub const GUI_EVENT_CLOSE: u32 = 3;
pub const GUI_EVENT_FOCUS: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiEvent {
    pub event_type: u32,
    pub param1: u32,
    pub param2: u32,
    pub param3: u32,
}

#[inline(always)]
pub unsafe fn syscall0(nr: u64) -> u64 {
    let ret: u64;
    #[cfg(target_arch = "x86_64")]
    asm!("syscall", inout("rax") nr => ret, out("rcx") _, out("r11") _, options(nostack));
    #[cfg(target_arch = "aarch64")]
    asm!("svc #0", in("x8") nr, lateout("x0") ret, options(nostack));
    ret
}

#[inline(always)]
pub unsafe fn syscall1(nr: u64, a0: u64) -> u64 {
    let ret: u64;
    #[cfg(target_arch = "x86_64")]
    asm!("syscall", inout("rax") nr => ret, in("rdi") a0, out("rcx") _, out("r11") _, options(nostack));
    #[cfg(target_arch = "aarch64")]
    asm!("svc #0", in("x8") nr, inout("x0") a0 => ret, options(nostack));
    ret
}

#[inline(always)]
pub unsafe fn syscall2(nr: u64, a0: u64, a1: u64) -> u64 {
    let ret: u64;
    #[cfg(target_arch = "x86_64")]
    asm!("syscall", inout("rax") nr => ret, in("rdi") a0, in("rsi") a1, out("rcx") _, out("r11") _, options(nostack));
    #[cfg(target_arch = "aarch64")]
    asm!("svc #0", in("x8") nr, inout("x0") a0 => ret, in("x1") a1, options(nostack));
    ret
}

#[inline(always)]
pub unsafe fn syscall3(nr: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    let ret: u64;
    #[cfg(target_arch = "x86_64")]
    asm!("syscall", inout("rax") nr => ret, in("rdi") a0, in("rsi") a1, in("rdx") a2, out("rcx") _, out("r11") _, options(nostack));
    #[cfg(target_arch = "aarch64")]
    asm!("svc #0", in("x8") nr, inout("x0") a0 => ret, in("x1") a1, in("x2") a2, options(nostack));
    ret
}

#[inline(always)]
pub unsafe fn syscall4(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    let ret: u64;
    #[cfg(target_arch = "x86_64")]
    asm!("syscall", inout("rax") nr => ret, in("rdi") a0, in("rsi") a1, in("rdx") a2, in("r10") a3, out("rcx") _, out("r11") _, options(nostack));
    #[cfg(target_arch = "aarch64")]
    asm!("svc #0", in("x8") nr, inout("x0") a0 => ret, in("x1") a1, in("x2") a2, in("x3") a3, options(nostack));
    ret
}

pub fn read(fd: u64, buf: &mut [u8]) -> Result<usize, isize> {
    let res = unsafe { syscall3(SYS_READ, fd, buf.as_mut_ptr() as u64, buf.len() as u64) } as isize;
    if res < 0 {
        Err(res)
    } else {
        Ok(res as usize)
    }
}

pub fn write(fd: u64, buf: &[u8]) -> Result<usize, isize> {
    let res = unsafe { syscall3(SYS_WRITE, fd, buf.as_ptr() as u64, buf.len() as u64) } as isize;
    if res < 0 {
        Err(res)
    } else {
        Ok(res as usize)
    }
}

pub fn open(path: &str, flags: u32) -> Result<u64, isize> {
    let res = unsafe { syscall2(SYS_OPEN, path.as_ptr() as u64, flags as u64) } as isize;
    if res < 0 {
        Err(res)
    } else {
        Ok(res as u64)
    }
}

pub fn close(fd: u64) -> Result<(), isize> {
    let res = unsafe { syscall1(SYS_CLOSE, fd) } as isize;
    if res < 0 {
        Err(res)
    } else {
        Ok(())
    }
}

pub fn getpid() -> u64 {
    unsafe { syscall0(SYS_GETPID) }
}

pub fn uptime() -> u64 {
    unsafe { syscall0(SYS_UPTIME) }
}

pub fn sleep(ticks: u64) {
    unsafe { syscall1(SYS_SLEEP, ticks); }
}

pub fn beep(hz: u32, duration_ms: u32) {
    unsafe { syscall2(SYS_BEEP, hz as u64, duration_ms as u64); }
}

pub fn exit(code: u64) -> ! {
    unsafe { syscall1(SYS_EXIT, code); }
    loop {}
}

pub fn waitpid(pid: u64) -> Result<u64, isize> {
    let res = unsafe { syscall3(SYS_WAITPID, pid, 0, 0) } as isize;
    if res < 0 {
        Err(res)
    } else {
        Ok(res as u64)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Gestión de Heap en Espacio de Usuario (brk / sbrk / malloc / free)
// ──────────────────────────────────────────────────────────────────────────────
pub fn brk(addr: usize) -> usize {
    unsafe { syscall1(SYS_BRK, addr as u64) as usize }
}

pub fn sbrk(increment: isize) -> *mut u8 {
    let cur = brk(0);
    if increment == 0 {
        return cur as *mut u8;
    }
    let new_brk = if increment > 0 {
        cur.checked_add(increment as usize).unwrap_or(cur)
    } else {
        cur.saturating_sub((-increment) as usize)
    };
    let res = brk(new_brk);
    if res == cur && increment != 0 {
        core::ptr::null_mut()
    } else {
        cur as *mut u8
    }
}

#[repr(C, align(16))]
struct BlockHeader {
    size: usize,
    is_free: usize,
    next: *mut BlockHeader,
}

static mut FREE_LIST_HEAD: *mut BlockHeader = core::ptr::null_mut();

pub fn malloc(size: usize) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }
    let aligned_size = (size + 15) & !15;
    let header_size = core::mem::size_of::<BlockHeader>();

    unsafe {
        // 1. Buscar en lista de bloques reciclados
        let mut curr = FREE_LIST_HEAD;
        while !curr.is_null() {
            if (*curr).is_free == 1 && (*curr).size >= aligned_size {
                (*curr).is_free = 0;
                return (curr as *mut u8).add(header_size);
            }
            curr = (*curr).next;
        }

        // 2. Expandir el heap mediante sbrk
        let total = header_size + aligned_size;
        let mem = sbrk(total as isize);
        if mem.is_null() {
            return core::ptr::null_mut();
        }

        let new_block = mem as *mut BlockHeader;
        (*new_block).size = aligned_size;
        (*new_block).is_free = 0;
        (*new_block).next = FREE_LIST_HEAD;
        FREE_LIST_HEAD = new_block;

        (mem as *mut u8).add(header_size)
    }
}

pub fn free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let header_size = core::mem::size_of::<BlockHeader>();
        let header = ptr.sub(header_size) as *mut BlockHeader;
        (*header).is_free = 1;
    }
}

pub struct BeaverUserAllocator;

unsafe impl core::alloc::GlobalAlloc for BeaverUserAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        malloc(layout.size())
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        free(ptr);
    }
}

pub fn gui_create_window(title: &str, x: usize, y: usize, width: usize, height: usize) -> Result<u32, isize> {
    let dims: u64 = ((width as u64) << 16) | (height as u64);
    let coords: u64 = ((x as u64) << 16) | (y as u64);
    let res = unsafe { syscall4(SYS_GUI_CREATE_WINDOW, title.as_ptr() as u64, title.len() as u64, dims, coords) };
    if res == u64::MAX {
        Err(-1)
    } else {
        Ok(res as u32)
    }
}

pub fn gui_destroy_window(win_id: u32) -> Result<(), isize> {
    let res = unsafe { syscall1(SYS_GUI_DESTROY_WINDOW, win_id as u64) };
    if res == u64::MAX {
        Err(-1)
    } else {
        Ok(())
    }
}

pub fn gui_draw_rect(win_id: u32, rx: usize, ry: usize, rw: usize, rh: usize, color: u32) -> Result<(), isize> {
    let coords: u64 = ((rx as u64) << 16) | (ry as u64);
    let dims: u64 = ((rw as u64) << 16) | (rh as u64);
    let res = unsafe { syscall4(SYS_GUI_DRAW_RECT, win_id as u64, coords, dims, color as u64) };
    if res == u64::MAX {
        Err(-1)
    } else {
        Ok(())
    }
}

pub fn gui_get_event(win_id: u32) -> Option<GuiEvent> {
    let mut event = GuiEvent { event_type: 0, param1: 0, param2: 0, param3: 0 };
    let res = unsafe { syscall2(SYS_GUI_GET_EVENT, win_id as u64, &mut event as *mut _ as u64) };
    if res == 1 {
        Some(event)
    } else {
        None
    }
}

pub fn gui_invalidate(win_id: u32) {
    unsafe { syscall1(SYS_GUI_INVALIDATE, win_id as u64) };
}

pub fn gettimeofday(tv: &mut TimeVal, tz: Option<&mut TimeZone>) -> i32 {
    let tz_ptr = match tz {
        Some(t) => t as *mut _ as u64,
        None => 0,
    };
    let res = unsafe { syscall2(SYS_GETTIMEOFDAY, tv as *mut _ as u64, tz_ptr) };
    res as i32
}

pub fn clock_gettime(clk_id: u64, ts: &mut TimeSpec) -> i32 {
    let res = unsafe { syscall2(SYS_CLOCK_GETTIME, clk_id, ts as *mut _ as u64) };
    res as i32
}

pub fn time(tloc: Option<&mut u64>) -> u64 {
    let tloc_ptr = match tloc {
        Some(p) => p as *mut _ as u64,
        None => 0,
    };
    unsafe { syscall1(SYS_TIME, tloc_ptr) }
}

pub fn audio_play(pcm_data: &[u8], sample_rate: u32, channels: u16, bits_per_sample: u16) -> Result<usize, isize> {
    let fmt_packed: u64 = ((bits_per_sample as u64) << 16) | (channels as u64);
    let res = unsafe { syscall4(SYS_AUDIO_PLAY, pcm_data.as_ptr() as u64, pcm_data.len() as u64, sample_rate as u64, fmt_packed) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(res as usize)
    }
}

pub fn audio_play_wav(wav_bytes: &[u8]) -> Result<usize, isize> {
    let res = unsafe { syscall4(SYS_AUDIO_PLAY, wav_bytes.as_ptr() as u64, wav_bytes.len() as u64, 0, 0) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(res as usize)
    }
}

pub fn audio_status(status: &mut UserAudioStatus) -> Result<(), isize> {
    let res = unsafe { syscall1(SYS_AUDIO_STATUS, status as *mut _ as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(())
    }
}

pub fn audio_stop() {
    unsafe { syscall0(SYS_AUDIO_STOP) };
}

pub fn kill(pid: i32, sig: i32) -> Result<(), isize> {
    let res = unsafe { syscall2(SYS_KILL, pid as u64, sig as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(())
    }
}

pub fn sigaction(sig: i32, act: Option<&SigAction>, oldact: Option<&mut SigAction>) -> Result<(), isize> {
    let act_ptr = match act {
        Some(a) => a as *const _ as u64,
        None => 0,
    };
    let oldact_ptr = match oldact {
        Some(oa) => oa as *mut _ as u64,
        None => 0,
    };
    let res = unsafe { syscall3(SYS_SIGACTION, sig as u64, act_ptr, oldact_ptr) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(())
    }
}

pub fn sigprocmask(how: i32, set: Option<&u64>, oldset: Option<&mut u64>) -> Result<(), isize> {
    let set_ptr = match set {
        Some(s) => s as *const _ as u64,
        None => 0,
    };
    let oldset_ptr = match oldset {
        Some(os) => os as *mut _ as u64,
        None => 0,
    };
    let res = unsafe { syscall3(SYS_SIGPROCMASK, how as u64, set_ptr, oldset_ptr) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(())
    }
}

pub fn getrandom(buf: &mut [u8], flags: u32) -> Result<usize, isize> {
    let res = unsafe { syscall3(SYS_GETRANDOM, buf.as_mut_ptr() as u64, buf.len() as u64, flags as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(res as usize)
    }
}

pub fn socket(domain: i32, sock_type: i32, protocol: i32) -> Result<i32, isize> {
    let res = unsafe { syscall3(SYS_SOCKET, domain as u64, sock_type as u64, protocol as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(res as i32)
    }
}

pub fn connect(fd: i32, remote_ip: u32, remote_port: u16) -> Result<(), isize> {
    let res = unsafe { syscall3(SYS_CONNECT, fd as u64, remote_ip as u64, remote_port as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(())
    }
}

pub fn send(fd: i32, buf: &[u8], flags: i32) -> Result<usize, isize> {
    let res = unsafe { syscall4(SYS_SEND, fd as u64, buf.as_ptr() as u64, buf.len() as u64, flags as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(res as usize)
    }
}

pub fn recv(fd: i32, buf: &mut [u8], flags: i32) -> Result<usize, isize> {
    let res = unsafe { syscall4(SYS_RECV, fd as u64, buf.as_mut_ptr() as u64, buf.len() as u64, flags as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(res as usize)
    }
}

pub fn bind(fd: i32, port: u16) -> Result<(), isize> {
    let res = unsafe { syscall2(SYS_BIND, fd as u64, port as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(())
    }
}

pub fn listen(fd: i32, backlog: i32) -> Result<(), isize> {
    let res = unsafe { syscall2(SYS_LISTEN, fd as u64, backlog as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(())
    }
}

pub fn accept(fd: i32) -> Result<i32, isize> {
    let res = unsafe { syscall1(SYS_ACCEPT, fd as u64) };
    let code = res as isize;
    if code < 0 {
        Err(code)
    } else {
        Ok(res as i32)
    }
}

/// Construye un registro ClientHello estándar de TLS 1.3 (RFC 8446) en un búfer preasignado de usuario
pub fn tls13_build_client_hello(hostname: &str, out: &mut [u8]) -> Result<usize, isize> {
    let host_bytes = hostname.as_bytes();
    let sni_ext_len = host_bytes.len() + 5;
    let supp_versions_len = 5;
    let extensions_len = 4 + sni_ext_len + supp_versions_len;
    let body_len = 2 + 32 + 1 + 32 + 2 + 4 + 1 + 1 + 2 + extensions_len;
    let handshake_len = 4 + body_len;
    let total_record_len = 5 + handshake_len;

    if out.len() < total_record_len {
        return Err(-28); // -ENOSPC
    }

    // Cabecera de Registro TLS (Tipo 22 = Handshake, versión 0x0301 TLS 1.0 heredada, longitud)
    out[0] = 22; // TLS_CONTENT_HANDSHAKE
    out[1] = 0x03;
    out[2] = 0x01;
    out[3] = ((handshake_len >> 8) & 0xFF) as u8;
    out[4] = (handshake_len & 0xFF) as u8;

    // Cabecera Handshake (Tipo 1 = ClientHello, longitud de 3 bytes)
    out[5] = 1;
    out[6] = ((body_len >> 16) & 0xFF) as u8;
    out[7] = ((body_len >> 8) & 0xFF) as u8;
    out[8] = (body_len & 0xFF) as u8;

    let mut idx = 9;
    // Legacy Version: TLS 1.2 (0x0303)
    out[idx] = 0x03; out[idx + 1] = 0x03;
    idx += 2;

    // Client Random: 32 bytes de entropía generados con getrandom()
    if getrandom(&mut out[idx..idx + 32], 0).is_err() {
        for b in &mut out[idx..idx + 32] { *b = 0xAA; }
    }
    out[idx] = 0xBE; out[idx + 1] = 0xAF; // Beaver TLS Magic
    idx += 32;

    // Legacy Session ID: 32 bytes
    out[idx] = 32;
    idx += 1;
    for i in 0..32 { out[idx + i] = 0x55; }
    idx += 32;

    // Cipher Suites: TLS_CHACHA20_POLY1305_SHA256 (0x1303) y TLS_AES_128_GCM_SHA256 (0x1301)
    out[idx] = 0x00; out[idx + 1] = 0x04;
    out[idx + 2] = 0x13; out[idx + 3] = 0x03;
    out[idx + 4] = 0x13; out[idx + 5] = 0x01;
    idx += 6;

    // Compression Methods: 1 byte (0x00 = No compresión)
    out[idx] = 1;
    out[idx + 1] = 0x00;
    idx += 2;

    // Extensiones
    out[idx] = ((extensions_len >> 8) & 0xFF) as u8;
    out[idx + 1] = (extensions_len & 0xFF) as u8;
    idx += 2;

    // Extensión SNI (0x0000)
    out[idx] = 0x00; out[idx + 1] = 0x00;
    out[idx + 2] = ((sni_ext_len >> 8) & 0xFF) as u8;
    out[idx + 3] = (sni_ext_len & 0xFF) as u8;
    idx += 4;
    let list_len = host_bytes.len() + 3;
    out[idx] = ((list_len >> 8) & 0xFF) as u8;
    out[idx + 1] = (list_len & 0xFF) as u8;
    out[idx + 2] = 0x00;
    out[idx + 3] = ((host_bytes.len() >> 8) & 0xFF) as u8;
    out[idx + 4] = (host_bytes.len() & 0xFF) as u8;
    idx += 5;
    out[idx..idx + host_bytes.len()].copy_from_slice(host_bytes);
    idx += host_bytes.len();

    // Extensión Supported Versions (0x002B) para negociar TLS 1.3 (0x0304)
    out[idx] = 0x00; out[idx + 1] = 0x2B;
    out[idx + 2] = 0x00; out[idx + 3] = 0x03;
    out[idx + 4] = 0x02;
    out[idx + 5] = 0x03; out[idx + 6] = 0x04;
    idx += 7;

    Ok(idx)
}

