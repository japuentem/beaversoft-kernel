// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Physical Page Frame Allocator (4 KiB Granularity)
// ============================================================================

use core::sync::atomic::{AtomicUsize, Ordering};

pub const PAGE_SIZE: usize = 4096;

pub struct FrameAllocator {
    next_frame: AtomicUsize,
    memory_end: usize,
    allocated_frames: AtomicUsize,
}

impl FrameAllocator {
    pub const fn new() -> Self {
        Self {
            next_frame: AtomicUsize::new(0),
            memory_end: 0,
            allocated_frames: AtomicUsize::new(0),
        }
    }

    pub fn init(&mut self, start_addr: usize, end_addr: usize) {
        // Alinear al siguiente múltiplo de 4096 (4 KiB)
        let aligned_start = (start_addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        self.next_frame.store(aligned_start, Ordering::SeqCst);
        self.memory_end = end_addr & !(PAGE_SIZE - 1);
        self.allocated_frames.store(0, Ordering::SeqCst);
    }

    pub fn allocate_frame(&self) -> Option<usize> {
        loop {
            let current = self.next_frame.load(Ordering::SeqCst);
            if current + PAGE_SIZE > self.memory_end {
                return None; // Fuera de memoria física
            }
            let next = current + PAGE_SIZE;
            if self.next_frame.compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                self.allocated_frames.fetch_add(1, Ordering::Relaxed);
                // Limpiar la página asignada a ceros para seguridad
                unsafe {
                    core::ptr::write_bytes(current as *mut u8, 0, PAGE_SIZE);
                }
                return Some(current);
            }
        }
    }

    pub fn allocated_frames_count(&self) -> usize {
        self.allocated_frames.load(Ordering::Relaxed)
    }

    pub fn total_frames_count(&self) -> usize {
        let current = self.next_frame.load(Ordering::Relaxed);
        if self.memory_end > current {
            (self.memory_end - current) / PAGE_SIZE + self.allocated_frames_count()
        } else {
            self.allocated_frames_count()
        }
    }
}

pub static FRAME_ALLOCATOR: FrameAllocator = FrameAllocator::new();
