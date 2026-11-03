// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: PS/2 Keyboard Driver (Port 0x60) with Scancode Set 1 Decoder
// ============================================================================

use crate::io::inb;
use crate::sync::SpinMutex;

const KEYBOARD_DATA_PORT: u16 = 0x60;

pub const KEY_UP_ARROW: char = '\x11';
pub const KEY_DOWN_ARROW: char = '\x12';

pub struct KeyboardState {
    shift_pressed: bool,
    caps_lock: bool,
    extended: bool,
}

impl KeyboardState {
    pub const fn new() -> Self {
        Self {
            shift_pressed: false,
            caps_lock: false,
            extended: false,
        }
    }

    pub fn process_scancode(&mut self, scancode: u8) -> Option<char> {
        // Manejo de prefijo extendido 0xE0 (flechas, teclas de navegación)
        if scancode == 0xE0 {
            self.extended = true;
            return None;
        }

        if self.extended {
            self.extended = false;
            // Ignorar liberaciones de teclas extendidas (bit 7 activado: 0x80..0xFF)
            if scancode >= 0x80 {
                return None;
            }
            return match scancode {
                0x48 => Some(KEY_UP_ARROW),   // Flecha Arriba
                0x50 => Some(KEY_DOWN_ARROW), // Flecha Abajo
                _ => None,
            };
        }

        // Manejo de pulsación o liberación de SHIFT
        match scancode {
            0x2A | 0x36 => { // Shift izquierdo o derecho presionado
                self.shift_pressed = true;
                return None;
            }
            0xAA | 0xB6 => { // Shift izquierdo o derecho liberado
                self.shift_pressed = false;
                return None;
            }
            0x3A => { // Caps Lock presionado
                self.caps_lock = !self.caps_lock;
                return None;
            }
            _ => {}
        }

        // Ignorar liberaciones de teclas (bit 7 activado: 0x80..0xFF)
        if scancode >= 0x80 {
            return None;
        }

        let uppercase = self.shift_pressed ^ self.caps_lock;

        let ch = match scancode {
            0x02 => if self.shift_pressed { '!' } else { '1' },
            0x03 => if self.shift_pressed { '@' } else { '2' },
            0x04 => if self.shift_pressed { '#' } else { '3' },
            0x05 => if self.shift_pressed { '$' } else { '4' },
            0x06 => if self.shift_pressed { '%' } else { '5' },
            0x07 => if self.shift_pressed { '^' } else { '6' },
            0x08 => if self.shift_pressed { '&' } else { '7' },
            0x09 => if self.shift_pressed { '*' } else { '8' },
            0x0A => if self.shift_pressed { '(' } else { '9' },
            0x0B => if self.shift_pressed { ')' } else { '0' },
            0x0C => if self.shift_pressed { '_' } else { '-' },
            0x0D => if self.shift_pressed { '+' } else { '=' },
            0x0E => '\x08', // Backspace
            0x0F => '\t',   // Tab

            // Fila QWERTY
            0x10 => if uppercase { 'Q' } else { 'q' },
            0x11 => if uppercase { 'W' } else { 'w' },
            0x12 => if uppercase { 'E' } else { 'e' },
            0x13 => if uppercase { 'R' } else { 'r' },
            0x14 => if uppercase { 'T' } else { 't' },
            0x15 => if uppercase { 'Y' } else { 'y' },
            0x16 => if uppercase { 'U' } else { 'u' },
            0x17 => if uppercase { 'I' } else { 'i' },
            0x18 => if uppercase { 'O' } else { 'o' },
            0x19 => if uppercase { 'P' } else { 'p' },
            0x1A => if self.shift_pressed { '{' } else { '[' },
            0x1B => if self.shift_pressed { '}' } else { ']' },
            0x1C => '\n', // Enter

            // Fila ASDF
            0x1E => if uppercase { 'A' } else { 'a' },
            0x1F => if uppercase { 'S' } else { 's' },
            0x20 => if uppercase { 'D' } else { 'd' },
            0x21 => if uppercase { 'F' } else { 'f' },
            0x22 => if uppercase { 'G' } else { 'g' },
            0x23 => if uppercase { 'H' } else { 'h' },
            0x24 => if uppercase { 'J' } else { 'j' },
            0x25 => if uppercase { 'K' } else { 'k' },
            0x26 => if uppercase { 'L' } else { 'l' },
            0x27 => if self.shift_pressed { ':' } else { ';' },
            0x28 => if self.shift_pressed { '"' } else { '\'' },

            // Fila ZXCV
            0x2C => if uppercase { 'Z' } else { 'z' },
            0x2D => if uppercase { 'X' } else { 'x' },
            0x2E => if uppercase { 'C' } else { 'c' },
            0x2F => if uppercase { 'V' } else { 'v' },
            0x30 => if uppercase { 'B' } else { 'b' },
            0x31 => if uppercase { 'N' } else { 'n' },
            0x32 => if uppercase { 'M' } else { 'm' },
            0x33 => if self.shift_pressed { '<' } else { ',' },
            0x34 => if self.shift_pressed { '>' } else { '.' },
            0x35 => if self.shift_pressed { '?' } else { '/' },

            0x39 => ' ', // Barra espaciadora

            _ => return None,
        };

        Some(ch)
    }
}

pub static KEYBOARD: SpinMutex<KeyboardState> = SpinMutex::new(KeyboardState::new());

pub fn on_keyboard_interrupt() -> Option<char> {
    let scancode = unsafe { inb(KEYBOARD_DATA_PORT) };
    KEYBOARD.lock().process_scancode(scancode)
}
