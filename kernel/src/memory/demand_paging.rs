// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Demand Paging & Copy-on-Write (CoW) Subsystem for Page Fault (#PF 14)
// ============================================================================

use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_arch = "x86_64")]
use crate::memory::paging::{
    map_user_page, PageTable, PAGE_ADDR_MASK, PAGE_PRESENT,
    PAGE_USER, PAGE_WRITABLE, flush_tlb_page, read_cr3
};
#[cfg(target_arch = "x86_64")]
use crate::memory::frame_allocator::{FRAME_ALLOCATOR, PAGE_SIZE};
use crate::syscall::{USER_HEAP_START, USER_HEAP_MAX};
use crate::memory::paging::USER_STACK_VADDR;

/// Bit de software en la entrada PTE (Bit 9 disponible para el SO en x86_64)
pub const PTE_COW_BIT: u64 = 0x200;

/// Contadores de telemetría de fallos de página resueltos en tiempo real
pub static DEMAND_PAGE_FAULTS: AtomicU64 = AtomicU64::new(0);
pub static COW_PAGE_FAULTS: AtomicU64 = AtomicU64::new(0);

/// Evalúa y resuelve un fallo de página (#PF) ocurrido en espacio de usuario.
/// Devuelve `true` si el fallo fue resuelto exitosamente (por demanda o CoW),
/// permitiendo a la CPU reintentar la instrucción transparente y atómicamente.
pub fn handle_page_fault(fault_addr: u64, error_code: u64) -> bool {
    let page_not_present = (error_code & 0x1) == 0;
    let is_write = (error_code & 0x2) != 0;

    let aligned_vaddr = fault_addr & !0xFFF;

    // 1. Caso de Paginación por Demanda (Página ausente en memoria física)
    if page_not_present {
        // Verificar si la dirección virtual pertenece a una región válida para asignación por demanda:
        // a) Heap de usuario dinámico (USER_HEAP_START .. USER_HEAP_MAX)
        // b) Pila de usuario dinámica (hasta 2 MiB por debajo o por encima de USER_STACK_VADDR)
        // c) Región anónima de memoria mapeada de usuario (0x4000_0000 .. 0x7FFF_FFFF)
        let is_valid_user_range = (aligned_vaddr >= USER_HEAP_START && aligned_vaddr < USER_HEAP_MAX)
            || (aligned_vaddr >= USER_STACK_VADDR.saturating_sub(0x0020_0000) && aligned_vaddr < (USER_STACK_VADDR + 0x0020_0000))
            || (aligned_vaddr >= 0x4000_0000 && aligned_vaddr < 0x8000_0000);

        if is_valid_user_range {
            #[cfg(target_arch = "x86_64")]
            {
                let proc_cr3 = read_cr3();
                if proc_cr3 != 0 {
                    if let Some(frame) = FRAME_ALLOCATOR.allocate_frame() {
                        // Limpiar el marco físico con ceros por seguridad (evitar fuga de datos del kernel a usuario)
                        unsafe {
                            core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE);
                        }

                        if map_user_page(proc_cr3, aligned_vaddr, frame as u64, true).is_ok() {
                            DEMAND_PAGE_FAULTS.fetch_add(1, Ordering::SeqCst);
                            return true;
                        }
                    }
                }
            }
        }
    }

    // 2. Caso de Copy-on-Write (CoW): Intento de escritura en página presente marcada con PTE_COW_BIT
    if !page_not_present && is_write {
        #[cfg(target_arch = "x86_64")]
        {
            let proc_cr3 = read_cr3();
            if proc_cr3 != 0 {
                if let Some((pte_ptr, is_cow)) = check_and_get_cow_pte(proc_cr3, aligned_vaddr) {
                    if is_cow {
                        if let Some(new_frame) = FRAME_ALLOCATOR.allocate_frame() {
                            unsafe {
                                let old_pte = *pte_ptr;
                                let old_phys = old_pte & PAGE_ADDR_MASK;
                                // Copiar íntegramente los 4096 bytes del marco original al nuevo marco privado
                                core::ptr::copy_nonoverlapping(old_phys as *const u8, new_frame as *mut u8, PAGE_SIZE);

                                // Actualizar la PTE: nuevo marco, otorgar permisos de escritura y apagar bit CoW
                                let new_pte = (new_frame as u64 & PAGE_ADDR_MASK)
                                    | (old_pte & 0xFFF & !PTE_COW_BIT)
                                    | PAGE_PRESENT
                                    | PAGE_USER
                                    | PAGE_WRITABLE;

                                *pte_ptr = new_pte;
                            }
                            flush_tlb_page(aligned_vaddr);
                            COW_PAGE_FAULTS.fetch_add(1, Ordering::SeqCst);
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Marca una página mapeada como Copy-on-Write (CoW):
/// Quita el bit `PAGE_WRITABLE` y activa `PTE_COW_BIT`.
#[cfg(target_arch = "x86_64")]
pub fn mark_page_cow(pml4_phys: u64, vaddr: u64) -> Result<(), &'static str> {
    let aligned_vaddr = vaddr & !0xFFF;
    if let Some((pte_ptr, _)) = check_and_get_cow_pte(pml4_phys, aligned_vaddr) {
        unsafe {
            let mut pte = *pte_ptr;
            pte &= !PAGE_WRITABLE; // Solo lectura
            pte |= PTE_COW_BIT;    // Habilitar CoW
            *pte_ptr = pte;
        }
        if read_cr3() == (pml4_phys & PAGE_ADDR_MASK) {
            flush_tlb_page(aligned_vaddr);
        }
        Ok(())
    } else {
        Err("La página no está mapeada para marcar CoW")
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn mark_page_cow(_pml4_phys: u64, _vaddr: u64) -> Result<(), &'static str> {
    Ok(())
}

/// Inspecciona las tablas de páginas PML4 -> PDPT -> PD -> PT para `vaddr`
/// y retorna el puntero crudo a la entrada PTE y un booleano indicando si tiene `PTE_COW_BIT`.
#[cfg(target_arch = "x86_64")]
fn check_and_get_cow_pte(pml4_phys: u64, vaddr: u64) -> Option<(*mut u64, bool)> {
    let pml4_idx = ((vaddr >> 39) & 0x1FF) as usize;
    let pdpt_idx = ((vaddr >> 30) & 0x1FF) as usize;
    let pd_idx   = ((vaddr >> 21) & 0x1FF) as usize;
    let pt_idx   = ((vaddr >> 12) & 0x1FF) as usize;

    unsafe {
        let pml4 = &*((pml4_phys & PAGE_ADDR_MASK) as *const PageTable);
        let pml4e = pml4.entries[pml4_idx];
        if (pml4e & PAGE_PRESENT) == 0 { return None; }

        let pdpt = &*((pml4e & PAGE_ADDR_MASK) as *const PageTable);
        let pdpte = pdpt.entries[pdpt_idx];
        if (pdpte & PAGE_PRESENT) == 0 { return None; }

        let pd = &*((pdpte & PAGE_ADDR_MASK) as *const PageTable);
        let pde = pd.entries[pd_idx];
        if (pde & PAGE_PRESENT) == 0 { return None; }
        if (pde & crate::memory::paging::PAGE_HUGE) != 0 { return None; }

        let pt = &mut *((pde & PAGE_ADDR_MASK) as *mut PageTable);
        let pte_ptr = &mut pt.entries[pt_idx] as *mut u64;
        let pte = *pte_ptr;
        if (pte & PAGE_PRESENT) == 0 { return None; }

        let is_cow = (pte & PTE_COW_BIT) != 0;
        Some((pte_ptr, is_cow))
    }
}

/// Retorna estadísticas de fallos de página resueltos
pub fn get_stats() -> (u64, u64) {
    (
        DEMAND_PAGE_FAULTS.load(Ordering::Relaxed),
        COW_PAGE_FAULTS.load(Ordering::Relaxed),
    )
}
