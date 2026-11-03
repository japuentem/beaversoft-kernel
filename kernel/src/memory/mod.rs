// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Memory Management Subsystem (Physical Frames & Dynamic Heap)
// ============================================================================

pub mod frame_allocator;
pub mod heap;

use crate::memory::frame_allocator::FRAME_ALLOCATOR;
use crate::memory::heap::HEAP_ALLOCATOR;
use crate::kprint_color;
use crate::vga::VgaColor;

extern "C" {
    static _kernel_start: u8;
    static _kernel_end: u8;
}

pub const HEAP_START: usize = 0x0040_0000; // 4 MiB (área segura después del kernel)
pub const HEAP_SIZE: usize  = 4 * 1024 * 1024; // 4 MiB de Heap dinámico para el kernel

// ============================================================================
// Regiones Dedicadas de Hardware y Multimedia (DMA & Compositor)
// ============================================================================
pub const AUDIO_DMA_BUFFER_ADDR: usize = 0x0100_0000; // 16 MiB (Audio DMA circular ring buffers HDA / AC97)
pub const BACKBUFFER_PHYS_ADDR: usize  = 0x0200_0000; // 32 MiB (Compositor de Doble Búfer: 1024x768x4 = 3,145,728 bytes)

pub fn init_memory() {
    let k_start = unsafe { &_kernel_start as *const u8 as usize };
    let k_end   = unsafe { &_kernel_end as *const u8 as usize };
    let k_size  = k_end - k_start;

    // 1. Inicializar el Heap del kernel (4 MiB - 8 MiB)
    unsafe {
        HEAP_ALLOCATOR.init(HEAP_START, HEAP_SIZE);
    }

    // 2. Inicializar asignador de marcos físicos a partir de 36 MiB (respetando Heap, Audio DMA y Backbuffer)
    let frame_start = 0x0240_0000; // 36 MiB
    let memory_limit = 64 * 1024 * 1024; // 64 MiB
    unsafe {
        let frame_alloc_ptr = &FRAME_ALLOCATOR as *const _ as *mut frame_allocator::FrameAllocator;
        (*frame_alloc_ptr).init(frame_start, memory_limit);
    }

    kprint_color!(
        VgaColor::White, VgaColor::Black,
        "[MEM] Kernel Image: {:#x} - {:#x} ({:.2} KiB)\n",
        k_start, k_end, k_size as f32 / 1024.0
    );
    kprint_color!(
        VgaColor::White, VgaColor::Black,
        "[MEM] Kernel Heap:  {:#x} - {:#x} ({} KiB)\n",
        HEAP_START, HEAP_START + HEAP_SIZE, HEAP_SIZE / 1024
    );
}

pub fn kernel_start() -> usize {
    unsafe { &_kernel_start as *const u8 as usize }
}

pub fn kernel_end() -> usize {
    unsafe { &_kernel_end as *const u8 as usize }
}
