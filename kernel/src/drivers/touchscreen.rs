// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Capacitive Touchscreen Controller Driver (I2C Goodix GT911 / FocalTech FT6236)
//              Translates hardware digitizer packets into Beaver OS GUI Touch Events
// ============================================================================

#![allow(dead_code)]

use crate::sync::SpinMutex;

/// Estado de un punto de contacto táctil
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Down,
    Moved,
    Up,
}

/// Punto de contacto capacitivo en coordenadas absolutas de pantalla
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    pub id: u8,
    pub x: u16,
    pub y: u16,
    pub pressure: u8,
    pub phase: TouchPhase,
}

/// Estado global del digitalizador táctil
pub struct TouchscreenState {
    pub is_initialized: bool,
    pub width: u16,
    pub height: u16,
    pub touch_count: u8,
    pub active_touches: [Option<TouchPoint>; 5],
    pub total_events: u64,
}

impl TouchscreenState {
    pub const fn new() -> Self {
        Self {
            is_initialized: false,
            width: 1024,
            height: 768,
            touch_count: 0,
            active_touches: [None, None, None, None, None],
            total_events: 0,
        }
    }

    /// Calibra la resolución virtual del sensor táctil con el Framebuffer activo
    pub fn calibrate(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.is_initialized = true;
    }

    /// Procesa una trama de coordenadas del sensor y despacha el evento al Smart Hub
    pub fn process_raw_touch(&mut self, id: u8, raw_x: u16, raw_y: u16, max_x: u16, max_y: u16, is_down: bool) {
        if max_x == 0 || max_y == 0 {
            return;
        }

        // Normalizar coordenadas a la resolución del monitor/pantalla
        let screen_x = ((raw_x as u32 * self.width as u32) / max_x as u32) as u16;
        let screen_y = ((raw_y as u32 * self.height as u32) / max_y as u32) as u16;

        let slot = (id as usize) % 5;
        let prev = self.active_touches[slot];

        let phase = if is_down {
            match prev {
                Some(_) => TouchPhase::Moved,
                None => TouchPhase::Down,
            }
        } else {
            TouchPhase::Up
        };

        let point = TouchPoint {
            id,
            x: screen_x,
            y: screen_y,
            pressure: if is_down { 128 } else { 0 },
            phase,
        };

        if is_down {
            self.active_touches[slot] = Some(point);
        } else {
            self.active_touches[slot] = None;
        }

        self.total_events = self.total_events.wrapping_add(1);

        // Despachar el evento táctil directamente a la interfaz Beaver Smart Hub
        crate::gui::smart_hub::handle_click(screen_x as usize, screen_y as usize);
    }
}

pub static TOUCHSCREEN: SpinMutex<TouchscreenState> = SpinMutex::new(TouchscreenState::new());

/// Inicializa el controlador del panel táctil capacitivo
pub fn init_touchscreen() {
    let mut ts = TOUCHSCREEN.lock();
    ts.calibrate(1024, 768);
    crate::kprintln!("[TOUCH] Controlador de Digitalizador Táctil Capacitivo registrado (1024x768).");
}

/// Inyecta un toque simulado por hardware para verificación diagnóstica
pub fn inject_touch_event(x: u16, y: u16, is_down: bool) {
    let mut ts = TOUCHSCREEN.lock();
    ts.process_raw_touch(0, x, y, 1024, 768, is_down);
}
