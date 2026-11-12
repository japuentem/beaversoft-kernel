// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: AArch64 MMU, Paging & Translation Table Controller
//              4-Level 4KB Granule, TTBR0/TTBR1 Higher-Half Architecture
// ============================================================================

use core::arch::asm;

// Atributos de memoria para MAIR_EL1
pub const MAIR_DEVICE_NGNRNE: u64 = 0x00; // Device Memory (MMIO sin reordenamiento ni buffers)
pub const MAIR_NORMAL_WB: u64     = 0xFF; // Normal Memory (Cacheable Write-Back)

// Descriptores de tabla de páginas AArch64 (4KB granule)
pub const PTE_VALID: u64    = 1 << 0;
pub const PTE_TABLE: u64    = 1 << 1;
pub const PTE_PAGE: u64     = 1 << 1;
pub const PTE_AF: u64       = 1 << 10; // Access Flag (evita fallos de acceso en hardware moderno)
pub const PTE_SH_INNER: u64 = 3 << 8;  // Inner Shareable (multiprocesamiento coherente)
pub const PTE_ATTR_NORM: u64= 1 << 2;  // MAIR Attr 1 (Normal Memory)
pub const PTE_ATTR_DEV: u64 = 0 << 2;  // MAIR Attr 0 (Device MMIO)
pub const PTE_USER: u64     = 1 << 6;  // Permiso AP[1]=1 (Accesible desde Ring 3 / EL0)

pub struct Aarch64Mmu;

#[repr(C, align(4096))]
struct PageTable512([u64; 512]);

static mut KERNEL_L0_TABLE: PageTable512 = PageTable512([0; 512]);
static mut KERNEL_L1_TABLE: PageTable512 = PageTable512([0; 512]);
static mut KERNEL_L2_RAM: PageTable512 = PageTable512([0; 512]);

impl Aarch64Mmu {
    /// Construye tablas L0->L1->L2 con mapeo de identidad (MMIO Device + RAM 2MiB blocks) y activa el MMU.
    pub fn init_and_enable_identity_mmu(fb_phys: u64) {
        unsafe {
            let l2_addr = core::ptr::addr_of!(KERNEL_L2_RAM) as u64;
            let l1_addr = core::ptr::addr_of!(KERNEL_L1_TABLE) as u64;
            let l0_addr = core::ptr::addr_of!(KERNEL_L0_TABLE) as u64;

            // L0[0] -> Tabla L1 (cubre los primeros 512 GiB)
            KERNEL_L0_TABLE.0[0] = (l1_addr & 0x0000_FFFF_FFFF_F000) | PTE_VALID | PTE_TABLE;

            // L1[0]: 0x0000_0000 .. 0x3FFF_FFFF (1 GiB Device-nGnRnE: GICv2, PL011 UART, VirtIO, PCIe MMIO)
            KERNEL_L1_TABLE.0[0] = 0x0000_0000 | PTE_VALID | PTE_AF | PTE_ATTR_DEV;

            // L1[1]: 0x4000_0000 .. 0x7FFF_FFFF -> Tabla L2 de bloques de 2 MiB (EL1-only por defecto)
            for i in 0..512 {
                let block_phys = 0x4000_0000u64 + (i as u64) * 0x20_0000;
                KERNEL_L2_RAM.0[i] = block_phys | PTE_VALID | PTE_AF | PTE_SH_INNER | PTE_ATTR_NORM;
            }
            KERNEL_L1_TABLE.0[1] = (l2_addr & 0x0000_FFFF_FFFF_F000) | PTE_VALID | PTE_TABLE;

            // Si el bootloader móvil entrega un SimpleFB >= 2 GiB, mapear su bloque de 1 GiB como Device/FB
            if fb_phys >= 0x8000_0000 {
                let l1_idx = (fb_phys >> 30) as usize;
                if l1_idx < 512 {
                    let block_base = (l1_idx as u64) << 30;
                    KERNEL_L1_TABLE.0[l1_idx] = block_base | PTE_VALID | PTE_AF | PTE_ATTR_DEV;
                }
            }

            Self::configure_attributes();
            Self::set_ttbr0(l0_addr);
            Self::set_ttbr1(l0_addr);
            Self::flush_tlb();
            Self::enable_mmu();
        }
    }

    /// Devuelve la dirección física de la tabla raíz `KERNEL_L0_TABLE` del supervisor.
    pub fn kernel_l0_phys() -> u64 {
        core::ptr::addr_of!(KERNEL_L0_TABLE) as u64
    }

    /// Restaura `TTBR0_EL1` a la tabla raíz del kernel e invalida TLB.
    pub fn switch_to_kernel_ttbr0() {
        Self::set_ttbr0(Self::kernel_l0_phys());
        Self::flush_tlb();
    }

    /// Crea un espacio de direcciones `TTBR0_EL1` aislado por proceso usando marcos de 4 KiB.
    pub fn create_process_address_space() -> Option<u64> {
        use crate::memory::frame_allocator::FRAME_ALLOCATOR;
        let proc_l0 = FRAME_ALLOCATOR.allocate_frame()? as u64;
        let proc_l1 = FRAME_ALLOCATOR.allocate_frame()? as u64;
        let proc_l2 = FRAME_ALLOCATOR.allocate_frame()? as u64;

        unsafe {
            let l0_ptr = proc_l0 as *mut u64;
            let l1_ptr = proc_l1 as *mut u64;
            let l2_ptr = proc_l2 as *mut u64;

            *l0_ptr = (proc_l1 & 0x0000_FFFF_FFFF_F000) | PTE_VALID | PTE_TABLE;
            *l1_ptr = KERNEL_L1_TABLE.0[0]; // MMIO 0..1 GiB (EL1-only)

            for i in 0..512 {
                // Copiar bloques de 2 MiB de RAM sin bit PTE_USER (EL1-only)
                *l2_ptr.add(i) = KERNEL_L2_RAM.0[i] & !PTE_USER;
            }
            *l1_ptr.add(1) = (proc_l2 & 0x0000_FFFF_FFFF_F000) | PTE_VALID | PTE_TABLE;

            for i in 2..512 {
                *l1_ptr.add(i) = KERNEL_L1_TABLE.0[i];
            }
        }
        Some(proc_l0)
    }

    /// Mapea una página de 4 KiB accesible desde `EL0` (`PTE_USER`) en un `proc_l0` aislado.
    pub fn map_user_page(proc_l0: u64, vaddr: u64, phys: u64, writable: bool) -> Result<(), &'static str> {
        use crate::memory::frame_allocator::FRAME_ALLOCATOR;
        let l0_idx = ((vaddr >> 39) & 0x1FF) as usize;
        let l1_idx = ((vaddr >> 30) & 0x1FF) as usize;
        let l2_idx = ((vaddr >> 21) & 0x1FF) as usize;
        let l3_idx = ((vaddr >> 12) & 0x1FF) as usize;

        if l0_idx != 0 {
            return Err("vaddr fuera del espacio inferior TTBR0 (L0[0])");
        }

        unsafe {
            let l0_ptr = proc_l0 as *mut u64;
            let l0_entry = *l0_ptr.add(l0_idx);
            if (l0_entry & PTE_VALID) == 0 {
                return Err("L0[0] inválido en proceso ARM64");
            }
            let l1_ptr = (l0_entry & 0x0000_FFFF_FFFF_F000) as *mut u64;
            let mut l1_entry = *l1_ptr.add(l1_idx);
            if (l1_entry & PTE_VALID) == 0 || (l1_entry & PTE_TABLE) == 0 {
                let new_l2 = FRAME_ALLOCATOR.allocate_frame().ok_or("Sin marcos para L2 de usuario ARM64")? as u64;
                l1_entry = (new_l2 & 0x0000_FFFF_FFFF_F000) | PTE_VALID | PTE_TABLE;
                *l1_ptr.add(l1_idx) = l1_entry;
            }

            let l2_ptr = (l1_entry & 0x0000_FFFF_FFFF_F000) as *mut u64;
            let mut l2_entry = *l2_ptr.add(l2_idx);
            if (l2_entry & PTE_VALID) == 0 || (l2_entry & PTE_TABLE) == 0 {
                let new_l3 = FRAME_ALLOCATOR.allocate_frame().ok_or("Sin marcos para L3 de usuario ARM64")? as u64;
                let l3_ptr = new_l3 as *mut u64;
                if (l2_entry & PTE_VALID) != 0 {
                    // Dividir bloque de 2 MiB en 512 páginas de 4 KiB manteniendo EL1-only
                    let block_base = l2_entry & 0x0000_FFFF_FFE0_0000;
                    for i in 0..512 {
                        *l3_ptr.add(i) = (block_base + (i as u64) * 4096)
                            | PTE_VALID
                            | PTE_PAGE
                            | PTE_AF
                            | PTE_SH_INNER
                            | PTE_ATTR_NORM;
                    }
                }
                l2_entry = (new_l3 & 0x0000_FFFF_FFFF_F000) | PTE_VALID | PTE_TABLE;
                *l2_ptr.add(l2_idx) = l2_entry;
            }

            let l3_ptr = (l2_entry & 0x0000_FFFF_FFFF_F000) as *mut u64;
            let ap2_ro = if writable { 0 } else { 1u64 << 7 };
            *l3_ptr.add(l3_idx) = (phys & 0x0000_FFFF_FFFF_F000)
                | PTE_VALID
                | PTE_PAGE
                | PTE_AF
                | PTE_SH_INNER
                | PTE_ATTR_NORM
                | PTE_USER
                | ap2_ro;
        }
        Ok(())
    }

    /// Habilita acceso EL0 (PTE_USER) únicamente en el bloque de 2 MiB que contiene `phys_addr` (dentro de 0x4000_0000..0x7FFF_FFFF).
    pub fn allow_el0_2mb_block(phys_addr: u64) {
        if phys_addr < 0x4000_0000 || phys_addr >= 0x8000_0000 {
            return;
        }
        let idx = ((phys_addr - 0x4000_0000) >> 21) as usize;
        unsafe {
            KERNEL_L2_RAM.0[idx] |= PTE_USER;
            Self::flush_tlb();
        }
    }

    /// Inicializa registros de control del MMU (MAIR, TCR) antes de habilitar paginación
    pub fn configure_attributes() {
        let mair = MAIR_DEVICE_NGNRNE | (MAIR_NORMAL_WB << 8);

        // TCR_EL1:
        // T0SZ = 16 (espacio virtual de 48 bits para EL0: 0x0000_0000_0000..0x0000_FFFF_FFFF_FFFF)
        // T1SZ = 16 (espacio virtual de 48 bits para EL1: 0xFFFF_0000_0000..0xFFFF_FFFF_FFFF_FFFF)
        // TG0 = 0b00 (gránulo de 4KB para TTBR0)
        // TG1 = 0b10 (gránulo de 4KB para TTBR1)
        // IPS = 0b010 (40 bits físico: 1 TiB de direccionamiento real)
        // ORGN0/IRGN0/ORGN1/IRGN1 = Write-Back, Read-Allocate, Write-Allocate Cacheable
        let tcr: u64 = (16 << 0)   // T0SZ
            | (16 << 16)           // T1SZ
            | (0b00 << 14)         // TG0 = 4KB
            | (0b10 << 30)         // TG1 = 4KB
            | (0b010 << 32)        // IPS = 40 bits
            | (0b01 << 8)          // IRGN0 = Normal WB
            | (0b01 << 10)         // ORGN0 = Normal WB
            | (0b01 << 24)         // IRGN1 = Normal WB
            | (0b01 << 26)         // ORGN1 = Normal WB
            | (0b11 << 12)         // SH0 = Inner Shareable
            | (0b11 << 28);        // SH1 = Inner Shareable

        unsafe {
            asm!("msr mair_el1, {}", in(reg) mair, options(nomem, nostack));
            asm!("msr tcr_el1, {}", in(reg) tcr, options(nomem, nostack));
            asm!("isb", options(nomem, nostack));
        }
    }

    /// Carga la tabla de páginas de espacio de usuario (EL0)
    pub fn set_ttbr0(table_phys_addr: u64) {
        unsafe {
            asm!("msr ttbr0_el1, {}", in(reg) table_phys_addr, options(nomem, nostack));
            asm!("isb", options(nomem, nostack));
        }
    }

    /// Carga la tabla de páginas del kernel supervisor (EL1)
    pub fn set_ttbr1(table_phys_addr: u64) {
        unsafe {
            asm!("msr ttbr1_el1, {}", in(reg) table_phys_addr, options(nomem, nostack));
            asm!("isb", options(nomem, nostack));
        }
    }

    /// Invalida todas las entradas de TLB en EL1
    pub fn flush_tlb() {
        unsafe {
            asm!("dsb ishst", options(nomem, nostack));
            asm!("tlbi vmalle1is", options(nomem, nostack));
            asm!("dsb ish", options(nomem, nostack));
            asm!("isb", options(nomem, nostack));
        }
    }

    /// Habilita el MMU y los cachés L1/L2 en SCTLR_EL1
    pub fn enable_mmu() {
        unsafe {
            let mut sctlr: u64;
            asm!("mrs {}, sctlr_el1", out(reg) sctlr, options(nomem, nostack));
            sctlr |= (1 << 0)  // M: Habilita el MMU
                   | (1 << 2)  // C: Habilita Data Cache
                   | (1 << 12); // I: Habilita Instruction Cache
            asm!("msr sctlr_el1, {}", in(reg) sctlr, options(nomem, nostack));
            asm!("isb", options(nomem, nostack));
        }
    }
}

pub fn set_ttbr0(table_phys_addr: u64) {
    Aarch64Mmu::set_ttbr0(table_phys_addr);
}

pub fn invalidate_tlb() {
    Aarch64Mmu::flush_tlb();
}
