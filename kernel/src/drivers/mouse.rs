// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: PS/2 Mouse Driver with 3-byte Packet Decoding & Dynamic Coordinates
// ============================================================================

use crate::io::{inb, outb};
use crate::sync::SpinMutex;

pub const PS2_DATA_PORT: u16 = 0x60;
pub const PS2_STATUS_PORT: u16 = 0x64;
pub const PS2_COMMAND_PORT: u16 = 0x64;

#[derive(Copy, Clone, Debug)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
    pub click_count: u32,
}

impl MouseState {
    pub const fn new() -> Self {
        Self {
            x: 300,
            y: 200,
            left_button: false,
            right_button: false,
            middle_button: false,
            click_count: 0,
        }
    }
}

pub struct MouseDriver {
    pub state: MouseState,
    packet: [u8; 3],
    packet_index: u8,
    initialized: bool,
}

impl MouseDriver {
    pub const fn new() -> Self {
        Self {
            state: MouseState::new(),
            packet: [0; 3],
            packet_index: 0,
            initialized: false,
        }
    }

    pub unsafe fn init(&mut self) {
        if self.initialized {
            return;
        }

        // Esperar a que el buffer de entrada del 8042 esté libre
        wait_input_clear();
        // Habilitar dispositivo auxiliar (Ratón PS/2)
        outb(PS2_COMMAND_PORT, 0xA8);

        // Leer Byte de Configuración del Controlador
        wait_input_clear();
        outb(PS2_COMMAND_PORT, 0x20);
        wait_output_full();
        let mut status = inb(PS2_DATA_PORT);

        // Habilitar interrupción de ratón (IRQ 12, Bit 1) y habilitar reloj de ratón (Bit 5 = 0)
        status |= 0x02;
        status &= !0x20;

        // Escribir Byte de Configuración actualizado
        wait_input_clear();
        outb(PS2_COMMAND_PORT, 0x60);
        wait_input_clear();
        outb(PS2_DATA_PORT, status);

        // Enviar comando al ratón: Restaurar configuración por defecto (0xF6)
        self.send_to_mouse(0xF6);
        let _ = self.read_mouse_ack();

        // Enviar comando al ratón: Habilitar transmisión de paquetes (0xF4)
        self.send_to_mouse(0xF4);
        let _ = self.read_mouse_ack();

        // Desbloquear IRQ 12 en el PIC Slave
        crate::interrupts::pic::PICS.unmask_irq(12);

        self.initialized = true;
    }

    unsafe fn send_to_mouse(&self, cmd: u8) {
        wait_input_clear();
        outb(PS2_COMMAND_PORT, 0xD4); // Próximo byte se envía al ratón
        wait_input_clear();
        outb(PS2_DATA_PORT, cmd);
    }

    unsafe fn read_mouse_ack(&self) -> u8 {
        if wait_output_full() {
            inb(PS2_DATA_PORT)
        } else {
            0
        }
    }

    pub fn process_byte(&mut self, byte: u8) -> Option<MouseState> {
        match self.packet_index {
            0 => {
                // En el protocolo estándar PS/2, el bit 3 del byte de sincronización SIEMPRE es 1
                if (byte & 0x08) != 0 {
                    self.packet[0] = byte;
                    self.packet_index = 1;
                }
            }
            1 => {
                self.packet[1] = byte;
                self.packet_index = 2;
            }
            2 => {
                self.packet[2] = byte;
                self.packet_index = 0;

                let flags = self.packet[0];
                let raw_x = self.packet[1];
                let raw_y = self.packet[2];

                // Si ocurrió overflow en el sensor del ratón, descartar muestra
                if (flags & 0xC0) != 0 {
                    return None;
                }

                // Cálculo con signo para desplazamiento X
                let dx = if (flags & 0x10) != 0 {
                    (raw_x as i32) - 256
                } else {
                    raw_x as i32
                };

                // Cálculo con signo para desplazamiento Y (En PS/2 +Y es hacia arriba, en pantalla raster +Y es hacia abajo)
                let dy = if (flags & 0x20) != 0 {
                    (raw_y as i32) - 256
                } else {
                    raw_y as i32
                };

                let left = (flags & 0x01) != 0;
                let right = (flags & 0x02) != 0;
                let middle = (flags & 0x04) != 0;

                if left && !self.state.left_button {
                    self.state.click_count = self.state.click_count.wrapping_add(1);
                }

                self.state.left_button = left;
                self.state.right_button = right;
                self.state.middle_button = middle;

                // Límites de resolución VESA 1024x768
                self.state.x = (self.state.x + dx).clamp(0, 1023);
                self.state.y = (self.state.y - dy).clamp(0, 767);

                return Some(self.state);
            }
            _ => {
                self.packet_index = 0;
            }
        }
        None
    }
}

fn wait_input_clear() {
    let mut timeout = 100_000;
    while timeout > 0 {
        if (unsafe { inb(PS2_STATUS_PORT) } & 0x02) == 0 {
            return;
        }
        timeout -= 1;
    }
}

fn wait_output_full() -> bool {
    let mut timeout = 100_000;
    while timeout > 0 {
        if (unsafe { inb(PS2_STATUS_PORT) } & 0x01) != 0 {
            return true;
        }
        timeout -= 1;
    }
    false
}

pub static MOUSE: SpinMutex<MouseDriver> = SpinMutex::new(MouseDriver::new());

pub fn init_mouse() {
    unsafe {
        MOUSE.lock().init();
    }
}
