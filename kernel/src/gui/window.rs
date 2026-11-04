// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Multi-Window Manager with Dynamic Drag & Drop, Z-Order, Minimize & Maximize
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use crate::sync::SpinMutex;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WindowId {
    Monitor = 0,
    Terminal = 1,
    Files = 2,
    Note = 3,
    Player = 4,
    Sync = 5,
    Photos = 6,
    Calendar = 7,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WindowButton {
    Close,
    Minimize,
    Maximize,
}

#[derive(Clone, Debug)]
pub struct Window {
    pub id: WindowId,
    pub title: &'static str,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub minimized: bool,
    pub maximized: bool,
    pub restore_x: usize,
    pub restore_y: usize,
    pub restore_w: usize,
    pub restore_h: usize,
}

pub struct WindowManager {
    pub windows: Vec<Window>,
    pub z_order: [WindowId; 8], // Índice 0 = fondo, Índice 7 = frente (activa)
    pub dragging_window: Option<WindowId>,
    pub drag_offset_x: usize,
    pub drag_offset_y: usize,
}

impl WindowManager {
    pub const fn new() -> Self {
        Self {
            windows: Vec::new(),
            z_order: [WindowId::Photos, WindowId::Calendar, WindowId::Sync, WindowId::Note, WindowId::Monitor, WindowId::Files, WindowId::Player, WindowId::Terminal],
            dragging_window: None,
            drag_offset_x: 0,
            drag_offset_y: 0,
        }
    }

    pub fn init_defaults(&mut self) {
        if self.windows.is_empty() {
            self.windows.push(Window {
                id: WindowId::Monitor,
                title: "Monitor de Recursos en Vivo (Transparente)",
                x: 20,
                y: 44,
                width: 450,
                height: 320,
                minimized: false,
                maximized: false,
                restore_x: 20,
                restore_y: 44,
                restore_w: 450,
                restore_h: 320,
            });
            self.windows.push(Window {
                id: WindowId::Terminal,
                title: "Terminal Interactivo Beaver (Ring 3 Ready)",
                x: 490,
                y: 44,
                width: 510,
                height: 320,
                minimized: false,
                maximized: false,
                restore_x: 490,
                restore_y: 44,
                restore_w: 510,
                restore_h: 320,
            });
            self.windows.push(Window {
                id: WindowId::Files,
                title: "BeaverFiles - Explorador VFS y Beaver Mesh",
                x: 20,
                y: 380,
                width: 615,
                height: 260,
                minimized: false,
                maximized: false,
                restore_x: 20,
                restore_y: 380,
                restore_w: 615,
                restore_h: 260,
            });
            self.windows.push(Window {
                id: WindowId::Player,
                title: "BeaverAudio - Reproductor Multimedia",
                x: 650,
                y: 380,
                width: 350,
                height: 260,
                minimized: false,
                maximized: false,
                restore_x: 650,
                restore_y: 380,
                restore_w: 350,
                restore_h: 260,
            });
            self.windows.push(Window {
                id: WindowId::Note,
                title: "BeaverNote v1.0 - Bloc de Notas Persistente (/disk)",
                x: 180,
                y: 90,
                width: 620,
                height: 330,
                minimized: true,
                maximized: false,
                restore_x: 180,
                restore_y: 90,
                restore_w: 620,
                restore_h: 330,
            });
            self.windows.push(Window {
                id: WindowId::Sync,
                title: "BeaverSync - Sincronizacion Universal de Dispositivos (Malla Genisys)",
                x: 140,
                y: 70,
                width: 740,
                height: 380,
                minimized: true,
                maximized: false,
                restore_x: 140,
                restore_y: 70,
                restore_w: 740,
                restore_h: 380,
            });
            self.windows.push(Window {
                id: WindowId::Photos,
                title: "BeaverPhotos - Galeria y Visor de Imagenes",
                x: 210,
                y: 70,
                width: 600,
                height: 380,
                minimized: true,
                maximized: false,
                restore_x: 210,
                restore_y: 70,
                restore_w: 600,
                restore_h: 380,
            });
            self.windows.push(Window {
                id: WindowId::Calendar,
                title: "BeaverCalendar - Agenda y Calendario RTC",
                x: 260,
                y: 80,
                width: 480,
                height: 370,
                minimized: true,
                maximized: false,
                restore_x: 260,
                restore_y: 80,
                restore_w: 480,
                restore_h: 370,
            });
            self.z_order = [WindowId::Photos, WindowId::Calendar, WindowId::Sync, WindowId::Note, WindowId::Monitor, WindowId::Files, WindowId::Player, WindowId::Terminal];
        }
    }

    pub fn get_window(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    pub fn get_window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    pub fn is_minimized(&self, id: WindowId) -> bool {
        self.get_window(id).map_or(false, |w| w.minimized)
    }

    pub fn minimize(&mut self, id: WindowId) {
        if let Some(win) = self.get_window_mut(id) {
            win.minimized = true;
        }
    }

    pub fn restore_or_maximize(&mut self, id: WindowId, screen_w: usize, screen_h: usize) {
        if let Some(win) = self.get_window_mut(id) {
            if win.maximized {
                win.x = win.restore_x;
                win.y = win.restore_y;
                win.width = win.restore_w;
                win.height = win.restore_h;
                win.maximized = false;
            } else {
                win.restore_x = win.x;
                win.restore_y = win.y;
                win.restore_w = win.width;
                win.restore_h = win.height;
                win.x = 10;
                win.y = 34;
                win.width = screen_w.saturating_sub(20);
                win.height = screen_h.saturating_sub(96);
                win.maximized = true;
            }
            win.minimized = false;
        }
        self.bring_to_front(id);
    }

    pub fn unminimize_and_focus(&mut self, id: WindowId) {
        if let Some(win) = self.get_window_mut(id) {
            win.minimized = false;
        }
        self.bring_to_front(id);
    }

    /// Comprueba si el clic impactó alguno de los 3 semáforos circulares de cabecera (Cerrar, Minimizar, Maximizar)
    pub fn hit_test_buttons(&self, mx: usize, my: usize) -> Option<(WindowId, WindowButton)> {
        for &id in self.z_order.iter().rev() {
            if let Some(win) = self.get_window(id) {
                if !win.minimized {
                    if my >= win.y + 6 && my <= win.y + 24 {
                        // Semáforos circulares estilo Lord OS / Mac en la izquierda:
                        // Botón Rojo (Cerrar / Ocultar): win.x + 8 .. win.x + 24 (centro x+16)
                        if mx >= win.x + 8 && mx <= win.x + 24 {
                            return Some((id, WindowButton::Close));
                        }
                        // Botón Amarillo (Minimizar): win.x + 25 .. win.x + 40 (centro x+32)
                        if mx >= win.x + 25 && mx <= win.x + 40 {
                            return Some((id, WindowButton::Minimize));
                        }
                        // Botón Verde (Maximizar): win.x + 41 .. win.x + 56 (centro x+48)
                        if mx >= win.x + 41 && mx <= win.x + 56 {
                            return Some((id, WindowButton::Maximize));
                        }

                        // Botones a la derecha (compatibilidad):
                        if mx >= win.x + win.width.saturating_sub(24) && mx <= win.x + win.width.saturating_sub(6) {
                            return Some((id, WindowButton::Close));
                        }
                        if mx >= win.x + win.width.saturating_sub(40) && mx <= win.x + win.width.saturating_sub(24) {
                            return Some((id, WindowButton::Minimize));
                        }
                        if mx >= win.x + win.width.saturating_sub(56) && mx <= win.x + win.width.saturating_sub(40) {
                            return Some((id, WindowButton::Maximize));
                        }
                    }
                }
            }
        }
        None
    }

    /// Trae la ventana seleccionada al frente en el orden Z
    pub fn bring_to_front(&mut self, id: WindowId) {
        let mut new_order = [WindowId::Monitor; 8];
        let mut idx = 0;
        for &w in &self.z_order {
            if w != id {
                new_order[idx] = w;
                idx += 1;
            }
        }
        new_order[7] = id;
        self.z_order = new_order;
    }

    pub fn is_active(&self, id: WindowId) -> bool {
        self.z_order[7] == id
    }

    /// Comprueba si las coordenadas (mx, my) caen en la barra de título de una ventana
    pub fn hit_test_titlebar(&self, mx: usize, my: usize) -> Option<WindowId> {
        // Evaluar desde la ventana más al frente hacia el fondo
        for &id in self.z_order.iter().rev() {
            if let Some(win) = self.get_window(id) {
                if !win.minimized
                    && mx >= win.x
                    && mx <= win.x + win.width
                    && my >= win.y
                    && my <= win.y + 28
                {
                    return Some(id);
                }
            }
        }
        None
    }

    /// Comprueba si las coordenadas (mx, my) caen dentro del cuerpo de una ventana
    pub fn hit_test_window(&self, mx: usize, my: usize) -> Option<WindowId> {
        for &id in self.z_order.iter().rev() {
            if let Some(win) = self.get_window(id) {
                if !win.minimized
                    && mx >= win.x
                    && mx <= win.x + win.width
                    && my >= win.y
                    && my <= win.y + win.height
                {
                    return Some(id);
                }
            }
        }
        None
    }

    /// Inicia el arrastre de la ventana
    pub fn start_drag(&mut self, id: WindowId, mx: usize, my: usize) {
        self.bring_to_front(id);
        let win_coords = self.get_window(id).map(|w| (w.x, w.y));
        if let Some((win_x, win_y)) = win_coords {
            self.dragging_window = Some(id);
            self.drag_offset_x = mx.saturating_sub(win_x);
            self.drag_offset_y = my.saturating_sub(win_y);
        }
    }

    /// Actualiza la posición de la ventana mientras se arrastra
    pub fn update_drag(&mut self, mx: usize, my: usize) -> bool {
        if let Some(id) = self.dragging_window {
            let offset_x = self.drag_offset_x;
            let offset_y = self.drag_offset_y;
            let (scr_w, scr_h) = crate::gui::canvas::Canvas::screen_dimensions();
            if let Some(win) = self.get_window_mut(id) {
                let max_x = scr_w.saturating_sub(win.width);
                let max_y = scr_h.saturating_sub(60 + win.height).max(32);
                let new_x = mx.saturating_sub(offset_x).clamp(0, max_x);
                let new_y = my.saturating_sub(offset_y).clamp(32, max_y);
                if win.x != new_x || win.y != new_y {
                    win.x = new_x;
                    win.y = new_y;
                    return true;
                }
            }
        }
        false
    }

    /// Finaliza el arrastre
    pub fn stop_drag(&mut self) {
        self.dragging_window = None;
    }
}

pub static WINDOW_MGR: SpinMutex<WindowManager> = SpinMutex::new(WindowManager::new());
