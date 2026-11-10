// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: PrimeCell PL011 UART Driver for AArch64 (QEMU virt & SBCs)
// ============================================================================

use core::ptr::{read_volatile, write_volatile};

pub const PL011_UART_BASE: usize = 0x0900_0000;

const UART_DR: usize = PL011_UART_BASE + 0x00;
const UART_FR: usize = PL011_UART_BASE + 0x18;
const UART_IBRD: usize = PL011_UART_BASE + 0x24;
const UART_FBRD: usize = PL011_UART_BASE + 0x28;
const UART_LCRH: usize = PL011_UART_BASE + 0x2C;
const UART_CR: usize = PL011_UART_BASE + 0x30;

const FR_TXFF: u32 = 1 << 5; // Transmit FIFO full
const FR_RXFE: u32 = 1 << 4; // Receive FIFO empty

pub struct Pl011Uart;

impl Pl011Uart {
    /// Inicializa el controlador UART PL011 a 115200 baudios, 8N1
    pub fn init() {
        unsafe {
            // 1. Deshabilitar UART durante la configuración
            write_volatile(UART_CR as *mut u32, 0);

            // 2. Configurar divisores de baudios para 115200 a reloj de 24 MHz
            // IBRD = 24MHz / (16 * 115200) = 13.0208 -> IBRD=13, FBRD=1
            write_volatile(UART_IBRD as *mut u32, 13);
            write_volatile(UART_FBRD as *mut u32, 1);

            // 3. Formato 8 bits de datos, FIFO habilitada, sin paridad (WLEN=0b11, FEN=1)
            write_volatile(UART_LCRH as *mut u32, (0x3 << 5) | (1 << 4));

            // 4. Habilitar UART, Transmisión (TXE) y Recepción (RXE)
            write_volatile(UART_CR as *mut u32, (1 << 0) | (1 << 8) | (1 << 9));
        }
    }

    /// Escribe un byte a la consola serial
    pub fn write_byte(b: u8) {
        unsafe {
            // Esperar mientras la cola de transmisión esté llena
            while (read_volatile(UART_FR as *const u32) & FR_TXFF) != 0 {}
            write_volatile(UART_DR as *mut u32, b as u32);
        }
    }

    /// Escribe una cadena de texto en la consola serial
    pub fn write_str(s: &str) {
        for b in s.bytes() {
            if b == b'\n' {
                Self::write_byte(b'\r');
            }
            Self::write_byte(b);
        }
    }

    /// Lee un byte si hay datos disponibles en la cola de recepción
    pub fn read_byte() -> Option<u8> {
        unsafe {
            if (read_volatile(UART_FR as *const u32) & FR_RXFE) == 0 {
                Some((read_volatile(UART_DR as *const u32) & 0xFF) as u8)
            } else {
                None
            }
        }
    }
}

