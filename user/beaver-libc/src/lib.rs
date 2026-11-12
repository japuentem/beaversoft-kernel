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
pub const SYS_UPTIME:   u64 = 99;
pub const SYS_SLEEP:    u64 = 102;
pub const SYS_BEEP:     u64 = 103;

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

