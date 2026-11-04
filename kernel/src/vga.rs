// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: VGA Text Buffer Driver (0xB8000) with CP437, Hardware Cursor & fmt
// ============================================================================

use core::fmt;
use crate::io::outb;
use crate::sync::SpinMutex;

pub const VGA_BUFFER_ADDR: usize = 0xb8000;
pub const VGA_WIDTH: usize = 80;
pub const VGA_HEIGHT: usize = 25;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VgaColor {
    Black = 0x0,
    Blue = 0x1,
    Green = 0x2,
    Cyan = 0x3,
    Red = 0x4,
    Magenta = 0x5,
    Brown = 0x6,
    LightGray = 0x7,
    DarkGray = 0x8,
    LightBlue = 0x9,
    LightGreen = 0xA,
    LightCyan = 0xB,
    LightRed = 0xC,
    Pink = 0xD,
    Yellow = 0xE,
    White = 0xF,
}

pub struct VgaWriter {
    row: usize,
    col: usize,
    color: u8,
    utf8_prefix: u8,
}

impl VgaWriter {
    pub const fn new(fg: VgaColor, bg: VgaColor) -> Self {
        Self {
            row: 0,
            col: 0,
            color: (bg as u8) << 4 | (fg as u8),
            utf8_prefix: 0,
        }
    }

    pub fn set_color(&mut self, fg: VgaColor, bg: VgaColor) {
        self.color = (bg as u8) << 4 | (fg as u8);
    }

    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.row = row % VGA_HEIGHT;
        self.col = col % VGA_WIDTH;
        self.update_hardware_cursor();
    }

    pub fn get_position(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn clear(&mut self) {
        let buffer = VGA_BUFFER_ADDR as *mut u16;
        let blank_entry = (self.color as u16) << 8 | (b' ' as u16);
        for i in 0..(VGA_WIDTH * VGA_HEIGHT) {
            // SAFETY: `0xB8000` es el búfer MMIO de texto VGA mapeado por identidad y `i` está acotado a `80 * 25`.
            unsafe {
                core::ptr::write_volatile(buffer.add(i), blank_entry);
            }
        }
        self.row = 0;
        self.col = 0;
        self.utf8_prefix = 0;
        self.update_hardware_cursor();
    }

    pub fn write_char_at(&mut self, row: usize, col: usize, byte: u8, fg: VgaColor, bg: VgaColor) {
        if row < VGA_HEIGHT && col < VGA_WIDTH {
            let offset = row * VGA_WIDTH + col;
            let buffer = VGA_BUFFER_ADDR as *mut u16;
            let color_attr = ((bg as u8) << 4) | (fg as u8);
            let entry = (color_attr as u16) << 8 | (byte as u16);
            // SAFETY: `row` y `col` están verificados dentro de los límites del búfer MMIO VGA en `0xB8000`.
            unsafe {
                core::ptr::write_volatile(buffer.add(offset), entry);
            }
        }
    }

    pub fn write_str_at(&mut self, row: usize, mut col: usize, s: &str, fg: VgaColor, bg: VgaColor) {
        for b in s.bytes() {
            if col >= VGA_WIDTH {
                break;
            }
            self.write_char_at(row, col, b, fg, bg);
            col += 1;
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        if byte == b'\n' {
            self.new_line();
            return;
        }

        if byte == 0x08 { // Backspace
            if self.col > 0 {
                self.col -= 1;
                let offset = self.row * VGA_WIDTH + self.col;
                let buffer = VGA_BUFFER_ADDR as *mut u16;
                let entry = (self.color as u16) << 8 | (b' ' as u16);
                // SAFETY: `self.row` y `self.col` se mantienen dentro de `25x80`, apuntando a una celda válida en `0xB8000`.
                unsafe {
                    core::ptr::write_volatile(buffer.add(offset), entry);
                }
                self.update_hardware_cursor();
            }
            return;
        }

        // Manejo de secuencia UTF-8 de 2 bytes a Code Page 437 (CP437)
        if byte == 0xC3 {
            self.utf8_prefix = 0xC3;
            return;
        }

        let cp437_char = if self.utf8_prefix == 0xC3 {
            self.utf8_prefix = 0;
            match byte {
                0xA1 => 0xA0, // á
                0xA9 => 0x82, // é
                0xAD => 0xA1, // í
                0xB3 => 0xA2, // ó
                0xBA => 0xA3, // ú
                0xB1 => 0xA4, // ñ
                0xBC => 0xFC, // ü
                0x81 => 0x8F, // Á
                0x89 => 0x90, // É
                0x8D => 0x49, // Í
                0x93 => 0x4F, // Ó
                0x9A => 0x55, // Ú
                0x91 => 0xA5, // Ñ
                _    => b'?',
            }
        } else {
            byte
        };

        if self.col >= VGA_WIDTH {
            self.new_line();
        }

        let offset = self.row * VGA_WIDTH + self.col;
        let buffer = VGA_BUFFER_ADDR as *mut u16;
        let entry = (self.color as u16) << 8 | (cp437_char as u16);

        // SAFETY: `offset` está garantizado menor que `VGA_WIDTH * VGA_HEIGHT` dentro del búfer MMIO `0xB8000`.
        unsafe {
            core::ptr::write_volatile(buffer.add(offset), entry);
        }

        self.col += 1;
        self.update_hardware_cursor();
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    fn new_line(&mut self) {
        self.col = 0;
        if self.row < VGA_HEIGHT - 1 {
            self.row += 1;
        } else {
            self.scroll();
        }
        self.update_hardware_cursor();
    }

    fn scroll(&mut self) {
        let buffer = VGA_BUFFER_ADDR as *mut u16;
        for r in 1..VGA_HEIGHT {
            for c in 0..VGA_WIDTH {
                let src_offset = r * VGA_WIDTH + c;
                let dst_offset = (r - 1) * VGA_WIDTH + c;
                // SAFETY: `src_offset` y `dst_offset` están dentro del rango `[0, 80 * 25)` del búfer VGA `0xB8000`.
                unsafe {
                    let entry = core::ptr::read_volatile(buffer.add(src_offset));
                    core::ptr::write_volatile(buffer.add(dst_offset), entry);
                }
            }
        }
        let blank_entry = (self.color as u16) << 8 | (b' ' as u16);
        for c in 0..VGA_WIDTH {
            let offset = (VGA_HEIGHT - 1) * VGA_WIDTH + c;
            // SAFETY: `offset` corresponde a la última fila del búfer VGA `0xB8000` dentro de límites válidos.
            unsafe {
                core::ptr::write_volatile(buffer.add(offset), blank_entry);
            }
        }
    }

    fn update_hardware_cursor(&self) {
        let pos = (self.row * VGA_WIDTH + self.col) as u16;
        // SAFETY: Los puertos CRT Controller `0x3D4` y `0x3D5` actualizan la posición del cursor VGA en Ring 0.
        unsafe {
            outb(0x3D4, 0x0F);
            outb(0x3D5, (pos & 0xFF) as u8);
            outb(0x3D4, 0x0E);
            outb(0x3D5, ((pos >> 8) & 0xFF) as u8);
        }
    }
}

impl fmt::Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

pub static VGA_WRITER: SpinMutex<VgaWriter> = SpinMutex::new(VgaWriter::new(VgaColor::White, VgaColor::Black));
