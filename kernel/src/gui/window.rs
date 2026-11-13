// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Multi-Window Manager with Dynamic Drag & Drop, Z-Order, Minimize & Maximize
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use crate::sync::SpinMutex;

pub const GUI_EVENT_CLICK: u32 = 1;
pub const GUI_EVENT_KEY: u32 = 2;
pub const GUI_EVENT_CLOSE: u32 = 3;
pub const GUI_EVENT_FOCUS: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiEvent {
    pub event_type: u32, // 1 = Click, 2 = KeyPress, 3 = Close, 4 = Focus
    pub param1: u32,     // x (relativo al contenido) o código de tecla
    pub param2: u32,     // y (relativo al contenido) o modificadores
    pub param3: u32,     // botón (1 = Izquierdo, 2 = Derecho)
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum UserWindowHit {
    CloseButton,
    TitleBar,
    ClientArea(usize, usize),
}

#[derive(Clone, Debug)]
pub struct UserWindow {
    pub id: u32,
    pub owner_tid: u64,
    pub title: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub client_w: usize,
    pub client_h: usize,
    pub minimized: bool,
    pub maximized: bool,
    pub restore_x: usize,
    pub restore_y: usize,
    pub restore_w: usize,
    pub restore_h: usize,
    pub buffer: Vec<u32>,
    pub event_queue: Vec<GuiEvent>,
}

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
    Calculator = 8,
    About = 9,
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
    pub z_order: [WindowId; 10], // Índice 0 = fondo, Índice 9 = frente (activa)
    pub dragging_window: Option<WindowId>,
    pub drag_offset_x: usize,
    pub drag_offset_y: usize,
    pub user_windows: Vec<UserWindow>,
    pub next_user_win_id: u32,
    pub active_user_win: Option<u32>,
    pub dragging_user_window: Option<u32>,
}

impl WindowManager {
    pub const fn new() -> Self {
        Self {
            windows: Vec::new(),
            z_order: [
                WindowId::About,
                WindowId::Calculator,
                WindowId::Photos,
                WindowId::Calendar,
                WindowId::Sync,
                WindowId::Note,
                WindowId::Monitor,
                WindowId::Files,
                WindowId::Player,
                WindowId::Terminal,
            ],
            dragging_window: None,
            drag_offset_x: 0,
            drag_offset_y: 0,
            user_windows: Vec::new(),
            next_user_win_id: 1000,
            active_user_win: None,
            dragging_user_window: None,
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
                minimized: true,
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
                minimized: true,
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
                minimized: true,
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
                minimized: true,
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
            self.windows.push(Window {
                id: WindowId::Calculator,
                title: "Calculadora - Beaver OS",
                x: 370,
                y: 110,
                width: 270,
                height: 360,
                minimized: true,
                maximized: false,
                restore_x: 370,
                restore_y: 110,
                restore_w: 270,
                restore_h: 360,
            });
            self.windows.push(Window {
                id: WindowId::About,
                title: "Acerca de Beaver OS",
                x: 270,
                y: 120,
                width: 480,
                height: 320,
                minimized: true,
                maximized: false,
                restore_x: 270,
                restore_y: 120,
                restore_w: 480,
                restore_h: 320,
            });
            self.z_order = [
                WindowId::About,
                WindowId::Calculator,
                WindowId::Photos,
                WindowId::Calendar,
                WindowId::Sync,
                WindowId::Note,
                WindowId::Monitor,
                WindowId::Files,
                WindowId::Player,
                WindowId::Terminal,
            ];
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
        let mut new_order = [WindowId::Monitor; 10];
        let mut idx = 0;
        for &w in &self.z_order {
            if w != id {
                new_order[idx] = w;
                idx += 1;
            }
        }
        new_order[9] = id;
        self.z_order = new_order;
    }

    pub fn is_active(&self, id: WindowId) -> bool {
        self.z_order[9] == id
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
        } else if let Some(id) = self.dragging_user_window {
            let offset_x = self.drag_offset_x;
            let offset_y = self.drag_offset_y;
            let (scr_w, scr_h) = crate::gui::canvas::Canvas::screen_dimensions();
            if let Some(win) = self.get_user_window_mut(id) {
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
        self.dragging_user_window = None;
    }

    // ────────────────────────────────────────────────────────────────────────
    // Gestión y Despacho de Ventanas de Usuario (Desktop Compositor IPC)
    // ────────────────────────────────────────────────────────────────────────

    pub fn create_user_window(
        &mut self,
        owner_tid: u64,
        title: &str,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> Result<u32, &'static str> {
        if self.user_windows.len() >= 16 {
            return Err("Límite de ventanas de usuario alcanzado (máx. 16)");
        }
        let id = self.next_user_win_id;
        self.next_user_win_id += 1;

        let w = width.clamp(120, 1024);
        let h = height.clamp(80, 768);
        let client_w = w.saturating_sub(4);
        let client_h = h.saturating_sub(28);

        let uwin = UserWindow {
            id,
            owner_tid,
            title: String::from(title),
            x,
            y,
            width: w,
            height: h,
            client_w,
            client_h,
            minimized: false,
            maximized: false,
            restore_x: x,
            restore_y: y,
            restore_w: w,
            restore_h: h,
            buffer: alloc::vec![0xFFFFFFFF_u32; client_w * client_h],
            event_queue: Vec::new(),
        };

        self.user_windows.push(uwin);
        self.active_user_win = Some(id);
        Ok(id)
    }

    pub fn destroy_user_window(&mut self, id: u32) -> Result<(), &'static str> {
        let before = self.user_windows.len();
        self.user_windows.retain(|w| w.id != id);
        if self.user_windows.len() < before {
            if self.active_user_win == Some(id) {
                self.active_user_win = self.user_windows.last().map(|w| w.id);
            }
            Ok(())
        } else {
            Err("Ventana de usuario no encontrada")
        }
    }

    pub fn get_user_window(&self, id: u32) -> Option<&UserWindow> {
        self.user_windows.iter().find(|w| w.id == id)
    }

    pub fn get_user_window_mut(&mut self, id: u32) -> Option<&mut UserWindow> {
        self.user_windows.iter_mut().find(|w| w.id == id)
    }

    pub fn draw_rect_user_window(
        &mut self,
        id: u32,
        rx: usize,
        ry: usize,
        rw: usize,
        rh: usize,
        color: u32,
    ) -> Result<(), &'static str> {
        let win = self.get_user_window_mut(id).ok_or("Ventana no encontrada")?;
        let cw = win.client_w;
        let ch = win.client_h;
        for row in ry..(ry + rh).min(ch) {
            let row_offset = row * cw;
            for col in rx..(rx + rw).min(cw) {
                win.buffer[row_offset + col] = color;
            }
        }
        Ok(())
    }

    pub fn draw_text_user_window(
        &mut self,
        id: u32,
        tx: usize,
        ty: usize,
        text: &str,
        color: u32,
    ) -> Result<(), &'static str> {
        let win = self.get_user_window_mut(id).ok_or("Ventana no encontrada")?;
        let cw = win.client_w;
        let ch = win.client_h;

        let mut curr_x = tx;
        for c in text.chars() {
            let glyph = crate::gui::font::get_glyph(c);
            for row in 0..16 {
                let py = ty + row;
                if py >= ch {
                    break;
                }
                let byte = glyph[row];
                let row_offset = py * cw;
                for col in 0..8 {
                    let px = curr_x + col;
                    if px >= cw {
                        break;
                    }
                    if (byte >> (7 - col)) & 1 == 1 {
                        win.buffer[row_offset + px] = color;
                    }
                }
            }
            curr_x += 8;
        }
        Ok(())
    }

    pub fn blit_user_window(&mut self, id: u32, pixel_data: &[u32]) -> Result<(), &'static str> {
        let win = self.get_user_window_mut(id).ok_or("Ventana no encontrada")?;
        let copy_len = pixel_data.len().min(win.buffer.len());
        win.buffer[..copy_len].copy_from_slice(&pixel_data[..copy_len]);
        Ok(())
    }

    pub fn poll_user_window_event(&mut self, id: u32) -> Option<GuiEvent> {
        let win = self.get_user_window_mut(id)?;
        if !win.event_queue.is_empty() {
            Some(win.event_queue.remove(0))
        } else {
            None
        }
    }

    pub fn push_user_window_event(&mut self, id: u32, event: GuiEvent) {
        if let Some(win) = self.get_user_window_mut(id) {
            if win.event_queue.len() < 32 {
                win.event_queue.push(event);
            }
        }
    }

    pub fn bring_user_win_to_front(&mut self, id: u32) {
        if let Some(pos) = self.user_windows.iter().position(|w| w.id == id) {
            let win = self.user_windows.remove(pos);
            self.user_windows.push(win);
            self.active_user_win = Some(id);
        }
    }

    pub fn start_user_drag(&mut self, id: u32, mx: usize, my: usize) {
        let win_coords = self.get_user_window(id).map(|w| (w.x, w.y));
        if let Some((win_x, win_y)) = win_coords {
            self.dragging_user_window = Some(id);
            self.drag_offset_x = mx.saturating_sub(win_x);
            self.drag_offset_y = my.saturating_sub(win_y);
            self.bring_user_win_to_front(id);
        }
    }

    pub fn hit_test_user_windows(&self, mx: usize, my: usize) -> Option<(u32, UserWindowHit)> {
        for win in self.user_windows.iter().rev() {
            if win.minimized {
                continue;
            }
            if mx >= win.x && mx <= win.x + win.width && my >= win.y && my <= win.y + win.height {
                if my <= win.y + 26 {
                    if mx >= win.x + 8 && mx <= win.x + 24 {
                        return Some((win.id, UserWindowHit::CloseButton));
                    }
                    return Some((win.id, UserWindowHit::TitleBar));
                } else {
                    let rel_x = mx.saturating_sub(win.x + 2);
                    let rel_y = my.saturating_sub(win.y + 26);
                    return Some((win.id, UserWindowHit::ClientArea(rel_x, rel_y)));
                }
            }
        }
        None
    }
}

pub static WINDOW_MGR: SpinMutex<WindowManager> = SpinMutex::new(WindowManager::new());
