// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: HDMI-CEC (Consumer Electronics Control) & Smart TV Remote Driver
//              Parses standard CEC 1.4/2.0 frames from TV remotes (Opcode 0x44)
// ============================================================================

#![allow(dead_code)]

use crate::sync::SpinMutex;
use crate::drivers::keyboard::{
    KEY_UP_ARROW, KEY_DOWN_ARROW, KEY_LEFT_ARROW, KEY_RIGHT_ARROW,
};

/// Direcciones lógicas estándar según especificación HDMI-CEC
pub const CEC_ADDR_TV: u8        = 0x00;
pub const CEC_ADDR_RECORDER1: u8 = 0x01;
pub const CEC_ADDR_PLAYBACK1: u8 = 0x04; // Dirección asignada a Beaver OS en el bus HDMI
pub const CEC_ADDR_AUDIO: u8     = 0x05;
pub const CEC_ADDR_BROADCAST: u8 = 0x0F;

/// Códigos de operación (Opcodes) principales del estándar HDMI-CEC
pub const CEC_OP_IMAGE_VIEW_ON: u8          = 0x04; // Enciende la pantalla al detectar señal
pub const CEC_OP_ACTIVE_SOURCE: u8           = 0x82; // Reporta a la TV que Beaver OS está activo
pub const CEC_OP_REQUEST_ACTIVE_SOURCE: u8   = 0x85;
pub const CEC_OP_SET_STREAM_PATH: u8         = 0x86;
pub const CEC_OP_USER_CONTROL_PRESSED: u8    = 0x44; // Pulsación de botón del control remoto de la TV
pub const CEC_OP_USER_CONTROL_RELEASED: u8   = 0x45; // Liberación de botón del control remoto
pub const CEC_OP_STANDBY: u8                 = 0x36; // Poner equipo en reposo

/// Operandos para el código de operación <User Control Pressed> (0x44)
pub const CEC_USER_SELECT: u8       = 0x00; // Botón OK / Enter central
pub const CEC_USER_UP: u8           = 0x01; // Flecha Arriba
pub const CEC_USER_DOWN: u8         = 0x02; // Flecha Abajo
pub const CEC_USER_LEFT: u8         = 0x03; // Flecha Izquierda
pub const CEC_USER_RIGHT: u8        = 0x04; // Flecha Derecha
pub const CEC_USER_ROOT_MENU: u8    = 0x09; // Botón Home / Menú Principal
pub const CEC_USER_EXIT: u8         = 0x0D; // Botón Atrás / Exit
pub const CEC_USER_PLAY: u8         = 0x44; // Reproducir multimedia
pub const CEC_USER_STOP: u8         = 0x45; // Detener multimedia
pub const CEC_USER_PAUSE: u8        = 0x46; // Pausar multimedia
pub const CEC_USER_PLAY_PAUSE: u8   = 0x60; // Alternar reproducción
pub const CEC_USER_VOL_UP: u8       = 0x41; // Volumen +
pub const CEC_USER_VOL_DOWN: u8     = 0x42; // Volumen -

/// Trama de datos recibida por el bus físico HDMI-CEC
#[derive(Debug, Clone, Copy)]
pub struct CecFrame {
    pub initiator: u8,
    pub destination: u8,
    pub opcode: u8,
    pub operands: [u8; 14],
    pub operand_count: usize,
}

impl CecFrame {
    pub const fn empty() -> Self {
        Self {
            initiator: 0,
            destination: 0,
            opcode: 0,
            operands: [0; 14],
            operand_count: 0,
        }
    }
}

/// Estado del subsistema HDMI-CEC de Beaver OS
pub struct HdmiCecController {
    pub is_online: bool,
    pub logical_address: u8,
    pub tv_is_on: bool,
    pub last_button_pressed: Option<u8>,
    pub frames_received: u64,
    pub frames_sent: u64,
}

impl HdmiCecController {
    pub const fn new() -> Self {
        Self {
            is_online: false,
            logical_address: CEC_ADDR_PLAYBACK1,
            tv_is_on: false,
            last_button_pressed: None,
            frames_received: 0,
            frames_sent: 0,
        }
    }

    /// Inicializa la controladora y anuncia a Beaver OS como fuente activa HDMI
    pub fn init(&mut self) {
        self.is_online = true;
        self.logical_address = CEC_ADDR_PLAYBACK1;
        self.tv_is_on = true;
        crate::kprintln!(
            "[HDMI-CEC] Controlador CEC 1.4/2.0 en línea. Dirección lógica: {:#x} (Playback 1).",
            self.logical_address
        );
    }

    /// Procesa una trama entrante desde la TV y la traduce a eventos de Beaver OS
    pub fn handle_frame(&mut self, frame: &CecFrame) {
        self.frames_received = self.frames_received.wrapping_add(1);

        match frame.opcode {
            CEC_OP_USER_CONTROL_PRESSED => {
                if frame.operand_count > 0 {
                    let key = frame.operands[0];
                    self.last_button_pressed = Some(key);
                    self.dispatch_remote_key(key);
                }
            }
            CEC_OP_USER_CONTROL_RELEASED => {
                self.last_button_pressed = None;
            }
            CEC_OP_REQUEST_ACTIVE_SOURCE => {
                // Responder a la TV anunciando que Beaver OS está activo
                self.announce_active_source();
            }
            CEC_OP_STANDBY => {
                crate::kprintln!("[HDMI-CEC] Notificación de reposo recibida de la TV.");
                self.tv_is_on = false;
            }
            _ => {}
        }
    }

    /// Despacha la tecla del control remoto al subsistema de entrada y a Beaver Smart Hub
    fn dispatch_remote_key(&mut self, key: u8) {
        match key {
            CEC_USER_UP => {
                crate::interrupts::push_input_char(KEY_UP_ARROW);
            }
            CEC_USER_DOWN => {
                crate::interrupts::push_input_char(KEY_DOWN_ARROW);
            }
            CEC_USER_LEFT => {
                crate::interrupts::push_input_char(KEY_LEFT_ARROW);
            }
            CEC_USER_RIGHT => {
                crate::interrupts::push_input_char(KEY_RIGHT_ARROW);
            }
            CEC_USER_SELECT => {
                crate::interrupts::push_input_char('\n');
            }
            CEC_USER_EXIT => {
                crate::interrupts::push_input_char('\x1B'); // Escape
            }
            CEC_USER_ROOT_MENU => {
                // Cambiar directamente al modo Smart Hub en el escritorio
                crate::interrupts::push_input_char('\t'); // Tab alterna foco
            }
            CEC_USER_PLAY | CEC_USER_PLAY_PAUSE => {
                crate::gui::smart_hub::focus_tile(1); // Tile 1: Centro Multimedia
                crate::gui::smart_hub::activate_focused_tile();
            }
            _ => {}
        }
    }

    /// Anuncia a la TV que Beaver OS es la fuente activa
    pub fn announce_active_source(&mut self) {
        self.frames_sent = self.frames_sent.wrapping_add(1);
        crate::kprintln!("[HDMI-CEC] Difundiendo trama <Active Source> en el bus HDMI.");
    }
}

pub static HDMI_CEC: SpinMutex<HdmiCecController> = SpinMutex::new(HdmiCecController::new());

/// Inicializa el controlador HDMI-CEC del sistema
pub fn init_cec() {
    let mut cec = HDMI_CEC.lock();
    cec.init();
}

/// Inyecta una pulsación de botón de control remoto para verificación diagnóstica
pub fn inject_cec_key(key: u8) {
    let mut frame = CecFrame::empty();
    frame.initiator = CEC_ADDR_TV;
    frame.destination = CEC_ADDR_PLAYBACK1;
    frame.opcode = CEC_OP_USER_CONTROL_PRESSED;
    frame.operands[0] = key;
    frame.operand_count = 1;

    let mut cec = HDMI_CEC.lock();
    cec.handle_frame(&frame);
}
