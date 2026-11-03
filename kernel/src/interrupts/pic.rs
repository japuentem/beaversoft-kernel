// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Dual 8259A PIC (Programmable Interrupt Controller) Driver
// ============================================================================

use crate::io::{inb, io_wait, outb};

pub const PIC1_COMMAND: u16 = 0x20;
pub const PIC1_DATA: u16 = 0x21;
pub const PIC2_COMMAND: u16 = 0xA0;
pub const PIC2_DATA: u16 = 0xA1;

pub const PIC1_OFFSET: u8 = 32; // IRQ 0..7 remapeadas a interrupciones 32..39
pub const PIC2_OFFSET: u8 = 40; // IRQ 8..15 remapeadas a interrupciones 40..47

pub const PIC_EOI: u8 = 0x20; // End-of-Interrupt command

pub struct ChainedPics {
    pub pic1_offset: u8,
    pub pic2_offset: u8,
}

impl ChainedPics {
    pub const fn new(offset1: u8, offset2: u8) -> Self {
        Self {
            pic1_offset: offset1,
            pic2_offset: offset2,
        }
    }

    pub unsafe fn initialize(&self) {
        // Guardar máscaras actuales de interrupciones
        let _mask1 = inb(PIC1_DATA);
        let _mask2 = inb(PIC2_DATA);

        // ICW1: Inicialización en modo cascada, requiriendo ICW4
        outb(PIC1_COMMAND, 0x11);
        io_wait();
        outb(PIC2_COMMAND, 0x11);
        io_wait();

        // ICW2: Vectores base (Offsets)
        outb(PIC1_DATA, self.pic1_offset);
        io_wait();
        outb(PIC2_DATA, self.pic2_offset);
        io_wait();

        // ICW3: Conexión de cascada
        // Master: indicar que el Slave está en IRQ2 (bit 2 = 4 = 0b00000100)
        outb(PIC1_DATA, 4);
        io_wait();
        // Slave: indicar su identidad de conexión en la línea 2
        outb(PIC2_DATA, 2);
        io_wait();

        // ICW4: Modo 8086/88
        outb(PIC1_DATA, 0x01);
        io_wait();
        outb(PIC2_DATA, 0x01);
        io_wait();

        // Habilitar Timer (IRQ0), Teclado (IRQ1) y línea de Cascada (IRQ2)
        // Bit 0 = IRQ0 (Timer), Bit 1 = IRQ1 (Teclado), Bit 2 = IRQ2 (Cascada)
        outb(PIC1_DATA, 0b11111000); // 0xF8: Permitir IRQ0, IRQ1, IRQ2
        outb(PIC2_DATA, 0b11111111); // 0xFF: Enmascarar Slave por ahora
    }

    pub unsafe fn unmask_irq(&self, irq: u8) {
        if irq < 8 {
            let mask = inb(PIC1_DATA) & !(1 << irq);
            outb(PIC1_DATA, mask);
        } else {
            let mask = inb(PIC2_DATA) & !(1 << (irq - 8));
            outb(PIC2_DATA, mask);
        }
    }

    pub unsafe fn notify_end_of_interrupt(&self, interrupt_id: u8) {
        if interrupt_id >= self.pic2_offset {
            outb(PIC2_COMMAND, PIC_EOI);
        }
        outb(PIC1_COMMAND, PIC_EOI);
    }
}

pub static PICS: ChainedPics = ChainedPics::new(PIC1_OFFSET, PIC2_OFFSET);
