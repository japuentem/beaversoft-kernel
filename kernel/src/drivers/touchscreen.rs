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
    pub hw_present: bool,
    pub is_initialized: bool,
    pub probe_status: &'static str,
    pub width: u16,
    pub height: u16,
    pub touch_count: u8,
    pub active_touches: [Option<TouchPoint>; 5],
    pub total_events: u64,
}

impl TouchscreenState {
    pub const fn new() -> Self {
        Self {
            hw_present: false,
            is_initialized: false,
            probe_status: "Sin sondear",
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

    /// Registra una trama de coordenadas y devuelve `(screen_x, screen_y)` si debe despacharse un toque
    pub fn record_raw_touch(
        &mut self,
        id: u8,
        raw_x: u16,
        raw_y: u16,
        max_x: u16,
        max_y: u16,
        is_down: bool,
    ) -> Option<(usize, usize)> {
        if max_x == 0 || max_y == 0 {
            return None;
        }

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

        if is_down && phase == TouchPhase::Down {
            Some((screen_x as usize, screen_y as usize))
        } else {
            None
        }
    }
}

pub static TOUCHSCREEN: SpinMutex<TouchscreenState> = SpinMutex::new(TouchscreenState::new());

#[cfg(target_arch = "x86_64")]
fn probe_touchscreen_hardware() -> (bool, &'static str) {
    // 1. Sondear bus PCI por dispositivo Input Digitizer (Clase 0x09, Subclase 0x02)
    if crate::drivers::pci::find_device_by_class(0x09, Some(0x02)).is_some() {
        return (true, "Digitalizador táctil PCI (0x09:0x02) detectado");
    }

    // 2. Verificar dispositivos USB HID táctiles conocidos (Elan, SiS, Ilitek, eGalax, Goodix)
    if let Some(usb) = crate::drivers::usb::USB_MANAGER.try_lock() {
        const TOUCH_VENDORS: [u16; 5] = [0x04F3, 0x0457, 0x222A, 0x0EEF, 0x27C6];
        if usb.devices.iter().any(|d| TOUCH_VENDORS.contains(&d.vendor_id)) {
            return (true, "Panel táctil USB HID detectado");
        }
    }

    // 3. Sondear controlador I2C/Serial IO PCI (0x0C:0x80) para paneles GT911 / FT6236
    if crate::drivers::pci::find_device_by_class(0x0C, Some(0x80)).is_some() {
        return (false, "Controlador I2C PCI sondeado: sin panel GT911/FT6236");
    }

    (false, "Buses PCI/USB sondeados: sin digitalizador táctil")
}

#[cfg(target_arch = "aarch64")]
fn probe_touchscreen_hardware() -> (bool, &'static str) {
    (false, "Plataforma virt ARM64 sin digitalizador I2C/USB")
}

/// Inicializa el subsistema del panel táctil capacitivo
pub fn init_touchscreen() {
    let (present, status) = probe_touchscreen_hardware();
    let mut ts = TOUCHSCREEN.lock();
    ts.hw_present = present;
    ts.probe_status = status;
    ts.calibrate(1024, 768);
}

/// Procesa un toque táctil liberando primero el lock del driver antes de notificar a la GUI
pub fn inject_touch_event(x: u16, y: u16, is_down: bool) {
    let coords = {
        let mut ts = TOUCHSCREEN.lock();
        ts.record_raw_touch(0, x, y, 1024, 768, is_down)
    };

    if let Some((sx, sy)) = coords {
        crate::gui::smart_hub::handle_click(sx, sy);
    }
}
