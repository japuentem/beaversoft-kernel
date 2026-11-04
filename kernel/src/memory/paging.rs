// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: x86_64 4-Level Paging Engine (4 KiB Fine-Grained Kernel Tables,
//              Per-Process CR3 Address Spaces & Strict PAGE_USER Ring 3 Isolation)
// ============================================================================

use core::arch::asm;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use crate::memory::frame_allocator::{FRAME_ALLOCATOR, PAGE_SIZE};

pub const PAGE_PRESENT: u64       = 1 << 0; // 0x01: Página presente en RAM física
pub const PAGE_WRITABLE: u64      = 1 << 1; // 0x02: Lectura y escritura permitidas
pub const PAGE_USER: u64          = 1 << 2; // 0x04: Accesible desde Ring 3 (CPL=3)
pub const PAGE_WRITE_THROUGH: u64 = 1 << 3; // 0x08: Write-Through caching
pub const PAGE_CACHE_DISABLE: u64 = 1 << 4; // 0x10: Deshabilitar caché (MMIO)
pub const PAGE_ACCESSED: u64      = 1 << 5; // 0x20: Accedida por la CPU
pub const PAGE_DIRTY: u64         = 1 << 6; // 0x40: Modificada por la CPU
pub const PAGE_HUGE: u64          = 1 << 7; // 0x80: Página enorme de 2 MiB en PD
pub const PAGE_ADDR_MASK: u64     = 0x000F_FFFF_FFFF_F000;

/// Número de tablas PT de 4 KiB para mapear los primeros 64 MiB del kernel en páginas de 4 KiB
/// (32 tablas PT * 512 entradas * 4 KiB = 64 MiB = 16,384 páginas de 4 KiB).
pub const KERNEL_FINE_PT_TABLES: usize = 32;
pub const KERNEL_FINE_PAGES_COUNT: usize = KERNEL_FINE_PT_TABLES * 512;

/// Región virtual dedicada para procesos aislados de Ring 3 (fuera de los primeros 64 MiB del kernel):
/// - Código/Datos ELF y tareas de usuario: `0x0800_0000` (128 MiB) en adelante
/// - Pila de usuario (16 KiB = 4 páginas de 4 KiB): `0x0840_0000 .. 0x0840_4000` (132 MiB)
pub const USER_CODE_VADDR: u64  = 0x0000_0000_0800_0000;
pub const USER_ELF_VADDR: u64   = 0x0000_0000_0800_1000;
pub const USER_STACK_VADDR: u64 = 0x0000_0000_0840_0000;
pub const USER_STACK_PAGES: usize = 4; // 16 KiB

/// CR3 maestro del kernel (tabla PML4 supervisora con PAGE_USER = 0 en todo el espacio)
static KERNEL_CR3: AtomicU64 = AtomicU64::new(0);
/// Conteo de páginas de 4 KiB activas en la tabla maestra del kernel
static FINE_4K_PAGES_MAPPED: AtomicUsize = AtomicUsize::new(0);
/// Conteo de espacios de direcciones (CR3) por proceso creados
static PROCESS_CR3_COUNT: AtomicUsize = AtomicUsize::new(0);

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [u64; 512],
}

/// Lee el valor físico actual del registro `CR3`.
#[inline(always)]
pub fn read_cr3() -> u64 {
    let cr3: u64;
    // SAFETY: La lectura de `CR3` en Ring 0 es una operación no destructiva que obtiene la base física del PML4 activo.
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
    }
    cr3 & PAGE_ADDR_MASK
}

/// Carga una nueva tabla PML4 física en el registro `CR3`, vaciando el TLB no global.
#[inline(always)]
// SAFETY: El llamador garantiza que `pml4_phys` apunta a una tabla PML4 física válida de 4 KiB que incluye el mapeo supervisor del kernel.
pub unsafe fn write_cr3(pml4_phys: u64) {
    let aligned = pml4_phys & PAGE_ADDR_MASK;
    asm!("mov cr3, {}", in(reg) aligned, options(nostack, preserves_flags));
}

/// Invalida la entrada del TLB correspondiente a la dirección virtual `vaddr`.
#[inline(always)]
pub fn flush_tlb_page(vaddr: u64) {
    // SAFETY: `invlpg` solo invalida una línea de traducción en el TLB del procesador actual.
    unsafe {
        asm!("invlpg [{}]", in(reg) vaddr, options(nostack, preserves_flags));
    }
}

/// Devuelve el `CR3` maestro del kernel.
#[inline(always)]
pub fn kernel_cr3() -> u64 {
    let k = KERNEL_CR3.load(Ordering::Acquire);
    if k != 0 { k } else { read_cr3() }
}

/// Restaura el `CR3` maestro del kernel en el procesador actual.
#[inline(always)]
pub fn switch_to_kernel_cr3() {
    let k_cr3 = kernel_cr3();
    if k_cr3 != 0 && read_cr3() != k_cr3 {
        // SAFETY: `k_cr3` fue inicializado por `init_kernel_paging()` y contiene el mapeo supervisor completo del kernel.
        unsafe {
            write_cr3(k_cr3);
        }
    }
}

/// Devuelve el número de páginas finas de 4 KiB mapeadas en el kernel.
pub fn fine_4k_pages_count() -> usize {
    FINE_4K_PAGES_MAPPED.load(Ordering::Relaxed)
}

/// Devuelve el número de espacios de direcciones por proceso (`CR3`) creados.
pub fn process_cr3_count() -> usize {
    PROCESS_CR3_COUNT.load(Ordering::Relaxed)
}

/// Inicializa las tablas de páginas propias del kernel en páginas de 4 KiB para los primeros 64 MiB
/// y páginas supervisoras de 2 MiB para el resto de los 4 GiB (incluyendo MMIO VBE, E1000 y APIC).
/// Todas las entradas del kernel tienen `PAGE_USER = 0` (estrictamente Ring 0 Supervisor).
pub fn init_kernel_paging() -> u64 {
    let pml4_phys = FRAME_ALLOCATOR.allocate_frame().expect("[PAGING] Sin marcos para PML4 del kernel") as u64;
    let pdpt_phys = FRAME_ALLOCATOR.allocate_frame().expect("[PAGING] Sin marcos para PDPT del kernel") as u64;

    let mut pd_phys = [0u64; 4];
    for i in 0..4 {
        pd_phys[i] = FRAME_ALLOCATOR.allocate_frame().expect("[PAGING] Sin marcos para PD del kernel") as u64;
    }

    // 1. Enlazar PML4[0] -> PDPT (Supervisor: PAGE_PRESENT | PAGE_WRITABLE, sin PAGE_USER)
    // SAFETY: `pml4_phys` y `pdpt_phys` son marcos de 4 KiB recién asignados y puestos a cero en memoria con identity mapping.
    unsafe {
        let pml4 = &mut *(pml4_phys as *mut PageTable);
        pml4.entries[0] = pdpt_phys | PAGE_PRESENT | PAGE_WRITABLE;

        let pdpt = &mut *(pdpt_phys as *mut PageTable);
        for i in 0..4 {
            pdpt.entries[i] = pd_phys[i] | PAGE_PRESENT | PAGE_WRITABLE;
        }
    }

    // 2. Poblar los primeros 64 MiB (PD[0], entradas 0..32) con 32 tablas PT de páginas finas de 4 KiB
    let mut fine_pages = 0usize;
    for pd_idx in 0..KERNEL_FINE_PT_TABLES {
        let pt_phys = FRAME_ALLOCATOR.allocate_frame().expect("[PAGING] Sin marcos para PT de 4 KiB") as u64;
        // SAFETY: `pt_phys` y `pd_phys[0]` son marcos físicos válidos de 4 KiB pertenecientes al kernel.
        unsafe {
            let pd0 = &mut *(pd_phys[0] as *mut PageTable);
            pd0.entries[pd_idx] = pt_phys | PAGE_PRESENT | PAGE_WRITABLE;

            let pt = &mut *(pt_phys as *mut PageTable);
            let base_phys = (pd_idx * 512 * PAGE_SIZE) as u64;
            for pt_idx in 0..512 {
                let frame_phys = base_phys + (pt_idx * PAGE_SIZE) as u64;
                // Todas las páginas de 4 KiB del kernel son Supervisor (PAGE_USER = 0)
                pt.entries[pt_idx] = frame_phys | PAGE_PRESENT | PAGE_WRITABLE;
                fine_pages += 1;
            }
        }
    }

    // 3. Poblar el resto de los 4 GiB (64 MiB .. 4 GiB) con páginas supervisoras de 2 MiB (PAGE_USER = 0)
    //    para cubrir asignaciones altas de RAM y regiones MMIO PCI (VBE 0xE0000000, E1000 0xFEBC0000, APIC 0xFEE00000).
    for pd_table_idx in 0..4 {
        let start_entry = if pd_table_idx == 0 { KERNEL_FINE_PT_TABLES } else { 0 };
        // SAFETY: `pd_phys[pd_table_idx]` apunta a la tabla PD correspondiente dentro de los 4 GiB.
        unsafe {
            let pd = &mut *(pd_phys[pd_table_idx] as *mut PageTable);
            for pd_idx in start_entry..512 {
                let phys_2m = ((pd_table_idx * 512 + pd_idx) as u64) * 0x20_0000;
                pd.entries[pd_idx] = phys_2m | PAGE_PRESENT | PAGE_WRITABLE | PAGE_HUGE;
            }
        }
    }

    FINE_4K_PAGES_MAPPED.store(fine_pages, Ordering::Release);
    KERNEL_CR3.store(pml4_phys, Ordering::Release);

    // 4. Activar la nueva tabla PML4 de 4 KiB en CR3
    // SAFETY: `pml4_phys` mapea por identidad todo el kernel, pila, heap y MMIO en Ring 0.
    unsafe {
        write_cr3(pml4_phys);
    }

    pml4_phys
}

/// Crea un nuevo espacio de direcciones aislado para un proceso de usuario (`CR3` propio).
/// - Clona las referencias supervisoras del kernel (`PAGE_USER = 0` en todas las tablas PT y PD del kernel),
///   de modo que las interrupciones, excepciones y `syscall` en Ring 0 funcionen sin fallos,
///   pero Ring 3 (`CPL=3`) tenga **prohibido** por hardware MMU leer o escribir cualquier página del kernel.
pub fn create_process_address_space() -> Option<u64> {
    let k_cr3 = kernel_cr3();
    if k_cr3 == 0 {
        return None;
    }

    let proc_pml4_phys = FRAME_ALLOCATOR.allocate_frame()? as u64;
    let proc_pdpt_phys = FRAME_ALLOCATOR.allocate_frame()? as u64;
    let proc_pd0_phys  = FRAME_ALLOCATOR.allocate_frame()? as u64;

    // SAFETY: `k_cr3` y los tres marcos recién asignados (`proc_pml4_phys`, `proc_pdpt_phys`, `proc_pd0_phys`) son válidos en RAM física.
    unsafe {
        let k_pml4 = &*(k_cr3 as *const PageTable);
        let k_pdpt_phys = k_pml4.entries[0] & PAGE_ADDR_MASK;
        let k_pdpt = &*(k_pdpt_phys as *const PageTable);
        let k_pd0_phys = k_pdpt.entries[0] & PAGE_ADDR_MASK;
        let k_pd0 = &*(k_pd0_phys as *const PageTable);

        let proc_pml4 = &mut *(proc_pml4_phys as *mut PageTable);
        let proc_pdpt = &mut *(proc_pdpt_phys as *mut PageTable);
        let proc_pd0  = &mut *(proc_pd0_phys as *mut PageTable);

        // En x86_64, el bit PAGE_USER en PML4E y PDPTE habilita la evaluación del bit PAGE_USER en niveles inferiores (PDE/PTE).
        // Cualquier PDE o PTE que tenga PAGE_USER = 0 sigue siendo estrictamente Supervisor (Ring 0).
        proc_pml4.entries[0] = proc_pdpt_phys | PAGE_PRESENT | PAGE_WRITABLE | PAGE_USER;
        proc_pdpt.entries[0] = proc_pd0_phys  | PAGE_PRESENT | PAGE_WRITABLE | PAGE_USER;

        // Las tablas PD[1..3] (1 GiB .. 4 GiB, incluyendo MMIO) se enlazan sin PAGE_USER (solo Ring 0)
        for i in 1..4 {
            proc_pdpt.entries[i] = k_pdpt.entries[i] & !PAGE_USER;
        }

        // Copiar las 512 entradas de PD[0] asegurando que TODAS tengan PAGE_USER = 0 inicialmente
        for i in 0..512 {
            proc_pd0.entries[i] = k_pd0.entries[i] & !PAGE_USER;
        }
    }

    PROCESS_CR3_COUNT.fetch_add(1, Ordering::Relaxed);
    Some(proc_pml4_phys)
}

/// Mapea una página física de 4 KiB (`paddr`) en la dirección virtual (`vaddr`) dentro del espacio
/// de direcciones `pml4_phys` con los permisos indicados (`flags`).
/// Si `flags` incluye `PAGE_USER`, solo esa página de 4 KiB tendrá acceso de usuario Ring 3;
/// todas las demás páginas hermanas de 4 KiB en la misma tabla PT conservan `PAGE_USER = 0`.
pub fn map_page_in(pml4_phys: u64, vaddr: u64, paddr: u64, flags: u64) -> Result<(), &'static str> {
    let pml4_idx = ((vaddr >> 39) & 0x1FF) as usize;
    let pdpt_idx = ((vaddr >> 30) & 0x1FF) as usize;
    let pd_idx   = ((vaddr >> 21) & 0x1FF) as usize;
    let pt_idx   = ((vaddr >> 12) & 0x1FF) as usize;

    let want_user = (flags & PAGE_USER) != 0;
    let parent_flags = PAGE_PRESENT | PAGE_WRITABLE | if want_user { PAGE_USER } else { 0 };

    // SAFETY: Recorrido y actualización de las tablas jerárquicas de 4 niveles en memoria física mapeada por identidad.
    unsafe {
        let pml4 = &mut *((pml4_phys & PAGE_ADDR_MASK) as *mut PageTable);
        if (pml4.entries[pml4_idx] & PAGE_PRESENT) == 0 {
            let new_pdpt = FRAME_ALLOCATOR.allocate_frame().ok_or("Sin marcos para PDPT")? as u64;
            pml4.entries[pml4_idx] = new_pdpt | parent_flags;
        } else if want_user {
            pml4.entries[pml4_idx] |= PAGE_USER;
        }

        let pdpt_phys = pml4.entries[pml4_idx] & PAGE_ADDR_MASK;
        let pdpt = &mut *(pdpt_phys as *mut PageTable);
        if (pdpt.entries[pdpt_idx] & PAGE_PRESENT) == 0 {
            let new_pd = FRAME_ALLOCATOR.allocate_frame().ok_or("Sin marcos para PD")? as u64;
            pdpt.entries[pdpt_idx] = new_pd | parent_flags;
        } else if want_user {
            pdpt.entries[pdpt_idx] |= PAGE_USER;
        }

        let pd_phys = pdpt.entries[pdpt_idx] & PAGE_ADDR_MASK;
        let pd = &mut *(pd_phys as *mut PageTable);
        let pde = pd.entries[pd_idx];

        let pt_phys = if (pde & PAGE_PRESENT) == 0 {
            let new_pt = FRAME_ALLOCATOR.allocate_frame().ok_or("Sin marcos para PT")? as u64;
            pd.entries[pd_idx] = new_pt | parent_flags;
            new_pt
        } else if (pde & PAGE_HUGE) != 0 {
            // Partir la página enorme de 2 MiB en 512 páginas individuales de 4 KiB (Supervisor PAGE_USER = 0)
            let new_pt = FRAME_ALLOCATOR.allocate_frame().ok_or("Sin marcos para partir Huge Page a 4 KiB")? as u64;
            let base_2m = pde & 0x000F_FFFF_FFE0_0000;
            let pt = &mut *(new_pt as *mut PageTable);
            for i in 0..512 {
                pt.entries[i] = (base_2m + (i * PAGE_SIZE) as u64) | PAGE_PRESENT | PAGE_WRITABLE;
            }
            pd.entries[pd_idx] = new_pt | parent_flags;
            new_pt
        } else if want_user && (pde & PAGE_USER) == 0 {
            // La PDE apuntaba a una PT supervisora compartida del kernel; clonar en una PT privada para este proceso
            let old_pt_phys = pde & PAGE_ADDR_MASK;
            let new_pt = FRAME_ALLOCATOR.allocate_frame().ok_or("Sin marcos para clonar PT de proceso")? as u64;
            core::ptr::copy_nonoverlapping(old_pt_phys as *const u8, new_pt as *mut u8, PAGE_SIZE);
            pd.entries[pd_idx] = new_pt | parent_flags;
            new_pt
        } else {
            if want_user {
                pd.entries[pd_idx] |= PAGE_USER;
            }
            pde & PAGE_ADDR_MASK
        };

        let pt = &mut *(pt_phys as *mut PageTable);
        pt.entries[pt_idx] = (paddr & PAGE_ADDR_MASK) | (flags & 0xFFF) | PAGE_PRESENT;
    }

    if read_cr3() == (pml4_phys & PAGE_ADDR_MASK) {
        flush_tlb_page(vaddr);
    }

    Ok(())
}

/// Mapea una página de 4 KiB exclusiva de usuario (`PAGE_USER`) en el espacio de direcciones `pml4_phys`.
pub fn map_user_page(pml4_phys: u64, vaddr: u64, paddr: u64, writable: bool) -> Result<(), &'static str> {
    let mut flags = PAGE_PRESENT | PAGE_USER;
    if writable {
        flags |= PAGE_WRITABLE;
    }
    map_page_in(pml4_phys, vaddr, paddr, flags)
}

/// Consulta la entrada de tabla de páginas para `vaddr` en el `pml4_phys` indicado.
/// Devuelve `Some((phys_addr, is_4k_page, is_user_accessible))` si la página está presente.
pub fn query_mapping(pml4_phys: u64, vaddr: u64) -> Option<(u64, bool, bool)> {
    let pml4_idx = ((vaddr >> 39) & 0x1FF) as usize;
    let pdpt_idx = ((vaddr >> 30) & 0x1FF) as usize;
    let pd_idx   = ((vaddr >> 21) & 0x1FF) as usize;
    let pt_idx   = ((vaddr >> 12) & 0x1FF) as usize;

    // SAFETY: Lectura de solo consulta de las tablas de páginas físicas en `pml4_phys`.
    unsafe {
        let pml4 = &*((pml4_phys & PAGE_ADDR_MASK) as *const PageTable);
        let pml4e = pml4.entries[pml4_idx];
        if (pml4e & PAGE_PRESENT) == 0 {
            return None;
        }
        let mut user_ok = (pml4e & PAGE_USER) != 0;

        let pdpt = &*((pml4e & PAGE_ADDR_MASK) as *const PageTable);
        let pdpte = pdpt.entries[pdpt_idx];
        if (pdpte & PAGE_PRESENT) == 0 {
            return None;
        }
        user_ok &= (pdpte & PAGE_USER) != 0;

        let pd = &*((pdpte & PAGE_ADDR_MASK) as *const PageTable);
        let pde = pd.entries[pd_idx];
        if (pde & PAGE_PRESENT) == 0 {
            return None;
        }
        user_ok &= (pde & PAGE_USER) != 0;

        if (pde & PAGE_HUGE) != 0 {
            let phys = (pde & 0x000F_FFFF_FFE0_0000) + (vaddr & 0x1F_FFFF);
            return Some((phys, false, user_ok));
        }

        let pt = &*((pde & PAGE_ADDR_MASK) as *const PageTable);
        let pte = pt.entries[pt_idx];
        if (pte & PAGE_PRESENT) == 0 {
            return None;
        }
        user_ok &= (pte & PAGE_USER) != 0;
        let phys = (pte & PAGE_ADDR_MASK) + (vaddr & 0xFFF);
        Some((phys, true, user_ok))
    }
}

