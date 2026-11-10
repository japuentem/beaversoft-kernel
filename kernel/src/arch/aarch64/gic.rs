// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: ARM Generic Interrupt Controller (GICv2) Driver for AArch64
// ============================================================================

use core::ptr::{read_volatile, write_volatile};

pub const GICD_BASE: usize = 0x0800_0000; // Distributor
pub const GICC_BASE: usize = 0x0801_0000; // CPU Interface

// Registros del Distributor (GICD)
const GICD_CTLR: usize = GICD_BASE + 0x000;
const GICD_TYPER: usize = GICD_BASE + 0x004;
const GICD_ISENABLER: usize = GICD_BASE + 0x100;
const GICD_IPRIORITYR: usize = GICD_BASE + 0x400;
const GICD_ITARGETSR: usize = GICD_BASE + 0x800;

// Registros de la CPU Interface (GICC)
const GICC_CTLR: usize = GICC_BASE + 0x000;
const GICC_PMR: usize = GICC_BASE + 0x004;
const GICC_IAR: usize = GICC_BASE + 0x00C;
const GICC_EOIR: usize = GICC_BASE + 0x010;

pub struct GicV2;

impl GicV2 {
    /// Inicializa el distribuidor GICD y la interfaz de CPU GICC
    pub fn init() {
        unsafe {
            // 1. Deshabilitar Distributor durante la inicialización
            write_volatile(GICD_CTLR as *mut u32, 0);

            // 2. Deshabilitar y limpiar todas las interrupciones compartidas (SPI)
            let typer = read_volatile(GICD_TYPER as *const u32);
            let num_lines = ((typer & 0x1F) + 1) * 32;

            for i in (32..num_lines).step_by(32) {
                let reg_idx = (i / 32) as usize;
                write_volatile((GICD_BASE + 0x180 + reg_idx * 4) as *mut u32, 0xFFFFFFFF);
            }

            // 3. Enrutar todas las interrupciones SPI hacia la CPU 0 (bit 0 = 0x01010101)
            for i in (32..num_lines).step_by(4) {
                let reg_idx = (i / 4) as usize;
                write_volatile((GICD_ITARGETSR + reg_idx * 4) as *mut u32, 0x01010101);
            }

            // 4. Fijar prioridad predeterminada media (0x80) para todas las líneas
            for i in (0..num_lines).step_by(4) {
                let reg_idx = (i / 4) as usize;
                write_volatile((GICD_IPRIORITYR + reg_idx * 4) as *mut u32, 0x80808080);
            }

            // 5. Habilitar el Distributor (GICD)
            write_volatile(GICD_CTLR as *mut u32, 1);

            // 6. Configurar CPU Interface (GICC): máscara de prioridad 0xF0 (acepta todo)
            write_volatile(GICC_PMR as *mut u32, 0xF0);

            // 7. Habilitar la CPU Interface (GICC)
            write_volatile(GICC_CTLR as *mut u32, 1);
        }
    }

    /// Habilita una línea de interrupción específica en el GIC
    pub fn enable_irq(irq: u32) {
        let reg_offset = (irq / 32) as usize * 4;
        let bit = 1 << (irq % 32);
        unsafe {
            let addr = (GICD_ISENABLER + reg_offset) as *mut u32;
            let val = read_volatile(addr);
            write_volatile(addr, val | bit);
        }
    }

    /// Reconoce la interrupción activa (obtiene el ID de IRQ desde GICC_IAR)
    pub fn acknowledge_irq() -> u32 {
        unsafe {
            read_volatile(GICC_IAR as *const u32) & 0x3FF
        }
    }

    /// Señaliza fin de interrupción (End Of Interrupt - EOI)
    pub fn end_of_irq(irq: u32) {
        unsafe {
            write_volatile(GICC_EOIR as *mut u32, irq);
        }
    }
}

