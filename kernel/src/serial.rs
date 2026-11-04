// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: UART 16550 Serial Driver (COM1 / 0x3F8) with fmt::Write support
// ============================================================================

use core::fmt;
use crate::io::{inb, outb};
use crate::sync::SpinMutex;

pub const COM1_BASE: u16 = 0x3F8;

pub struct SerialPort {
    port: u16,
}

impl SerialPort {
    pub const fn new(port: u16) -> Self {
        Self { port }
    }

    // SAFETY: El llamador debe asegurar que `self.port` apunta a un controlador UART 16550 válido y con acceso exclusivo.
    pub unsafe fn init(&self) {
        // Desactivar interrupciones UART
        outb(self.port + 1, 0x00);
        // Habilitar DLAB (Divisor Latch Access Bit)
        outb(self.port + 3, 0x80);
        // Configurar divisor de baudios a 3 (38400 baud)
        outb(self.port + 0, 0x03);
        outb(self.port + 1, 0x00);
        // 8 bits, sin paridad, 1 bit de parada (limpia DLAB)
        outb(self.port + 3, 0x03);
        // Habilitar FIFO, vaciar colas, umbral de 14 bytes
        outb(self.port + 2, 0xC7);
        // Habilitar RTS, DTR y línea Aux Output 2
        outb(self.port + 4, 0x0B);
    }

    fn is_transmit_empty(&self) -> bool {
        // SAFETY: Lectura del registro Line Status (`port + 5`) del UART 16550 en Ring 0.
        unsafe { (inb(self.port + 5) & 0x20) != 0 }
    }

    pub fn write_byte(&self, byte: u8) {
        while !self.is_transmit_empty() {
            core::hint::spin_loop();
        }
        // SAFETY: Escritura al registro de datos del UART (`self.port`) tras verificar que el búfer TX está vacío.
        unsafe {
            outb(self.port, byte);
        }
    }

    pub fn read_byte(&self) -> Option<u8> {
        // SAFETY: Lectura de los registros de estado y datos del UART 16550 en `self.port`.
        unsafe {
            if (inb(self.port + 5) & 0x01) != 0 {
                Some(inb(self.port))
            } else {
                None
            }
        }
    }

    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Self::write_str(self, s);
        Ok(())
    }
}

pub static SERIAL: SpinMutex<SerialPort> = SpinMutex::new(SerialPort::new(COM1_BASE));
