// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Dynamic Kernel Heap Allocator with Coalescing Free-List
// ============================================================================

use core::alloc::{GlobalAlloc, Layout};
use core::mem::{align_of, size_of};
use core::ptr::null_mut;
use crate::sync::SpinMutex;

#[repr(C)]
struct ListNode {
    size: usize,
    next: *mut ListNode,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        Self {
            size,
            next: null_mut(),
        }
    }
}

pub struct LinkedListAllocator {
    head: ListNode,
    total_size: usize,
    allocated_bytes: usize,
    alloc_count: usize,
    dealloc_count: usize,
}

// SAFETY: `LinkedListAllocator` solo se accede a través de `SpinMutex<LinkedListAllocator>` y administra memoria del heap global del kernel.
unsafe impl Send for LinkedListAllocator {}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
            total_size: 0,
            allocated_bytes: 0,
            alloc_count: 0,
            dealloc_count: 0,
        }
    }

    // SAFETY: El llamador debe garantizar que `[heap_start, heap_start + heap_size)` es memoria válida no utilizada y que `init` se llama solo una vez.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.total_size = heap_size;
        self.allocated_bytes = 0;
        self.alloc_count = 0;
        self.dealloc_count = 0;
        self.head.next = null_mut();
        self.add_free_region(heap_start, heap_size);
    }

    // SAFETY: El llamador debe garantizar que `addr` está alineada a `ListNode` y que el bloque `[addr, addr + size)` está libre y es válido.
    pub unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        if size < size_of::<ListNode>() {
            return;
        }

        let node_ptr = addr as *mut ListNode;
        (*node_ptr).size = size;
        (*node_ptr).next = null_mut();

        // Buscar posición ordenada por dirección física
        let mut current = &mut self.head as *mut ListNode;
        while !(*current).next.is_null() && (*current).next < node_ptr {
            current = (*current).next;
        }

        (*node_ptr).next = (*current).next;
        (*current).next = node_ptr;

        // 1. Coalescencia con el siguiente bloque libre
        if !(*node_ptr).next.is_null() {
            let next_ptr = (*node_ptr).next;
            if (node_ptr as usize) + (*node_ptr).size == (next_ptr as usize) {
                (*node_ptr).size += (*next_ptr).size;
                (*node_ptr).next = (*next_ptr).next;
            }
        }

        // 2. Coalescencia con el bloque libre anterior (current)
        if current != (&mut self.head as *mut ListNode) {
            if (current as usize) + (*current).size == (node_ptr as usize) {
                (*current).size += (*node_ptr).size;
                (*current).next = (*node_ptr).next;
            }
        }
    }

    fn find_region(&mut self, size: usize, align: usize) -> Option<(*mut ListNode, usize)> {
        // SAFETY: Se recorre la lista enlazada de nodos libres cuya validez e invariantes están protegidas por el cerrojo del asignador.
        unsafe {
            let mut current = &mut self.head as *mut ListNode;
            while !(*current).next.is_null() {
                let region = (*current).next;
                let alloc_start = align_up(region as usize, align);
                let alloc_end = alloc_start.checked_add(size)?;

                let region_end = (region as usize) + (*region).size;
                if alloc_end <= region_end {
                    let excess_size = region_end - alloc_end;
                    if excess_size == 0 || excess_size >= size_of::<ListNode>() {
                        // Desvincular región de la lista libre
                        (*current).next = (*region).next;
                        return Some((region, alloc_start));
                    }
                }
                current = region;
            }
            None
        }
    }

    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(align_of::<ListNode>())
            .expect("Alineación no válida")
            .pad_to_align();
        let size = layout.size().max(size_of::<ListNode>());
        (size, layout.align())
    }
}

pub struct LockedHeap(SpinMutex<LinkedListAllocator>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Normal,   // Uso < 75%
    Warning,  // Uso entre 75% y 89%
    Critical, // Uso >= 90% (Umbral Anti-OOM)
}

pub const EMERGENCY_RESERVE_BYTES: usize = 64 * 1024; // 64 KiB para estabilidad del kernel

impl LockedHeap {
    pub const fn empty() -> Self {
        Self(SpinMutex::new(LinkedListAllocator::new()))
    }

    // SAFETY: El llamador debe garantizar que `[heap_start, heap_start + heap_size)` es un rango de memoria física válido y exclusivo.
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        self.0.lock().init(heap_start, heap_size);
    }

    pub fn stats(&self) -> (usize, usize, usize, usize, usize) {
        let lock = self.0.lock();
        (
            lock.total_size,
            lock.allocated_bytes,
            lock.total_size.saturating_sub(lock.allocated_bytes),
            lock.alloc_count,
            lock.dealloc_count,
        )
    }

    pub fn usage_percent(&self) -> usize {
        let lock = self.0.lock();
        if lock.total_size == 0 {
            0
        } else {
            (lock.allocated_bytes * 100) / lock.total_size
        }
    }

    pub fn memory_pressure(&self) -> MemoryPressure {
        let pct = self.usage_percent();
        if pct < 75 {
            MemoryPressure::Normal
        } else if pct < 90 {
            MemoryPressure::Warning
        } else {
            MemoryPressure::Critical
        }
    }

    /// Recorre la lista de bloques libres para forzar coalescencia y compactación de fragmentos
    pub fn emergency_reclaim(&self) -> usize {
        let allocator = self.0.lock();
        let mut merged_bytes = 0;
        // SAFETY: Acceso exclusivo bajo `self.0.lock()` a los punteros `ListNode` válidos de la lista libre ordenada.
        unsafe {
            let mut curr = allocator.head.next;
            while !curr.is_null() && !(*curr).next.is_null() {
                let next = (*curr).next;
                if (curr as usize) + (*curr).size == (next as usize) {
                    let freed = (*next).size;
                    (*curr).size += freed;
                    (*curr).next = (*next).next;
                    merged_bytes += freed;
                } else {
                    curr = next;
                }
            }
        }
        merged_bytes
    }
}

// SAFETY: `LockedHeap` sincroniza todas las operaciones `alloc` y `dealloc` mediante `SpinMutex` inhibiendo interrupciones.
unsafe impl GlobalAlloc for LockedHeap {
    // SAFETY: `layout` tiene tamaño y alineación ajustados a `ListNode`, y el acceso a la lista libre está serializado por `self.0.lock()`.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.0.lock();

        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start + size;
            let region_end = (region as usize) + (*region).size;
            let excess_size = region_end - alloc_end;
            if excess_size > 0 {
                allocator.add_free_region(alloc_end, excess_size);
            }
            allocator.allocated_bytes += size;
            allocator.alloc_count += 1;
            alloc_start as *mut u8
        } else {
            null_mut()
        }
    }

    // SAFETY: `ptr` y `layout` provienen de una asignación previa de este mismo `LockedHeap`, protegida por `self.0.lock()`.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (size, _) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.0.lock();
        allocator.add_free_region(ptr as usize, size);
        allocator.allocated_bytes = allocator.allocated_bytes.saturating_sub(size);
        allocator.dealloc_count += 1;
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

#[global_allocator]
pub static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();
