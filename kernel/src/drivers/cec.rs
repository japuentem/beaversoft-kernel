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
pub const CEC_OP_IMAGE_VIEW_ON: u8          = 0x04;
pub const CEC_OP_ACTIVE_SOURCE: u8           = 0x82;
pub const CEC_OP_REQUEST_ACTIVE_SOURCE: u8   = 0x85;
pub const CEC_OP_SET_STREAM_PATH: u8         = 0x86;
pub const CEC_OP_USER_CONTROL_PRESSED: u8    = 0x44;
pub const CEC_OP_USER_CONTROL_RELEASED: u8   = 0x45;
pub const CEC_OP_STANDBY: u8                 = 0x36;

/// Operandos para el código de operación <User Control Pressed> (0x44)
pub const CEC_USER_SELECT: u8       = 0x00;
pub const CEC_USER_UP: u8           = 0x01;
pub const CEC_USER_DOWN: u8         = 0x02;
pub const CEC_USER_LEFT: u8         = 0x03;
pub const CEC_USER_RIGHT: u8        = 0x04;
pub const CEC_USER_ROOT_MENU: u8    = 0x09;
pub const CEC_USER_EXIT: u8         = 0x0D;
pub const CEC_USER_PLAY: u8         = 0x44;
pub const CEC_USER_STOP: u8         = 0x45;
pub const CEC_USER_PAUSE: u8        = 0x46;
pub const CEC_USER_PLAY_PAUSE: u8   = 0x60;
pub const CEC_USER_VOL_UP: u8       = 0x41;
pub const CEC_USER_VOL_DOWN: u8     = 0x42;

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
    pub hw_present: bool,
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
            hw_present: false,
            is_online: false,
            logical_address: CEC_ADDR_PLAYBACK1,
            tv_is_on: false,
            last_button_pressed: None,
            frames_received: 0,
            frames_sent: 0,
        }
    }

    /// Inicializa el decodificador de tramas HDMI-CEC y sondea presencia física
    pub fn init(&mut self) {
        self.hw_present = false; // En QEMU estándar x86_64/virt no hay puente CEC físico por defecto
        self.is_online = true;
        self.logical_address = CEC_ADDR_PLAYBACK1;
        self.tv_is_on = false;
    }

    /// Procesa una trama entrante y devuelve la tecla remota si corresponde (sin anidar locks)
    pub fn record_frame(&mut self, frame: &CecFrame) -> Option<u8> {
        self.frames_received = self.frames_received.wrapping_add(1);

        match frame.opcode {
            CEC_OP_USER_CONTROL_PRESSED => {
                if frame.operand_count > 0 {
                    let key = frame.operands[0];
                    self.last_button_pressed = Some(key);
                    self.tv_is_on = true;
                    return Some(key);
                }
            }
            CEC_OP_USER_CONTROL_RELEASED => {
                self.last_button_pressed = None;
            }
            CEC_OP_REQUEST_ACTIVE_SOURCE => {
                self.announce_active_source();
            }
            CEC_OP_STANDBY => {
                self.tv_is_on = false;
            }
            _ => {}
        }
        None
    }

    /// Registra el envío de trama <Active Source>
    pub fn announce_active_source(&mut self) {
        self.frames_sent = self.frames_sent.wrapping_add(1);
    }
}

pub static HDMI_CEC: SpinMutex<HdmiCecController> = SpinMutex::new(HdmiCecController::new());

/// Inicializa el subsistema HDMI-CEC
pub fn init_cec() {
    let mut cec = HDMI_CEC.lock();
    cec.init();
}

/// Despacha la tecla del control remoto fuera del lock de `HDMI_CEC` para evitar inversión/deadlock
fn dispatch_remote_key_unlocked(key: u8) {
    match key {
        CEC_USER_UP => crate::interrupts::push_input_char(KEY_UP_ARROW),
        CEC_USER_DOWN => crate::interrupts::push_input_char(KEY_DOWN_ARROW),
        CEC_USER_LEFT => crate::interrupts::push_input_char(KEY_LEFT_ARROW),
        CEC_USER_RIGHT => crate::interrupts::push_input_char(KEY_RIGHT_ARROW),
        CEC_USER_SELECT => crate::interrupts::push_input_char('\n'),
        CEC_USER_EXIT => crate::interrupts::push_input_char('\x1B'),
        CEC_USER_ROOT_MENU => crate::interrupts::push_input_char('\t'),
        CEC_USER_PLAY | CEC_USER_PLAY_PAUSE => {
            crate::gui::smart_hub::focus_tile(1);
            crate::gui::smart_hub::activate_focused_tile();
        }
        _ => {}
    }
}

/// Procesa una pulsación de botón CEC liberando el lock antes de invocar subsistemas externos
pub fn inject_cec_key(key: u8) {
    let mut frame = CecFrame::empty();
    frame.initiator = CEC_ADDR_TV;
    frame.destination = CEC_ADDR_PLAYBACK1;
    frame.opcode = CEC_OP_USER_CONTROL_PRESSED;
    frame.operands[0] = key;
    frame.operand_count = 1;

    let maybe_key = {
        let mut cec = HDMI_CEC.lock();
        cec.record_frame(&frame)
    };

    if let Some(k) = maybe_key {
        dispatch_remote_key_unlocked(k);
    }
}
