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

impl Aarch64Mmu {
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
