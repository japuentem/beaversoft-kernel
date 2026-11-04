// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver OS Universal GUI Desktop & Window Compositor
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use crate::sync::SpinMutex;
use crate::drivers::vbe::VBE;
use crate::gui::canvas::{Canvas, Color};
use crate::drivers::timer::get_uptime_seconds;
use crate::task::scheduler::SCHEDULER;

use crate::gui::window::{WINDOW_MGR, WindowId};
use crate::gui::note::{draw_note_window, render_note_window, BEAVER_NOTE};
use crate::gui::icons::*;

pub use crate::gui::apps_system::*;
pub use crate::gui::apps_media::*;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DesktopTheme {
    DarkCosmic,
    LightNordic,
}

pub static CURRENT_THEME: SpinMutex<DesktopTheme> = SpinMutex::new(DesktopTheme::DarkCosmic);

/// Redibuja únicamente una ventana específica como región sucia (Dirty Rect)
/// incluyendo su sombra difusa (+8px) y actualiza el sprite del cursor sin tocar el resto de la pantalla.
pub fn render_window_dirty(win_id: WindowId) {
    match win_id {
        WindowId::Monitor => render_monitor_window(),
        WindowId::Terminal => render_terminal_window(),
        WindowId::Files => render_files_window(),
        WindowId::Player => render_player_window(),
        WindowId::Note => render_note_window(),
        WindowId::Sync => render_sync_window(),
        WindowId::Photos => render_photos_window(),
        WindowId::Calendar => render_calendar_window(),
    }
    let (mx, my) = {
        let m = crate::drivers::mouse::MOUSE.lock();
        (m.state.x as usize, m.state.y as usize)
    };
    Canvas::invalidate_cursor();
    Canvas::draw_cursor_at(mx, my);
}

pub fn render_desktop() {
    let (width, height) = {
        let vbe = VBE.lock();
        if !vbe.is_active {
            return;
        }
        (vbe.width, vbe.height)
    };

    GUI_TERMINAL.lock().mode = GuiViewMode::Desktop;
    WINDOW_MGR.lock().init_defaults();

    let theme = *CURRENT_THEME.lock();

    // 1. Fondo de Pantalla Cósmico (Noche) o Nórdico (Día)
    match theme {
        DesktopTheme::DarkCosmic => Canvas::draw_cosmic_wallpaper(width, height),
        DesktopTheme::LightNordic => Canvas::draw_nordic_wallpaper(width, height),
    }

    // 2. Barra Superior de Menú y Estado (Top Bar Estilo Lord OS / macOS con Identidad Beaver)
    let topbar_bg = match theme {
        DesktopTheme::DarkCosmic => Color::BEAVER_OBSIDIAN,
        DesktopTheme::LightNordic => 0xE2E8F0,
    };
    let topbar_border = match theme {
        DesktopTheme::DarkCosmic => Color::SLATE_700,
        DesktopTheme::LightNordic => Color::BEAVER_BORDER_LIGHT,
    };
    let topbar_text = match theme {
        DesktopTheme::DarkCosmic => Color::WHITE,
        DesktopTheme::LightNordic => Color::BEAVER_TEXT_DARK,
    };

    Canvas::fill_rect(0, 0, width, 28, topbar_bg);
    Canvas::draw_line_h(0, 27, width, topbar_border);

    // Logotipo e Identidad Beaver
    Canvas::fill_rounded_rect(8, 4, 20, 20, 5, Color::BEAVER_AMBER);
    Canvas::draw_string(14, 6, "B", Color::WHITE, None);
    Canvas::draw_string_prop(34, 6, "Beaver OS", topbar_text, None);

    // Menús principales del sistema operativo con tipografía proporcional
    let mut menu_x = 110;
    menu_x += Canvas::draw_string_prop(menu_x, 6, "Archivo", if theme == DesktopTheme::DarkCosmic { Color::SLATE_200 } else { Color::BEAVER_TEXT_DARK }, None) + 16;
    menu_x += Canvas::draw_string_prop(menu_x, 6, "Red Mesh", Color::BEAVER_TEAL_BRIGHT, None) + 16;
    menu_x += Canvas::draw_string_prop(menu_x, 6, "Sincronizacion", Color::BEAVER_AMBER, None) + 16;
    menu_x += Canvas::draw_string_prop(menu_x, 6, "Audio", if theme == DesktopTheme::DarkCosmic { Color::SLATE_200 } else { Color::BEAVER_TEXT_DARK }, None) + 16;
    Canvas::draw_string_prop(menu_x, 6, "Ayuda", if theme == DesktopTheme::DarkCosmic { Color::SLATE_200 } else { Color::BEAVER_TEXT_DARK }, None);

    // Botón Buscador / Spotlight en Top Bar
    let pill_bg = if theme == DesktopTheme::DarkCosmic { Color::SLATE_800 } else { Color::WHITE };
    Canvas::fill_rounded_rect(390, 4, 85, 20, 10, pill_bg);
    draw_sidebar_icon(396, 6, SidebarIconKind::Search, Color::BEAVER_TEAL_BRIGHT);
    Canvas::draw_string_prop(416, 6, "Buscar", topbar_text, None);

    // Pildora de estado de la malla: nodos descubiertos por BeaverSync (Fase 5)
    let online_nodes = {
        let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
        mgr.online_count()
    };
    let mesh_pill_text = if online_nodes > 0 {
        alloc::format!("Mesh: {} nodo(s)", online_nodes)
    } else {
        alloc::string::String::from("Mesh: sin nodos")
    };
    let mesh_w = Canvas::string_width_prop(&mesh_pill_text);
    let mesh_pill_w = (mesh_w + 22).min(210);
    Canvas::fill_rounded_rect(485, 4, mesh_pill_w, 20, 10, pill_bg);
    let dot_color = if online_nodes > 0 { Color::BEAVER_TEAL_BRIGHT } else { Color::SLATE_400 };
    Canvas::fill_circle(495, 14, 3, dot_color);
    Canvas::draw_string_prop(503, 6, &mesh_pill_text, topbar_text, None);

    // Botón Conmutador de Tema (Día / Noche)
    Canvas::fill_rounded_rect(704, 4, 85, 20, 10, pill_bg);
    match theme {
        DesktopTheme::DarkCosmic => {
            Canvas::fill_circle(717, 14, 6, Color::BEAVER_AMBER);
            Canvas::fill_circle(720, 12, 5, pill_bg);
            Canvas::draw_string_prop(730, 6, "Noche", Color::BEAVER_AMBER, None);
        }
        DesktopTheme::LightNordic => {
            Canvas::fill_circle(717, 14, 4, Color::BEAVER_AMBER);
            Canvas::draw_line_h(709, 14, 3, Color::BEAVER_AMBER);
            Canvas::draw_line_h(723, 14, 3, Color::BEAVER_AMBER);
            Canvas::draw_line_v(717, 6, 3, Color::BEAVER_AMBER);
            Canvas::draw_line_v(717, 20, 3, Color::BEAVER_AMBER);
            Canvas::draw_string_prop(730, 6, "Dia", Color::BEAVER_AMBER_DARK, None);
        }
    }

    // Fecha y hora LOCAL (RTC en UTC + desfase TZ_OFFSET_HOURS), usuario y tiempo de actividad
    let rtc = crate::drivers::timer::local_datetime();
    let (_, _, curr_user) = SCHEDULER.lock().current_credentials();
    let uptime = get_uptime_seconds();
    let status_right = format!("{:02}/{:02} {:02}:{:02} | {} ({}s)", rtc.day, rtc.month, rtc.hour, rtc.minute, curr_user, uptime);
    let right_w = Canvas::string_width_prop(&status_right);
    let right_x = width.saturating_sub(right_w + 14);
    Canvas::draw_string_prop(right_x, 6, &status_right, topbar_text, None);

    // 3. Renderizado de Ventanas según Z-Order dinámico en BACKBUFFER
    let z_order = WINDOW_MGR.lock().z_order;
    for &win_id in &z_order {
        if WINDOW_MGR.lock().is_minimized(win_id) {
            continue;
        }
        match win_id {
            WindowId::Monitor => draw_monitor_window(),
            WindowId::Terminal => draw_terminal_window(),
            WindowId::Files => draw_files_window(),
            WindowId::Player => draw_player_window(),
            WindowId::Note => draw_note_window(),
            WindowId::Sync => draw_sync_window(),
            WindowId::Photos => draw_photos_window(),
            WindowId::Calendar => draw_calendar_window(),
        }
    }

    // 4. Dock Inferior Flotante de Cristal (Glassmorphism Capsule)
    let dock_w = 880;
    let dock_h = 50;
    let dock_x = (width.saturating_sub(dock_w)) / 2;
    let dock_y = height.saturating_sub(62);

    let dock_bg = match theme {
        DesktopTheme::DarkCosmic => 0x0B1222,
        DesktopTheme::LightNordic => 0xF8FAFC,
    };
    let dock_border = match theme {
        DesktopTheme::DarkCosmic => 0x334155,
        DesktopTheme::LightNordic => Color::BEAVER_BORDER_LIGHT,
    };
    let dock_label_col = match theme {
        DesktopTheme::DarkCosmic => Color::WHITE,
        DesktopTheme::LightNordic => Color::BEAVER_TEXT_DARK,
    };

    Canvas::fill_rounded_rect(dock_x + 4, dock_y + 4, dock_w, dock_h, 18, if theme == DesktopTheme::DarkCosmic { 0x05070D } else { 0x94A3B8 });
    Canvas::fill_rounded_rect(dock_x, dock_y, dock_w, dock_h, 18, dock_bg);
    Canvas::draw_rounded_rect(dock_x, dock_y, dock_w, dock_h, 18, dock_border);

    let app_meta: [(WindowId, &str, u32, SidebarIconKind); 8] = [
        (WindowId::Terminal, "Consola",    0x0F172A, SidebarIconKind::Terminal),
        (WindowId::Monitor,  "Monitor",    0x0D9488, SidebarIconKind::Settings),
        (WindowId::Files,    "Archivos",   0xF59E0B, SidebarIconKind::Home),
        (WindowId::Sync,     "BeaverSync", 0x1E3A8A, SidebarIconKind::Mesh),
        (WindowId::Player,   "Audio",      0x8B5CF6, SidebarIconKind::Music),
        (WindowId::Photos,   "Fotos",      0x0284C7, SidebarIconKind::Photos),
        (WindowId::Calendar, "Calendario", 0xEF4444, SidebarIconKind::Calendar),
        (WindowId::Note,     "Notas",      0xD97706, SidebarIconKind::Settings),
    ];

    for (i, (win_id, label, icon_bg, icon_kind)) in app_meta.iter().enumerate() {
        let btn_x = dock_x + 8 + i * 86;
        let btn_y = dock_y + 6;

        let icon_x = btn_x + 2;
        Canvas::fill_rounded_rect(icon_x, btn_y, 34, 34, 8, *icon_bg);
        Canvas::draw_rounded_rect(icon_x, btn_y, 34, 34, 8, Color::SLATE_600);
        Canvas::draw_line_h(icon_x + 3, btn_y + 1, 28, 0x94A3B8);

        if *win_id == WindowId::Files {
            draw_beaver_folder(icon_x + 5, btn_y + 8, 24, 18, false);
        } else {
            draw_sidebar_icon(icon_x + 9, btn_y + 9, *icon_kind, Color::WHITE);
        }

        Canvas::draw_string_prop(btn_x + 39, btn_y + 12, label, dock_label_col, None);

        let is_open = !WINDOW_MGR.lock().is_minimized(*win_id);
        if is_open {
            Canvas::fill_circle(icon_x + 17, dock_y + 44, 3, 0x0E7490);
            Canvas::fill_circle(icon_x + 17, dock_y + 44, 1, Color::WHITE);
        }
    }

    // App 8: Instalador USB
    {
        let btn_x = dock_x + 8 + 8 * 86;
        let btn_y = dock_y + 6;
        let icon_x = btn_x + 2;
        Canvas::fill_rounded_rect(icon_x, btn_y, 34, 34, 8, 0x0284C7);
        Canvas::draw_rounded_rect(icon_x, btn_y, 34, 34, 8, Color::SLATE_600);
        Canvas::draw_line_h(icon_x + 3, btn_y + 1, 28, 0x94A3B8);
        draw_sidebar_icon(icon_x + 9, btn_y + 9, SidebarIconKind::Disk, Color::WHITE);
        Canvas::draw_string_prop(btn_x + 39, btn_y + 12, "Instalar", dock_label_col, None);
    }

    // App 9: Salir / TTY1
    {
        let btn_x = dock_x + 8 + 9 * 86;
        let btn_y = dock_y + 6;
        let icon_x = btn_x + 2;
        Canvas::fill_rounded_rect(icon_x, btn_y, 34, 34, 8, Color::BEAVER_RUBY);
        Canvas::draw_rounded_rect(icon_x, btn_y, 34, 34, 8, 0xDC2626);
        Canvas::draw_line_h(icon_x + 3, btn_y + 1, 28, 0xFCA5A5);
        draw_sidebar_icon(icon_x + 9, btn_y + 9, SidebarIconKind::Trash, Color::WHITE);
        Canvas::draw_string_prop(btn_x + 39, btn_y + 12, "Salir", dock_label_col, None);
    }

    // 5. Spotlight Modal Overlay (si está abierto)
    draw_spotlight_overlay(width, height);

    // 6. Pie de Página / Instrucción para el usuario
    Canvas::draw_string_prop(
        20,
        height.saturating_sub(12),
        "VESA VBE TrueColor | BeaverNote Persistente (/disk) | Dirty Rects Activos | ESC = TTY1",
        Color::SLATE_500,
        None,
    );

    // 7. Cursor de ratón moderno interactivo
    Canvas::invalidate_cursor();
    let (mx, my) = {
        let m = crate::drivers::mouse::MOUSE.lock();
        (m.state.x as usize, m.state.y as usize)
    };
    Canvas::draw_cursor_at(mx, my);

    // 8. Presentación atómica de doble búfer (RAM -> LFB)
    Canvas::present();
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum GuiViewMode {
    Desktop,
    FullScreenCli,
}

pub struct GuiTerminalState {
    pub mode: GuiViewMode,
    pub lines: Vec<String>,
    pub input_buffer: String,
    pub current_line: String,
}

impl GuiTerminalState {
    pub const fn new() -> Self {
        Self {
            mode: GuiViewMode::Desktop,
            lines: Vec::new(),
            input_buffer: String::new(),
            current_line: String::new(),
        }
    }

    pub fn init_defaults(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::from("Beaver Graphical Shell v0.8.0 (x86_64 Long Mode)"));
            self.lines.push(String::from("Motor de comandos unificado: Escribe 'help' para ver los 40+ comandos."));
            self.lines.push(String::from("Escribe 'cli' o presiona ESC para pasar a consola completa."));
        }
    }

    pub fn append_str(&mut self, s: &str) {
        for ch in s.chars() {
            if ch == '\n' {
                let line = core::mem::take(&mut self.current_line);
                self.lines.push(line);
                if self.lines.len() > 300 {
                    self.lines.remove(0);
                }
            } else if ch == '\r' {
                // ignorar
            } else {
                self.current_line.push(ch);
            }
        }
    }

    pub fn flush_current_line(&mut self) {
        if !self.current_line.is_empty() {
            let line = core::mem::take(&mut self.current_line);
            self.lines.push(line);
            if self.lines.len() > 300 {
                self.lines.remove(0);
            }
        }
    }
}

pub fn gui_terminal_write_str(s: &str) {
    if let Some(mut term) = GUI_TERMINAL.try_lock() {
        term.append_str(s);
    }
}

pub static GUI_TERMINAL: SpinMutex<GuiTerminalState> = SpinMutex::new(GuiTerminalState::new());

pub fn draw_terminal_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Terminal) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Terminal))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "Terminal Interactivo Beaver (Ring 3 Ready)", is_active);
    GUI_TERMINAL.lock().init_defaults();

    Canvas::fill_rounded_rect(x + 6, y + 34, w.saturating_sub(12), h.saturating_sub(40), 6, Color::SLATE_900);
    Canvas::draw_rounded_rect(x + 6, y + 34, w.saturating_sub(12), h.saturating_sub(40), 6, Color::SLATE_700);

    let (curr_uid, _, curr_user) = SCHEDULER.lock().current_credentials();
    let prompt_symbol = if curr_uid == 0 { '#' } else { '$' };

    let state = GUI_TERMINAL.lock();
    let total_lines = state.lines.len();
    let max_visible = ((h.saturating_sub(55)) / 18).max(1);
    let start_idx = total_lines.saturating_sub(max_visible);

    let max_chars = ((w.saturating_sub(32)) / 8).max(10);
    let mut line_y = y + 42;
    for line in &state.lines[start_idx..] {
        let color = if line.contains("root@beaver") || line.contains("guest@beaver") || line.contains("beaver@beaver") {
            Color::BEAVER_ACCENT_CYAN
        } else if line.contains("[OK]") || line.contains("[Supervisor") {
            Color::BEAVER_GREEN
        } else if line.contains("[ERROR]") || line.contains("denegado") {
            Color::BEAVER_RED
        } else if line.starts_with("[INFO]") || line.starts_with("Comandos") {
            Color::BEAVER_YELLOW
        } else {
            Color::WHITE
        };
        let display_line = if line.len() > max_chars { &line[..max_chars] } else { line };
        Canvas::draw_string(x + 14, line_y, display_line, color, None);
        line_y += 18;
    }

    let cwd = crate::shell::get_current_dir();
    let prompt_str = format!("{}@beaver:{}{} {}_", curr_user, cwd, prompt_symbol, state.input_buffer);
    let prompt_display = if prompt_str.len() > max_chars { &prompt_str[..max_chars] } else { &prompt_str };
    Canvas::draw_string(x + 14, line_y.min(y + h - 22), prompt_display, Color::BEAVER_GREEN, None);
}

pub fn render_terminal_window() {
    draw_terminal_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Terminal) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
    }
}

pub fn render_fullscreen_cli() {
    let (width, height) = {
        let vbe = VBE.lock();
        if !vbe.is_active {
            return;
        }
        (vbe.width, vbe.height)
    };

    Canvas::fill_rect(0, 0, width, height, 0x0A0F1D);

    Canvas::fill_rect(0, 0, width, 26, Color::SLATE_800);
    Canvas::draw_rect(0, 0, width, 26, Color::SLATE_700);

    Canvas::draw_string(14, 5, "[BEAVER TTY1 - CONSOLA BARE-METAL]", Color::BEAVER_ACCENT_CYAN, None);
    Canvas::draw_string(370, 5, "Escribe 'desktop' para GUI | ESC = alternar", Color::BEAVER_YELLOW, None);

    let (curr_uid, _, curr_user) = SCHEDULER.lock().current_credentials();
    let uptime = get_uptime_seconds();
    let status_str = format!("{} (UID: {}) | Up: {}s", curr_user, curr_uid, uptime);
    let status_x = width.saturating_sub(status_str.len() * 8 + 16);
    Canvas::draw_string(status_x, 5, &status_str, Color::WHITE, None);

    let prompt_symbol = if curr_uid == 0 { '#' } else { '$' };
    let state = GUI_TERMINAL.lock();
    let total_lines = state.lines.len();
    let max_visible = ((height.saturating_sub(68)) / 19).max(1);
    let start_idx = total_lines.saturating_sub(max_visible);

    let mut y = 36;
    for line in &state.lines[start_idx..] {
        let color = if line.contains("root@beaver") || line.contains("guest@beaver") || line.contains("beaver@beaver") {
            Color::BEAVER_ACCENT_CYAN
        } else if line.contains("[OK]") || line.contains("[Supervisor") {
            Color::BEAVER_GREEN
        } else if line.contains("[ERROR]") || line.contains("denegado") {
            Color::BEAVER_RED
        } else if line.starts_with("[INFO]") || line.starts_with("Comandos") {
            Color::BEAVER_YELLOW
        } else {
            Color::WHITE
        };
        Canvas::draw_string(16, y, line, color, None);
        y += 19;
    }

    let cwd = crate::shell::get_current_dir();
    let prompt_str = format!("{}@beaver:{}{} {}_", curr_user, cwd, prompt_symbol, state.input_buffer);
    Canvas::draw_string(16, y.min(height.saturating_sub(24)), &prompt_str, Color::BEAVER_GREEN, None);

    Canvas::invalidate_cursor();
    let (mx, my) = {
        let m = crate::drivers::mouse::MOUSE.lock();
        (m.state.x as usize, m.state.y as usize)
    };
    Canvas::draw_cursor_at(mx, my);

    Canvas::present();
}

pub fn handle_gui_char(ch: char) {
    let (is_desktop, is_spot_open, is_note_active) = {
        let is_desk = GUI_TERMINAL.lock().mode == GuiViewMode::Desktop;
        let is_spot = SPOTLIGHT.lock().is_open;
        let is_note = WINDOW_MGR.lock().is_active(WindowId::Note);
        (is_desk, is_spot, is_note)
    };

    // Prioridad 1: Si Spotlight está abierto, capturar sus eventos con prioridad absoluta
    if is_desktop && is_spot_open {
        match ch {
            '\x1B' => {
                // ESC: Cierra Spotlight inmediatamente
                SPOTLIGHT.lock().is_open = false;
                render_desktop();
            }
            '\n' => {
                let target_opt = {
                    let mut spot = SPOTLIGHT.lock();
                    let results = get_spotlight_results(&spot.query);
                    if !results.is_empty() {
                        let idx = spot.selected_idx.min(results.len() - 1);
                        spot.is_open = false;
                        Some(results[idx].target)
                    } else {
                        spot.is_open = false;
                        None
                    }
                };
                if let Some(target) = target_opt {
                    crate::drivers::audio::click_feedback();
                    if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                        mgr.unminimize_and_focus(target);
                    }
                }
                render_desktop();
            }
            '\x08' => {
                // Backspace en búsqueda Spotlight
                let mut spot = SPOTLIGHT.lock();
                spot.query.pop();
                spot.selected_idx = 0;
                drop(spot);
                render_desktop();
            }
            crate::drivers::keyboard::KEY_UP_ARROW => {
                let mut spot = SPOTLIGHT.lock();
                if spot.selected_idx > 0 {
                    spot.selected_idx -= 1;
                }
                drop(spot);
                render_desktop();
            }
            crate::drivers::keyboard::KEY_DOWN_ARROW => {
                let mut spot = SPOTLIGHT.lock();
                let count = get_spotlight_results(&spot.query).len();
                if count > 0 && spot.selected_idx + 1 < count {
                    spot.selected_idx += 1;
                }
                drop(spot);
                render_desktop();
            }
            c if c >= ' ' && c <= '~' => {
                let mut spot = SPOTLIGHT.lock();
                if spot.query.len() < 36 {
                    spot.query.push(c);
                    spot.selected_idx = 0;
                }
                drop(spot);
                render_desktop();
            }
            _ => {}
        }
        return;
    }

    // Prioridad 2: Alternar entre Escritorio y FullScreenCli con tecla ESC si Spotlight no está activo
    if ch == '\x1b' {
        let mut state = GUI_TERMINAL.lock();
        if state.mode == GuiViewMode::Desktop {
            state.mode = GuiViewMode::FullScreenCli;
            drop(state);
            render_fullscreen_cli();
        } else {
            state.mode = GuiViewMode::Desktop;
            drop(state);
            render_desktop();
        }
        return;
    }

    if is_desktop && is_note_active {
        match ch {
            '\n' => {
                BEAVER_NOTE.lock().insert_newline();
                render_note_window();
            }
            '\x08' => {
                BEAVER_NOTE.lock().delete_char();
                render_note_window();
            }
            '\x13' => {
                let _ = BEAVER_NOTE.lock().save_to_disk();
                render_note_window();
            }
            c if c >= ' ' && c <= '~' => {
                BEAVER_NOTE.lock().insert_char(c);
                render_note_window();
            }
            _ => {}
        }
        return;
    }

    match ch {
        '\n' => {
            let cmd = {
                let mut state = GUI_TERMINAL.lock();
                let cmd = state.input_buffer.trim().to_string();
                let (curr_uid, _, curr_user) = SCHEDULER.lock().current_credentials();
                let prompt_symbol = if curr_uid == 0 { '#' } else { '$' };
                let cwd = crate::shell::get_current_dir();
                state.lines.push(format!("{}@beaver:{}{} {}", curr_user, cwd, prompt_symbol, cmd));
                state.input_buffer.clear();
                cmd
            };

            let mut mode_changed = false;

            if !cmd.is_empty() {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                match parts[0] {
                    "cli" | "fullscreen" => {
                        let mut state = GUI_TERMINAL.lock();
                        state.lines.push(String::from("[INFO] Modo consola a pantalla completa activado."));
                        state.lines.push(String::from("[INFO] Escribe 'desktop' o presiona ESC para volver a la interfaz grafica."));
                        state.mode = GuiViewMode::FullScreenCli;
                        mode_changed = true;
                    }
                    "desktop" | "gui" => {
                        let mut state = GUI_TERMINAL.lock();
                        state.lines.push(String::from("[INFO] Restaurando entorno de escritorio multi-ventana..."));
                        state.mode = GuiViewMode::Desktop;
                        mode_changed = true;
                    }
                    "exit" | "quit" => {
                        let mut state = GUI_TERMINAL.lock();
                        if state.mode == GuiViewMode::Desktop {
                            state.lines.push(String::from("[SISTEMA] Has salido del entorno multi-ventana a la Consola TTY1."));
                            state.lines.push(String::from("[SISTEMA] Escribe 'desktop' o presiona ESC para volver a la interfaz grafica."));
                            state.mode = GuiViewMode::FullScreenCli;
                            mode_changed = true;
                        } else {
                            state.lines.push(String::from("[SISTEMA] Ya te encuentras en la consola principal TTY1. Escribe 'desktop' para GUI."));
                        }
                    }
                    "gui-off" => {
                        let mut state = GUI_TERMINAL.lock();
                        state.lines.push(String::from("[SISTEMA] En arquitectura x86_64 bare-metal, la consola nativa de alto rendimiento es TTY1."));
                        state.lines.push(String::from("[SISTEMA] Escribe 'desktop' o presiona ESC para volver a la interfaz grafica."));
                        state.mode = GuiViewMode::FullScreenCli;
                        mode_changed = true;
                    }
                    "clear" => {
                        GUI_TERMINAL.lock().lines.clear();
                    }
                    _ => {
                        crate::shell::SHELL.lock().execute_command(&cmd);
                        GUI_TERMINAL.lock().flush_current_line();
                    }
                }
            }

            let current_mode = GUI_TERMINAL.lock().mode;

            if mode_changed {
                match current_mode {
                    GuiViewMode::Desktop => render_desktop(),
                    GuiViewMode::FullScreenCli => render_fullscreen_cli(),
                }
            } else {
                match current_mode {
                    GuiViewMode::Desktop => render_terminal_window(),
                    GuiViewMode::FullScreenCli => render_fullscreen_cli(),
                }
            }
        }
        '\x08' => {
            let mut state = GUI_TERMINAL.lock();
            state.input_buffer.pop();
            let mode = state.mode;
            drop(state);
            match mode {
                GuiViewMode::Desktop => render_terminal_window(),
                GuiViewMode::FullScreenCli => render_fullscreen_cli(),
            }
        }
        c if c >= ' ' && c <= '~' => {
            let mut state = GUI_TERMINAL.lock();
            let limit = if state.mode == GuiViewMode::FullScreenCli { 90 } else { 36 };
            if state.input_buffer.len() < limit {
                state.input_buffer.push(c);
                let mode = state.mode;
                drop(state);
                if !crate::interrupts::has_pending_input() {
                    match mode {
                        GuiViewMode::Desktop => render_terminal_window(),
                        GuiViewMode::FullScreenCli => render_fullscreen_cli(),
                    }
                }
            }
        }
        _ => {}
    }
}

pub fn handle_mouse_event(state: crate::drivers::mouse::MouseState) {
    let mode = {
        if let Some(terminal) = GUI_TERMINAL.try_lock() {
            terminal.mode
        } else {
            return;
        }
    };

    let mx = state.x as usize;
    let my = state.y as usize;

    static mut PREV_LEFT_BTN: bool = false;
    // SAFETY: PREV_LEFT_BTN solo es accedido en el flujo de eventos de ratón serializado.
    let prev_btn = unsafe { PREV_LEFT_BTN };
    let left_pressed_now = state.left_button && !prev_btn;
    let left_released_now = !state.left_button && prev_btn;
    // SAFETY: Actualización atómica del estado previo del botón izquierdo.
    unsafe {
        PREV_LEFT_BTN = state.left_button;
    }

    if mode == GuiViewMode::Desktop {
        if left_pressed_now {
            if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                if let Some((win_id, btn)) = mgr.hit_test_buttons(mx, my) {
                    crate::drivers::audio::click_feedback();
                    match btn {
                        crate::gui::window::WindowButton::Close | crate::gui::window::WindowButton::Minimize => {
                            mgr.minimize(win_id);
                        }
                        crate::gui::window::WindowButton::Maximize => {
                            let (w, h) = Canvas::screen_dimensions();
                            mgr.restore_or_maximize(win_id, w, h);
                        }
                    }
                    drop(mgr);
                    render_desktop();
                    return;
                }

                if let Some(win_id) = mgr.hit_test_titlebar(mx, my) {
                    mgr.start_drag(win_id, mx, my);
                    drop(mgr);
                    render_desktop();
                    return;
                } else if let Some(win_id) = mgr.hit_test_window(mx, my) {
                    let was_already_active = mgr.is_active(win_id);
                    mgr.bring_to_front(win_id);
                    let win_coords = mgr.get_window(win_id).map(|w| (w.x, w.y));
                    drop(mgr);

                    if win_id == WindowId::Note {
                        if let Some((wx, wy)) = win_coords {
                            let _ = crate::gui::note::handle_note_click(mx, my, wx, wy);
                        }
                    } else if win_id == WindowId::Files {
                        if let Some((wx, wy)) = win_coords {
                            handle_files_click(mx, my, wx, wy);
                        }
                    } else if win_id == WindowId::Player {
                        if let Some((wx, wy)) = win_coords {
                            handle_player_click(mx, my, wx, wy);
                        }
                    } else if win_id == WindowId::Sync {
                        if let Some((wx, wy)) = win_coords {
                            handle_sync_click(mx, my, wx, wy);
                        }
                    } else if win_id == WindowId::Photos {
                        if let Some((wx, wy)) = win_coords {
                            handle_photos_click(mx, my, wx, wy);
                        }
                    } else if win_id == WindowId::Calendar {
                        if let Some((wx, wy)) = win_coords {
                            handle_calendar_click(mx, my, wx, wy);
                        }
                    }

                    let spot_opened = SPOTLIGHT.try_lock().map(|s| s.is_open).unwrap_or(false);
                    if was_already_active && !spot_opened && !WINDOW_MGR.lock().is_minimized(win_id) {
                        // Redibujo parcial por región sucia (Dirty Rect) cuando la ventana ya estaba al frente
                        render_window_dirty(win_id);
                    } else {
                        render_desktop();
                    }
                    return;
                }
            }
            handle_desktop_click(mx, my);
        } else if state.left_button {
            let moved = {
                if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                    mgr.update_drag(mx, my)
                } else {
                    false
                }
            };
            if moved {
                render_desktop();
            } else {
                Canvas::try_draw_cursor_at(mx, my);
            }
        } else if left_released_now {
            let was_dragging = {
                if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                    if mgr.dragging_window.is_some() {
                        mgr.stop_drag();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            if was_dragging {
                render_desktop();
            } else {
                Canvas::try_draw_cursor_at(mx, my);
            }
        } else {
            Canvas::try_draw_cursor_at(mx, my);
        }
    } else {
        Canvas::try_draw_cursor_at(mx, my);
        if left_pressed_now {
            handle_fullscreen_click(mx, my);
        }
    }
}

fn handle_desktop_click(mx: usize, my: usize) {
    let (scr_w, scr_h) = Canvas::screen_dimensions();

    // 0. Si Spotlight está abierto, evaluar clics en el buscador usando la resolución activa
    {
        let is_spot_open = SPOTLIGHT.lock().is_open;
        if is_spot_open {
            let modal_w = 480;
            let modal_h = 240;
            let modal_x = (scr_w.saturating_sub(modal_w)) / 2;
            let modal_y = 120;

            if mx >= modal_x && mx <= modal_x + modal_w && my >= modal_y && my <= modal_y + modal_h {
                let target_opt = {
                    let spot = SPOTLIGHT.lock();
                    let results = get_spotlight_results(&spot.query);
                    let mut found = None;
                    for (i, res) in results.iter().enumerate().take(4) {
                        let ry = modal_y + 66 + i * 40;
                        if my >= ry && my <= ry + 34 {
                            found = Some(res.target);
                            break;
                        }
                    }
                    found
                };

                if let Some(target_win) = target_opt {
                    crate::drivers::audio::click_feedback();
                    SPOTLIGHT.lock().is_open = false;
                    if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                        mgr.unminimize_and_focus(target_win);
                    }
                    render_desktop();
                    return;
                }
                // Si hizo clic dentro de la caja de texto, mantener Spotlight abierto
                if my >= modal_y + 14 && my <= modal_y + 50 {
                    return;
                }
                return;
            } else {
                SPOTLIGHT.lock().is_open = false;
                render_desktop();
                return;
            }
        }
    }

    // 1. Clic en la barra superior (Top Bar)
    if my <= 28 {
        if mx >= 390 && mx <= 475 {
            crate::drivers::audio::click_feedback();
            let mut spot = SPOTLIGHT.lock();
            spot.is_open = !spot.is_open;
            drop(spot);
            render_desktop();
            return;
        }

        if mx >= 704 && mx <= 790 {
            crate::drivers::audio::click_feedback();
            {
                let mut th = CURRENT_THEME.lock();
                *th = match *th {
                    DesktopTheme::DarkCosmic => DesktopTheme::LightNordic,
                    DesktopTheme::LightNordic => DesktopTheme::DarkCosmic,
                };
            }
            render_desktop();
            return;
        }
    }

    // 2. Clic en la barra Dock (Launcher inferior de 10 aplicaciones) independiente de la resolución
    let dock_w = 880;
    let dock_x = (scr_w.saturating_sub(dock_w)) / 2;
    let dock_y = scr_h.saturating_sub(62);

    if my >= dock_y + 6 && my <= dock_y + 44 {
        for i in 0..10 {
            let btn_x = dock_x + 8 + i * 86;
            if mx >= btn_x && mx <= btn_x + 82 {
                crate::drivers::audio::click_feedback();
                match i {
                    0 => {
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Terminal);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    1 => {
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Monitor);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    2 => {
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Files);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    3 => {
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Sync);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    4 => {
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Player);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    5 => {
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Photos);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    6 => {
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Calendar);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    7 => {
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Note);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    8 => {
                        if let Some(mut state) = GUI_TERMINAL.try_lock() {
                            state.lines.push(String::from("[DOCK] Lanzando Instalador Universal de Beaver OS..."));
                            drop(state);
                            crate::shell::SHELL.lock().execute_command("install -y");
                            GUI_TERMINAL.lock().flush_current_line();
                            render_terminal_window();
                        }
                        return;
                    }
                    9 => {
                        if let Some(mut state) = GUI_TERMINAL.try_lock() {
                            state.mode = GuiViewMode::FullScreenCli;
                            state.lines.push(String::from("[SISTEMA] Has salido del entorno multi-ventana a la Consola TTY1."));
                            state.lines.push(String::from("[SISTEMA] Escribe 'desktop' o presiona ESC para volver a la interfaz grafica."));
                            drop(state);
                            render_fullscreen_cli();
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }
    }

    // Clic en el área de fondo del escritorio
    if let Some(mut state) = GUI_TERMINAL.try_lock() {
        if state.lines.len() > 50 {
            state.lines.remove(0);
        }
        state.lines.push(format!("[MOUSE] Clic en fondo ({}, {})", mx, my));
        drop(state);
        let terminal_active = {
            if let Some(mgr) = WINDOW_MGR.try_lock() {
                mgr.is_active(WindowId::Terminal)
            } else {
                false
            }
        };
        if terminal_active {
            render_terminal_window();
        } else {
            render_desktop();
        }
    }
}

fn handle_fullscreen_click(mx: usize, my: usize) {
    if my <= 26 {
        if let Some(mut state) = GUI_TERMINAL.try_lock() {
            state.mode = GuiViewMode::Desktop;
            drop(state);
            render_desktop();
        }
    } else if let Some(mut state) = GUI_TERMINAL.try_lock() {
        if state.lines.len() > 50 {
            state.lines.remove(0);
        }
        state.lines.push(format!("[MOUSE] Clic en consola ({}, {})", mx, my));
        drop(state);
        render_fullscreen_cli();
    }
}
