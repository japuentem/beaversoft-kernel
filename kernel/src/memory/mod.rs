// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Memory Management Subsystem (Physical Frames & Dynamic Heap)
//              Fase 2: integrado con mapa de memoria real (MB1/UEFI),
//              bitmap allocator, sin f32, sin cast *const→*mut.
// ============================================================================

pub mod frame_allocator;
pub mod heap;
pub mod memmap;

pub use memmap::MemMap;

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
pub const AUDIO_DMA_BUFFER_ADDR: usize = 0x0100_0000; // 16 MiB
pub const BACKBUFFER_PHYS_ADDR: usize  = 0x0200_0000; // 32 MiB

pub fn kernel_start() -> usize {
    unsafe { &_kernel_start as *const u8 as usize }
}

pub fn kernel_end() -> usize {
    unsafe { &_kernel_end as *const u8 as usize }
}

/// Inicializa el subsistema de memoria con el mapa real entregado por el cargador.
/// Debe llamarse con el `MemMap` parseado antes de cualquier asignacion de marcos.
pub fn init_memory(memmap: &MemMap) {
    let k_start = kernel_start();
    let k_end   = kernel_end();
    let k_size  = k_end - k_start;

    // 1. Inicializar el Heap del kernel (4 MiB en direccion fija)
    unsafe {
        HEAP_ALLOCATOR.init(HEAP_START, HEAP_SIZE);
    }

    // 2. Inicializar el bitmap allocator con el mapa de memoria real.
    //    Internamente marca todo como ocupado y libera solo las regiones usables,
    //    luego re-reserva las regiones criticas del kernel.
    //    NO hay cast *const → *mut, NO hay UB.
    FRAME_ALLOCATOR.init_from_memmap(memmap);

    // 3. Imprimir estado de memoria (sin f32, solo KiB y MiB enteros)
    let k_size_kib  = k_size / 1024;
    let heap_kib    = HEAP_SIZE / 1024;
    let free_mib    = FRAME_ALLOCATOR.free_frames_count() * 4096 / (1024 * 1024);
    let total_mib   = FRAME_ALLOCATOR.total_usable_frames() * 4096 / (1024 * 1024);

    kprint_color!(
        VgaColor::White, VgaColor::Black,
        "[MEM] Kernel Image: {:#x} - {:#x} ({} KiB)\n",
        k_start, k_end, k_size_kib
    );
    kprint_color!(
        VgaColor::White, VgaColor::Black,
        "[MEM] Kernel Heap:  {:#x} - {:#x} ({} KiB)\n",
        HEAP_START, HEAP_START + HEAP_SIZE, heap_kib
    );
    kprint_color!(
        VgaColor::LightGreen, VgaColor::Black,
        "[MEM] RAM Fisica:   {} MiB libres / {} MiB usables (mapa real)\n",
        free_mib, total_mib
    );
}
