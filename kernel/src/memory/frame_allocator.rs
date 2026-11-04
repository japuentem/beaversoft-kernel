// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Physical Page Frame Allocator — Bitmap de 128 KiB
//              Soporta hasta 4 GiB de RAM (1 048 576 marcos de 4 KiB).
//              Reemplaza el bump-allocator previo que nunca liberaba marcos
//              y contenia comportamiento indefinido (cast *const → *mut static).
// ============================================================================

use core::sync::atomic::{AtomicUsize, Ordering};
use crate::sync::SpinMutex;

pub const PAGE_SIZE: usize = 4096;

/// Numero de marcos de 4 KiB para cubrir 4 GiB.
const TOTAL_FRAMES: usize = 4 * 1024 * 1024 * 1024 / PAGE_SIZE; // 1 048 576

/// Bits por palabra del bitmap.
const BITS: usize = 64;

/// Palabras necesarias para el bitmap.
const BITMAP_WORDS: usize = TOTAL_FRAMES / BITS; // 16 384 palabras = 128 KiB

// ============================================================================
// Estado interno del allocator (protegido por SpinMutex)
// ============================================================================

struct FrameAllocatorInner {
    /// 0 = libre, 1 = ocupado.  Indice: frame = word*64 + bit.
    bitmap: [u64; BITMAP_WORDS],
    /// Numero de marcos actualmente libres (para estadisticas rapidas).
    free_frames: usize,
    /// Numero total de marcos que el mapa de memoria marco como usables.
    total_usable: usize,
    /// Primer marco potencialmente libre (hint para acelerar busqueda).
    search_hint: usize,
}

impl FrameAllocatorInner {
    const fn new() -> Self {
        Self {
            // Todo ocupado al inicio; se libera solo lo que el mapa de memoria marque usable.
            bitmap: [u64::MAX; BITMAP_WORDS],
            free_frames:  0,
            total_usable: 0,
            search_hint:  0,
        }
    }

    /// Marca un marco como libre (bit = 0).
    #[inline]
    fn mark_free(&mut self, frame: usize) {
        if frame >= TOTAL_FRAMES { return; }
        let word = frame / BITS;
        let bit  = frame % BITS;
        let was_set = (self.bitmap[word] >> bit) & 1;
        self.bitmap[word] &= !(1u64 << bit);
        if was_set != 0 {
            self.free_frames = self.free_frames.saturating_add(1);
        }
        if word < self.search_hint / BITS {
            self.search_hint = frame;
        }
    }

    /// Marca un marco como ocupado (bit = 1).
    #[inline]
    fn mark_used(&mut self, frame: usize) {
        if frame >= TOTAL_FRAMES { return; }
        let word = frame / BITS;
        let bit  = frame % BITS;
        let was_clear = ((self.bitmap[word] >> bit) & 1) == 0;
        self.bitmap[word] |= 1u64 << bit;
        if was_clear {
            self.free_frames = self.free_frames.saturating_sub(1);
        }
    }

    /// Libera todos los marcos de la region [start_frame, end_frame).
    fn free_range(&mut self, start_frame: usize, end_frame: usize) {
        for f in start_frame..end_frame.min(TOTAL_FRAMES) {
            self.mark_free(f);
        }
    }

    /// Reserva todos los marcos de la region [start_frame, end_frame).
    fn reserve_range(&mut self, start_frame: usize, end_frame: usize) {
        for f in start_frame..end_frame.min(TOTAL_FRAMES) {
            self.mark_used(f);
        }
    }

    /// Asigna un marco libre. Devuelve la direccion fisica o None si no hay.
    fn allocate(&mut self) -> Option<usize> {
        let start_word = self.search_hint / BITS;
        // Buscar desde el hint hasta el final del bitmap
        for w in start_word..BITMAP_WORDS {
            if self.bitmap[w] == u64::MAX { continue; } // Toda ocupada
            let free_bit = (!self.bitmap[w]).trailing_zeros() as usize;
            let frame = w * BITS + free_bit;
            if frame >= TOTAL_FRAMES { break; }
            self.mark_used(frame);
            self.search_hint = frame + 1;
            // Limpiar la pagina a ceros
            // SAFETY: `frame * PAGE_SIZE` es un marco físico recién reservado en RAM usable con identity mapping.
            unsafe {
                core::ptr::write_bytes((frame * PAGE_SIZE) as *mut u8, 0, PAGE_SIZE);
            }
            return Some(frame * PAGE_SIZE);
        }
        // Buscar desde el principio si no se encontro desde el hint
        for w in 0..start_word {
            if self.bitmap[w] == u64::MAX { continue; }
            let free_bit = (!self.bitmap[w]).trailing_zeros() as usize;
            let frame = w * BITS + free_bit;
            self.mark_used(frame);
            self.search_hint = frame + 1;
            // SAFETY: `frame * PAGE_SIZE` es un marco físico libre recién marcado como ocupado en memoria usable mapeada por identidad.
            unsafe {
                core::ptr::write_bytes((frame * PAGE_SIZE) as *mut u8, 0, PAGE_SIZE);
            }
            return Some(frame * PAGE_SIZE);
        }
        None
    }

    /// Libera un marco previamente asignado.
    fn free(&mut self, addr: usize) {
        let frame = addr / PAGE_SIZE;
        self.mark_free(frame);
        if frame < self.search_hint {
            self.search_hint = frame;
        }
    }

    /// Asigna `n` marcos contiguos. Necesario para anillos DMA del E1000.
    fn allocate_contiguous(&mut self, n: usize) -> Option<usize> {
        if n == 0 { return Some(0); }
        let start_hint = self.search_hint;
        // Busqueda lineal desde el hint: recorremos marcos buscando n bits libres consecutivos
        'outer: for start in start_hint..TOTAL_FRAMES {
            // Verificacion rapida: si la palabra esta llena, saltar
            let w = start / BITS;
            if self.bitmap[w] == u64::MAX {
                // Saltar al siguiente word
                continue;
            }
            // Verificar que los n marcos desde 'start' esten todos libres
            for i in 0..n {
                let frame = start + i;
                if frame >= TOTAL_FRAMES { break 'outer; }
                let fw = frame / BITS;
                let fb = frame % BITS;
                if (self.bitmap[fw] >> fb) & 1 != 0 {
                    // Ocupado: saltar al marco siguiente despues del obstaculo
                    continue 'outer;
                }
            }
            // Encontrado: marcar todos como usados
            for i in 0..n {
                self.mark_used(start + i);
            }
            let addr = start * PAGE_SIZE;
            // Limpiar toda la region
            // SAFETY: Los `n` marcos contiguos desde `addr` acaban de reservarse en RAM usable mapeada por identidad.
            unsafe {
                core::ptr::write_bytes(addr as *mut u8, 0, n * PAGE_SIZE);
            }
            self.search_hint = start + n;
            return Some(addr);
        }
        None
    }
}

// ============================================================================
// API publica — envuelta en SpinMutex
// ============================================================================

pub struct FrameAllocator {
    inner: SpinMutex<FrameAllocatorInner>,
    /// Conteo de marcos asignados desde el inicio (monotonicamente creciente para estadisticas).
    allocated_total: AtomicUsize,
}

impl FrameAllocator {
    pub const fn new() -> Self {
        Self {
            inner: SpinMutex::new(FrameAllocatorInner::new()),
            allocated_total: AtomicUsize::new(0),
        }
    }

    /// Inicializa el allocator con el mapa de memoria real.
    /// Debe llamarse una sola vez, antes de cualquier asignacion.
    pub fn init_from_memmap(&self, memmap: &crate::memory::memmap::MemMap) {
        let mut g = self.inner.lock();

        // El bitmap empieza todo en 1 (ocupado); liberamos solo lo usable del mapa.
        for i in 0..memmap.count {
            let r = &memmap.regions[i];
            if !r.usable || r.len == 0 { continue; }
            let start_frame = (r.start as usize + PAGE_SIZE - 1) / PAGE_SIZE; // alinear arriba
            let end_frame   = (r.start + r.len) as usize / PAGE_SIZE;          // alinear abajo
            if end_frame > start_frame {
                g.free_range(start_frame, end_frame);
                g.total_usable += end_frame - start_frame;
            }
        }

        // Re-marcar como ocupadas las regiones criticas del kernel
        Self::reserve_kernel_regions(&mut g);
    }

    /// Reserva las regiones que el kernel ocupa y que no deben asignarse a nadie.
    fn reserve_kernel_regions(g: &mut FrameAllocatorInner) {
        use crate::memory::{HEAP_START, HEAP_SIZE, AUDIO_DMA_BUFFER_ADDR, BACKBUFFER_PHYS_ADDR};

        // Primer MiB (BIOS, IVT, EBDA, VGA, etc.)
        g.reserve_range(0, 0x100000 / PAGE_SIZE);

        // Imagen del kernel
        let k_start = crate::memory::kernel_start();
        let k_end   = crate::memory::kernel_end();
        let k_start_frame = k_start / PAGE_SIZE;
        let k_end_frame   = (k_end + PAGE_SIZE - 1) / PAGE_SIZE;
        g.reserve_range(k_start_frame, k_end_frame);

        // Heap del kernel
        let heap_start_frame = HEAP_START / PAGE_SIZE;
        let heap_end_frame   = (HEAP_START + HEAP_SIZE + PAGE_SIZE - 1) / PAGE_SIZE;
        g.reserve_range(heap_start_frame, heap_end_frame);

        // Buffer DMA de audio (16 MiB, 1 MiB de tamano aproximado)
        g.reserve_range(AUDIO_DMA_BUFFER_ADDR / PAGE_SIZE,
                         (AUDIO_DMA_BUFFER_ADDR + 1024 * 1024) / PAGE_SIZE);

        // Backbuffer del compositor (32 MiB, 4 MiB de tamano: 1024*768*4 = ~3 MiB)
        g.reserve_range(BACKBUFFER_PHYS_ADDR / PAGE_SIZE,
                         (BACKBUFFER_PHYS_ADDR + 4 * 1024 * 1024) / PAGE_SIZE);

        // UefiBootInfo en 0x9000 y mapa de memoria UEFI en 0x9100
        g.reserve_range(0x9000 / PAGE_SIZE, (0x9000 + PAGE_SIZE) / PAGE_SIZE);
    }

    /// Asigna un marco de 4 KiB. Devuelve la direccion fisica o None.
    pub fn allocate_frame(&self) -> Option<usize> {
        let addr = self.inner.lock().allocate();
        if addr.is_some() {
            self.allocated_total.fetch_add(1, Ordering::Relaxed);
        }
        addr
    }

    /// Libera un marco previamente asignado por `allocate_frame`.
    pub fn free_frame(&self, addr: usize) {
        self.inner.lock().free(addr);
    }

    /// Asigna `n` marcos fisicamente contiguos (necesario para anillos DMA E1000).
    pub fn allocate_contiguous(&self, n: usize) -> Option<usize> {
        let addr = self.inner.lock().allocate_contiguous(n);
        if addr.is_some() {
            self.allocated_total.fetch_add(n, Ordering::Relaxed);
        }
        addr
    }

    /// Marcos libres actualmente.
    pub fn free_frames_count(&self) -> usize {
        self.inner.lock().free_frames
    }

    /// Marcos usables totales segun el mapa de memoria.
    pub fn total_usable_frames(&self) -> usize {
        self.inner.lock().total_usable
    }

    /// Marcos asignados actualmente (total_usable - free_frames).
    pub fn allocated_frames_count(&self) -> usize {
        let g = self.inner.lock();
        g.total_usable.saturating_sub(g.free_frames)
    }
}

pub static FRAME_ALLOCATOR: FrameAllocator = FrameAllocator::new();
