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

