// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaversoft Universal GUI Desktop & Window Compositor
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use crate::sync::SpinMutex;
use crate::drivers::vbe::VBE;
use crate::gui::canvas::{Canvas, Color};
use crate::memory::heap::HEAP_ALLOCATOR;
use crate::memory::frame_allocator::FRAME_ALLOCATOR;
use crate::drivers::timer::get_uptime_seconds;
use crate::task::scheduler::SCHEDULER;

use crate::gui::window::{WINDOW_MGR, WindowId};
use crate::gui::note::{draw_note_window, render_note_window, BEAVER_NOTE};
use crate::gui::icons::*;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DesktopTheme {
    DarkCosmic,
    LightNordic,
}

pub static CURRENT_THEME: SpinMutex<DesktopTheme> = SpinMutex::new(DesktopTheme::DarkCosmic);

pub struct SpotlightState {
    pub is_open: bool,
    pub query: String,
}

pub static SPOTLIGHT: SpinMutex<SpotlightState> = SpinMutex::new(SpotlightState {
    is_open: false,
    query: String::new(),
});

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

    // Píldora de estado de la malla. [DEMO]: los nodos de mesh.rs aún están codificados
    // (ver docs/plan_continuacion_2026-11-01.md, Fase 5). Se quitará la etiqueta cuando el descubrimiento sea real.
    let mesh_text = "Mesh: 4 Nodos  |  Sync: 100%";
    let demo_w = Canvas::string_width_prop("DEMO");
    let mesh_w = Canvas::string_width_prop(mesh_text);
    let mesh_pill_w = (demo_w + mesh_w + 30).min(210);
    Canvas::fill_rounded_rect(485, 4, mesh_pill_w, 20, 10, pill_bg);
    Canvas::fill_circle(495, 14, 3, Color::BEAVER_TEAL_BRIGHT);
    Canvas::draw_string_prop(503, 6, "DEMO", Color::BEAVER_AMBER, None);
    Canvas::draw_string_prop(503 + demo_w + 8, 6, mesh_text, topbar_text, None);

    // Botón Conmutador de Tema (Día / Noche) con icono dibujado (la fuente VGA 8x16 no tiene emojis)
    Canvas::fill_rounded_rect(704, 4, 85, 20, 10, pill_bg);
    match theme {
        DesktopTheme::DarkCosmic => {
            // Luna creciente: círculo ámbar recortado por un círculo del color de la píldora
            Canvas::fill_circle(717, 14, 6, Color::BEAVER_AMBER);
            Canvas::fill_circle(720, 12, 5, pill_bg);
            Canvas::draw_string_prop(730, 6, "Noche", Color::BEAVER_AMBER, None);
        }
        DesktopTheme::LightNordic => {
            // Sol: disco central con 4 rayos
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

    // 3. Renderizado de Ventanas según Z-Order dinámico en BACKBUFFER (sin blits intermedios a LFB)
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

    // 4. Dock Inferior Flotante de Cristal (Glassmorphism Capsule estilo Lord OS)
    let dock_w = 880;
    let dock_h = 50;
    let dock_x = (width.saturating_sub(dock_w)) / 2;
    let dock_y = height - 62;

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

    // Sombra suave proyectada bajo el Dock
    Canvas::fill_rounded_rect(dock_x + 4, dock_y + 4, dock_w, dock_h, 18, if theme == DesktopTheme::DarkCosmic { 0x05070D } else { 0x94A3B8 });
    // Cápsula principal de cristal
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

    // 6. Pie de Página / Instrucción para el usuario con tipografía proporcional
    Canvas::draw_string_prop(
        20,
        height - 12,
        "VESA VBE 1024x768x32 TrueColor | BeaverNote Persistente (/disk) | Arrastra ventanas | ESC = TTY1",
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

    // 7. Presentación atómica de doble búfer (RAM -> LFB)
    Canvas::present();
}

pub fn draw_monitor_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Monitor) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Monitor))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "Monitor de Recursos en Vivo (Transparente)", is_active);
    let (total_heap, used_heap, free_heap, alloc_count, _dealloc_count) = HEAP_ALLOCATOR.stats();
    let pct = HEAP_ALLOCATOR.usage_percent();
    let frames_alloc = FRAME_ALLOCATOR.allocated_frames_count();
    let frames_total = FRAME_ALLOCATOR.total_usable_frames();
    let frames_free  = FRAME_ALLOCATOR.free_frames_count();
    let ram_free_mib  = frames_free  * 4096 / (1024 * 1024);
    let ram_total_mib = frames_total * 4096 / (1024 * 1024);

    Canvas::draw_string_prop(x + 15, y + 38, "ARQUITECTURA DE PROCESAMIENTO", Color::BEAVER_AMBER_DARK, None);
    Canvas::draw_string_prop(x + 15, y + 56, "CPU: x86_64 Long Mode Bare-Metal | PIC 100Hz", Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 15, y + 72, "Aislamiento: Syscall MSR / Ring 3 Userland Sandbox", Color::BEAVER_TEXT_MUTED, None);

    Canvas::draw_string_prop(x + 15, y + 96, "MEMORIA DINAMICA (Cero Buffers Ocultos)", Color::BEAVER_TEAL, None);
    let heap_str = format!("Heap: {} KiB | Usado: {} KiB | Libre: {} KiB", total_heap / 1024, used_heap / 1024, free_heap / 1024);
    Canvas::draw_string_prop(x + 15, y + 116, &heap_str, Color::BEAVER_TEXT_DARK, None);

    let bar_w = w.saturating_sub(30);
    Canvas::fill_rounded_rect(x + 15, y + 136, bar_w, 16, 4, Color::BEAVER_TITANIUM_BAR);
    let fill_w = (bar_w * pct.min(100)) / 100;
    if fill_w > 0 {
        Canvas::fill_rounded_rect(x + 15, y + 136, fill_w, 16, 4, Color::BEAVER_TEAL);
    }
    Canvas::draw_rounded_rect(x + 15, y + 136, bar_w, 16, 4, Color::BEAVER_BORDER_LIGHT);

    let pct_str = format!("Ocupacion: {}% | Reserva Anti-OOM: 64 KiB", pct);
    Canvas::draw_string_prop(x + 15, y + 158, &pct_str, Color::BEAVER_AMBER_DARK, None);

    Canvas::draw_string_prop(x + 15, y + 182, "HILOS CONCURRENTES (PREEMPTIVE ROUND-ROBIN)", Color::BEAVER_TEAL, None);
    Canvas::draw_string_prop(x + 15, y + 202, "TID 0: main_shell       | RUNNING | UID: 0 (root)", Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 15, y + 220, "TID 1: telemetry_daemon | SLEEP   | UID: 0 (root)", Color::BEAVER_TEXT_MUTED, None);
    Canvas::draw_string_prop(x + 15, y + 238, "TID 2: storage_guard    | SLEEP   | UID: 0 (root)", Color::BEAVER_TEXT_MUTED, None);

    let stats_str = format!("Heap allocs: {} | RAM: {} MiB libre / {} MiB total | Marcos: {}", alloc_count, ram_free_mib, ram_total_mib, frames_alloc);
    Canvas::draw_string_prop(x + 15, y + 262, &stats_str, Color::BEAVER_TEXT_MUTED, None);
}

pub fn render_monitor_window() {
    draw_monitor_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Monitor) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::present_rect(x, y, w, h);
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum FilesViewTab {
    Net,
    Disk,
    Home,
    Proc,
    Sys,
}

pub static FILES_VIEW_TAB: SpinMutex<FilesViewTab> = SpinMutex::new(FilesViewTab::Net);

#[derive(Clone, Copy)]
pub struct PlayerState {
    pub is_playing: bool,
    pub track_idx: usize,
}

pub static PLAYER_STATE: SpinMutex<PlayerState> = SpinMutex::new(PlayerState {
    is_playing: false,
    track_idx: 0,
});

pub fn draw_files_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Files) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Files))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "BeaverFiles - Explorador VFS y Malla Lord OS", is_active);

    let tab = *FILES_VIEW_TAB.lock();

    // 1. Barra Lateral Izquierda (Sidebar estilo Lord OS / Finder)
    let sb_w = 140;
    Canvas::fill_rect(x + 1, y + 32, sb_w, h.saturating_sub(33), Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_line_v(x + 1 + sb_w, y + 32, h.saturating_sub(33), Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 12, y + 42, "UBICACIONES", Color::BEAVER_TEXT_MUTED, None);

    let tabs = [
        (FilesViewTab::Net,  "Red Mesh",     SidebarIconKind::Mesh,     60),
        (FilesViewTab::Disk, "Disco (/disk)",SidebarIconKind::Disk,     84),
        (FilesViewTab::Home, "Home (/home)", SidebarIconKind::Home,     108),
        (FilesViewTab::Proc, "Proc (/proc)", SidebarIconKind::Settings, 132),
        (FilesViewTab::Sys,  "Sys (/sys)",   SidebarIconKind::Terminal, 156),
    ];

    for (t, label, icon_kind, ty) in &tabs {
        let is_selected = *t == tab;
        if is_selected {
            Canvas::fill_rounded_rect(x + 6, y + ty - 3, sb_w - 12, 22, 6, 0xDBEAFE);
            draw_sidebar_icon(x + 12, y + ty - 1, *icon_kind, Color::BEAVER_AMBER);
            Canvas::draw_string_prop(x + 34, y + ty + 1, label, Color::BEAVER_TEXT_DARK, None);
        } else {
            draw_sidebar_icon(x + 12, y + ty - 1, *icon_kind, Color::BEAVER_TEXT_MUTED);
            Canvas::draw_string_prop(x + 34, y + ty + 1, label, Color::BEAVER_TEXT_DARK, None);
        }
    }

    Canvas::draw_string_prop(x + 12, y + 190, "RED MALLA", Color::BEAVER_TEXT_MUTED, None);
    draw_sidebar_icon(x + 12, y + 210, SidebarIconKind::Mesh, Color::BEAVER_TEAL);
    Canvas::draw_string_prop(x + 34, y + 210, "[DEMO] station (:15)", Color::BEAVER_TEXT_MUTED, None);

    draw_sidebar_icon(x + 12, y + 232, SidebarIconKind::Mesh, Color::BEAVER_TEAL);
    Canvas::draw_string_prop(x + 34, y + 232, "[DEMO] laptop  (:18)", Color::BEAVER_TEXT_MUTED, None);

    draw_sidebar_icon(x + 12, y + 254, SidebarIconKind::Mesh, Color::BEAVER_TEAL);
    Canvas::draw_string_prop(x + 34, y + 254, "[DEMO] server  (:20)", Color::BEAVER_TEXT_MUTED, None);

    // 2. Panel Principal Derecho (Lienzo Blanco / Lord OS)
    let main_x = x + sb_w + 10;
    let main_w = w.saturating_sub(sb_w + 18);

    // Barra de Navegación / Buscador superior
    Canvas::fill_rounded_rect(main_x + 4, y + 36, 44, 22, 4, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_string(main_x + 12, y + 39, "<  >", Color::BEAVER_TEXT_DARK, None);

    let path_label = match tab {
        FilesViewTab::Net => "Red Mesh",
        FilesViewTab::Disk => "Disco (/disk)",
        FilesViewTab::Home => "Home",
        FilesViewTab::Proc => "Proc",
        FilesViewTab::Sys => "Sistema",
    };
    Canvas::fill_rounded_rect(main_x + 54, y + 36, 120, 22, 4, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_string_prop(main_x + 64, y + 39, path_label, Color::BEAVER_TEXT_DARK, None);

    let search_w = 140;
    let search_x = main_x + main_w.saturating_sub(search_w + 6);
    Canvas::fill_rounded_rect(search_x, y + 36, search_w, 22, 11, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_string_prop(search_x + 14, y + 39, "Buscar...", Color::BEAVER_TEXT_MUTED, None);

    Canvas::draw_line_h(main_x, y + 64, main_w, Color::BEAVER_BORDER_LIGHT);

    // Contenido según pestaña con Iconografía Real 3D
    match tab {
        FilesViewTab::Disk | FilesViewTab::Home => {
            let row1_y = y + 78;
            let col0 = main_x + 20;
            let col1 = main_x + 105;
            let col2 = main_x + 190;
            let col3 = main_x + 275;

            // Carpeta 3D bin/
            draw_beaver_folder(col0, row1_y, 56, 44, false);
            Canvas::draw_string_prop(col0 + 18, row1_y + 48, "bin", Color::BEAVER_TEXT_DARK, None);

            // Documento chime.wav con badge WAV
            draw_file_document(col1, row1_y, 44, 48, "WAV", Color::BEAVER_EMERALD);
            Canvas::draw_string_prop(col1 - 4, row1_y + 52, "chime.wav", Color::BEAVER_TEXT_DARK, None);

            // Documento logo.bmp con badge BMP
            draw_file_document(col2, row1_y, 44, 48, "BMP", Color::BEAVER_TEAL);
            Canvas::draw_string_prop(col2, row1_y + 52, "logo.bmp", Color::BEAVER_TEXT_DARK, None);

            // Documento notas.txt con badge TXT
            draw_file_document(col3, row1_y, 44, 48, "TXT", Color::BEAVER_AMBER);
            Canvas::draw_string_prop(col3 - 2, row1_y + 52, "notas.txt", Color::BEAVER_TEXT_DARK, None);

            // Fila 2
            let row2_y = y + 160;
            draw_file_document(col0 + 6, row2_y, 44, 48, "BS", Color::BEAVER_PURPLE);
            Canvas::draw_string_prop(col0 - 2, row2_y + 52, "welcome.bs", Color::BEAVER_TEXT_DARK, None);
        }
        FilesViewTab::Net => {
            let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
            let nodes = mgr.get_nodes();
            for (i, node) in nodes.iter().enumerate() {
                let card_x = main_x + (i % 2) * (main_w / 2);
                let card_y = y + 76 + (i / 2) * 82;
                let card_w = (main_w / 2).saturating_sub(10);
                if card_y + 75 <= y + h {
                    Canvas::fill_rounded_rect(card_x, card_y, card_w, 75, 8, Color::WHITE);
                    Canvas::draw_rounded_rect(card_x, card_y, card_w, 75, 8, Color::BEAVER_BORDER_LIGHT);

                    draw_device_icon(card_x + 10, card_y + 12, DeviceKind::Pc, Color::BEAVER_TEAL);

                    let title = format!("{}", node.name);
                    Canvas::draw_string_prop(card_x + 38, card_y + 10, &title, Color::BEAVER_TEXT_DARK, None);
                    let ip_cores = format!("IP: {} | {} Cores", node.ip.to_string(), node.cores);
                    Canvas::draw_string_prop(card_x + 38, card_y + 28, &ip_cores, Color::BEAVER_TEXT_MUTED, None);
                    let ram_shares = format!("RAM: {}MB | Compartido", node.ram_free_mb);
                    Canvas::draw_string_prop(card_x + 38, card_y + 46, &ram_shares, Color::BEAVER_EMERALD, None);
                }
            }
        }
        _ => {
            Canvas::fill_rounded_rect(main_x + 8, y + 78, main_w.saturating_sub(16), 80, 8, Color::WHITE);
            Canvas::draw_rounded_rect(main_x + 8, y + 78, main_w.saturating_sub(16), 80, 8, Color::BEAVER_BORDER_LIGHT);
            Canvas::draw_string_prop(main_x + 18, y + 90, "DIRECTORIO DEL SISTEMA VFS (Lord OS Style)", Color::BEAVER_AMBER_DARK, None);
            Canvas::draw_string_prop(main_x + 18, y + 112, "Acceso seguro mediante arquitectura microkernel y sandbox Ring 3.", Color::BEAVER_TEXT_DARK, None);
        }
    }

    // Pie de página de BeaverFiles estilo Lord OS
    let footer_y = y + h - 22;
    Canvas::draw_string_prop(main_x + 12, footer_y + 3, "5 elementos, 2.4 MB disponibles en BeaverFS", Color::BEAVER_TEXT_MUTED, None);
}

pub fn render_files_window() {
    draw_files_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Files) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::present_rect(x, y, w, h);
    }
}

pub fn draw_player_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Player) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Player))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "01 Beaver Chime.wav - BeaverAudio", is_active);

    let state = *PLAYER_STATE.lock();

    // 1. Barra Lateral Izquierda (Biblioteca estilo Lord OS)
    let sb_w = 110;
    Canvas::fill_rect(x + 1, y + 32, sb_w, h.saturating_sub(33), Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_line_v(x + 1 + sb_w, y + 32, h.saturating_sub(33), Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 10, y + 42, "BIBLIOTECA", Color::BEAVER_TEXT_MUTED, None);

    // Canciones (Seleccionado con píldora)
    Canvas::fill_rounded_rect(x + 5, y + 58, sb_w - 10, 22, 6, 0xDBEAFE);
    draw_sidebar_icon(x + 10, y + 60, SidebarIconKind::Music, Color::BEAVER_AMBER);
    Canvas::draw_string_prop(x + 30, y + 62, "Canciones", Color::BEAVER_TEXT_DARK, None);

    draw_sidebar_icon(x + 10, y + 84, SidebarIconKind::Settings, Color::BEAVER_TEXT_MUTED);
    Canvas::draw_string_prop(x + 30, y + 86, "Efectos", Color::BEAVER_TEXT_DARK, None);

    draw_sidebar_icon(x + 10, y + 108, SidebarIconKind::Terminal, Color::BEAVER_TEXT_MUTED);
    Canvas::draw_string_prop(x + 30, y + 110, "Sintesis", Color::BEAVER_TEXT_DARK, None);

    // 2. Lista de Pistas (Estilo Lord OS)
    let list_x = x + sb_w + 12;
    let list_w = w.saturating_sub(sb_w + 22);

    Canvas::draw_string_prop(list_x, y + 40, "#   Titulo                      Artista               Tiempo", Color::BEAVER_TEXT_MUTED, None);
    Canvas::draw_line_h(list_x, y + 54, list_w, Color::BEAVER_BORDER_LIGHT);

    let tracks = [
        ("1", "Beaver Chime (PCM Startup)",  "BeaverSoft Audio Lab", "0:03"),
        ("2", "Northern Lights (Ring3)",    "The Sample Band",      "3:45"),
        ("3", "Silicon Dawn (Waves DSP)",   "MSR Audio Engine",     "4:12"),
        ("4", "Kernel Harmony (HDA Dual)",  "Beaver Core Driver",   "2:30"),
    ];

    for (i, (num, title, artist, duration)) in tracks.iter().enumerate() {
        let ty = y + 64 + i * 26;
        if i == state.track_idx {
            Canvas::fill_rounded_rect(list_x - 4, ty - 3, list_w, 22, 5, 0xFEF3C7); // Acento cálido ámbar
            Canvas::draw_string_prop(list_x, ty + 1, num, Color::BEAVER_AMBER_DARK, None);
            Canvas::draw_string_prop(list_x + 24, ty + 1, title, Color::BEAVER_TEXT_DARK, None);
            Canvas::draw_string_prop(list_x + 240, ty + 1, artist, Color::BEAVER_TEXT_MUTED, None);
            Canvas::draw_string_prop(list_x + list_w.saturating_sub(44), ty + 1, duration, Color::BEAVER_AMBER_DARK, None);
        } else {
            Canvas::draw_string_prop(list_x, ty + 1, num, Color::BEAVER_TEXT_MUTED, None);
            Canvas::draw_string_prop(list_x + 24, ty + 1, title, Color::BEAVER_TEXT_DARK, None);
            Canvas::draw_string_prop(list_x + 240, ty + 1, artist, Color::BEAVER_TEXT_MUTED, None);
            Canvas::draw_string_prop(list_x + list_w.saturating_sub(44), ty + 1, duration, Color::BEAVER_TEXT_MUTED, None);
        }
    }

    // 3. Panel de Control Inferior Estilo Lord OS (Carátula Squircle, Glifos Vectoriales y Barra Fluida)
    let ctrl_y = y + h.saturating_sub(68);
    Canvas::fill_rect(x + 1, ctrl_y, w - 2, 67, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_line_h(x + 1, ctrl_y, w - 2, Color::BEAVER_BORDER_LIGHT);

    // Carátula Squircle (42x42 con radio 8)
    let art_x = x + 12;
    let art_y = ctrl_y + 12;
    Canvas::fill_rounded_rect(art_x, art_y, 42, 42, 8, 0x1E1B4B);
    Canvas::draw_rounded_rect(art_x, art_y, 42, 42, 8, Color::SLATE_600);
    Canvas::draw_string(art_x + 17, art_y + 13, "B", Color::BEAVER_AMBER, None);

    // Título y detalles de la canción
    let (cur_title, cur_status) = if state.is_playing {
        ("Beaver Chime (PCM Startup)", "Reproduciendo - HDA Duplex 44.1kHz")
    } else {
        ("Beaver Chime (PCM Startup)", "En pausa - Presione Play")
    };
    Canvas::draw_string_prop(x + 62, ctrl_y + 14, cur_title, Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 62, ctrl_y + 30, cur_status, Color::BEAVER_TEXT_MUTED, None);

    // Botones de reproducción multimedia vectoriales
    let ctrl_cx = x + w / 2 + 40;
    let btn_cy = ctrl_y + 24;

    draw_media_button(ctrl_cx.saturating_sub(56), btn_cy, 10, MediaBtnKind::Shuffle, false);
    draw_media_button(ctrl_cx.saturating_sub(28), btn_cy, 11, MediaBtnKind::Previous, false);
    draw_media_button(ctrl_cx, btn_cy, 15, if state.is_playing { MediaBtnKind::Pause } else { MediaBtnKind::Play }, true);
    draw_media_button(ctrl_cx + 28, btn_cy, 11, MediaBtnKind::Next, false);
    draw_media_button(ctrl_cx + 56, btn_cy, 10, MediaBtnKind::Repeat, false);

    // Barra de tiempo fluida con indicador circular (Scrub Bar)
    let bar_x = x + 62;
    let bar_w = w.saturating_sub(130);
    let bar_y = ctrl_y + 48;

    Canvas::draw_string(bar_x.saturating_sub(38), bar_y - 4, "0:02", Color::BEAVER_TEXT_MUTED, None);
    Canvas::fill_rounded_rect(bar_x, bar_y, bar_w, 4, 2, Color::BEAVER_BORDER_LIGHT);
    let fill_w = (bar_w * 66) / 100;
    Canvas::fill_rounded_rect(bar_x, bar_y, fill_w, 4, 2, Color::BEAVER_AMBER);
    Canvas::fill_circle(bar_x + fill_w, bar_y + 2, 4, Color::BEAVER_AMBER_DARK);
    Canvas::draw_string(bar_x + bar_w + 10, bar_y - 4, "0:03", Color::BEAVER_TEXT_MUTED, None);
}

pub fn render_player_window() {
    draw_player_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Player) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::present_rect(x, y, w, h);
    }
}

pub fn handle_files_click(mx: usize, my: usize, wx: usize, wy: usize) {
    if mx >= wx && mx <= wx + 140 {
        if my >= wy + 50 && my <= wy + 72 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Net;
        } else if my >= wy + 74 && my <= wy + 96 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Disk;
        } else if my >= wy + 98 && my <= wy + 120 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Home;
        } else if my >= wy + 122 && my <= wy + 144 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Proc;
        } else if my >= wy + 146 && my <= wy + 168 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Sys;
        }
    }

    // Clic en la barra de búsqueda de BeaverFiles para abrir Spotlight
    let win = WINDOW_MGR.lock().get_window(WindowId::Files).cloned();
    if let Some(w) = win {
        let sb_w = 140;
        let main_x = wx + sb_w + 10;
        let main_w = w.width.saturating_sub(sb_w + 18);
        let search_w = 140;
        let search_x = main_x + main_w.saturating_sub(search_w + 6);
        if mx >= search_x && mx <= search_x + search_w && my >= wy + 36 && my <= wy + 58 {
            crate::drivers::audio::click_feedback();
            SPOTLIGHT.lock().is_open = true;
        }
    }
}

pub fn handle_player_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::Player).cloned();
    if let Some(w) = win {
        let ctrl_y = wy + w.height.saturating_sub(68);
        let ctrl_cx = wx + w.width / 2 + 40;
        // Clic en botón Play/Pausa central (radio 15)
        if mx >= ctrl_cx.saturating_sub(18) && mx <= ctrl_cx + 18 && my >= ctrl_y + 10 && my <= ctrl_y + 40 {
            let mut state = PLAYER_STATE.lock();
            state.is_playing = !state.is_playing;
            let playing = state.is_playing;
            drop(state);
            if playing {
                crate::drivers::audio::play_chime();
            }
        }
    }
}

pub fn draw_sync_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Sync) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Sync))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "[DEMO] BeaverSync - Malla de Dispositivos (nodos simulados)", is_active);

    let (nodes, total_cores, total_ram, sync_count, logs) = {
        let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
        (mgr.get_nodes().to_vec(), mgr.total_cluster_cores(), mgr.total_cluster_ram_mb(), mgr.sync_count, mgr.sync_logs.clone())
    };

    // 1. Barra Superior del Panel de Sincronización
    // [DEMO]: hasta la Fase 5 del plan (driver E1000 real + descubrimiento UDP) los nodos están codificados en mesh.rs
    Canvas::fill_rounded_rect(x + 10, y + 36, w - 20, 26, 6, 0xFEF3C7);
    Canvas::draw_rounded_rect(x + 10, y + 36, w - 20, 26, 6, 0xFCD34D);
    Canvas::draw_string_prop(x + 18, y + 41, "DEMO: nodos simulados - la red aun no transmite por la tarjeta E1000", Color::BEAVER_AMBER_DARK, None);
    let right_stats = format!("Syncs: #{} | Nodos: {}", sync_count, nodes.len() + 1);
    Canvas::draw_string_prop(x + w.saturating_sub(210), y + 41, &right_stats, Color::BEAVER_AMBER_DARK, None);

    // 2. Tarjetas de Dispositivos Conectados (Topología Multi-Dispositivo Estilo Genisys)
    let left_w = (w.saturating_sub(30)) * 55 / 100;
    Canvas::fill_rounded_rect(x + 10, y + 68, left_w, 185, 8, Color::WHITE);
    Canvas::draw_rounded_rect(x + 10, y + 68, left_w, 185, 8, Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 16, y + 74, "DISPOSITIVOS INTERCONECTADOS (UN SOLO SO):", Color::BEAVER_AMBER_DARK, None);

    // Dispositivo Local con icono vectorial
    Canvas::fill_rounded_rect(x + 16, y + 92, left_w.saturating_sub(12), 34, 6, Color::BEAVER_TITANIUM_BAR);
    draw_device_icon(x + 20, y + 96, DeviceKind::Pc, Color::BEAVER_AMBER);
    Canvas::draw_string_prop(x + 48, y + 96, "beaver-node0 (Este Equipo)", Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 48, y + 110, "Estado: Maestro Local | 4 Cores | 512 MB | Sinc: 100%", Color::BEAVER_EMERALD, None);

    // Dispositivos Remotos en la Malla
    for (i, node) in nodes.iter().enumerate().take(3) {
        let dy = y + 130 + i * 38;
        Canvas::fill_rounded_rect(x + 16, dy, left_w.saturating_sub(12), 34, 6, Color::BEAVER_TITANIUM_BAR);

        let kind = match node.name.as_str() {
            "beaver-laptop" => DeviceKind::Laptop,
            "beaver-server" => DeviceKind::Server,
            _ => DeviceKind::Pc,
        };
        draw_device_icon(x + 20, dy + 4, kind, Color::BEAVER_TEAL);

        let line1 = format!("{} ({})", node.name, node.device_type);
        Canvas::draw_string_prop(x + 48, dy + 4, &line1, Color::BEAVER_TEXT_DARK, None);
        let line2 = format!("IP: {} | {} Cores | {}MB | Lat: {}ms | Sinc: {}%",
            node.ip.to_string(), node.cores, node.ram_mb, node.latency_ms, node.sync_percent
        );
        Canvas::draw_string_prop(x + 48, dy + 19, &line2, Color::BEAVER_TEXT_MUTED, None);
    }

    // 3. Panel de Cómputo Agregado / Supercomputadora Malla (Derecha)
    let right_x = x + 15 + left_w;
    let right_w = w.saturating_sub(left_w + 25);
    Canvas::fill_rounded_rect(right_x, y + 68, right_w, 185, 8, Color::WHITE);
    Canvas::draw_rounded_rect(right_x, y + 68, right_w, 185, 8, Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(right_x + 12, y + 74, "PODER DE MALLA AGREGADO:", Color::BEAVER_TEAL, None);

    let cores_str = format!("* Cores Totales:  {} nucleos x86_64", total_cores);
    Canvas::draw_string_prop(right_x + 12, y + 96, &cores_str, Color::BEAVER_TEXT_DARK, None);

    let ram_gb = total_ram / 1024;
    let ram_dec = (total_ram % 1024) / 100;
    let ram_str = format!("* Memoria Malla:  {} MB ({}.{} GB)", total_ram, ram_gb, ram_dec);
    Canvas::draw_string_prop(right_x + 12, y + 116, &ram_str, Color::BEAVER_TEXT_DARK, None);

    Canvas::draw_string_prop(right_x + 12, y + 136, "* Auto-Sinc:      /disk/notas.txt", Color::BEAVER_EMERALD, None);
    Canvas::draw_string_prop(right_x + 12, y + 156, "* Replicacion:    Multi-nodo P2P", Color::BEAVER_TEXT_MUTED, None);
    Canvas::draw_string_prop(right_x + 12, y + 176, "* Cero Caidas:    Espejo LBA 16 Activo", Color::BEAVER_AMBER_DARK, None);

    // Botón Interactivo: "Sincronizar Malla Ahora"
    let sync_btn_y = y + 210;
    Canvas::fill_rounded_rect(right_x + 10, sync_btn_y, right_w.saturating_sub(20), 32, 6, Color::BEAVER_AMBER);
    Canvas::draw_string_prop(right_x + 20, sync_btn_y + 10, "Sincronizar Malla Ahora (DEMO)", Color::WHITE, None);

    // 4. Panel Inferior: Telemetría de Sincronización en Tiempo Real
    let feed_y = y + 260;
    let feed_h = h.saturating_sub(270);
    Canvas::fill_rounded_rect(x + 10, feed_y, w - 20, feed_h, 8, Color::SLATE_900);
    Canvas::draw_rounded_rect(x + 10, feed_y, w - 20, feed_h, 8, Color::SLATE_700);

    Canvas::draw_string(x + 18, feed_y + 6, "REGISTRO DE TELEMETRIA EN VIVO (EVENTOS BEAVERSYNC):", Color::SLATE_400, None);
    let mut log_y = feed_y + 24;
    for log in logs.iter().rev().take(4) {
        Canvas::draw_string(x + 18, log_y, log, Color::BEAVER_GREEN, None);
        log_y += 18;
    }
}

pub fn render_sync_window() {
    draw_sync_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Sync) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::present_rect(x, y, w, h);
    }
}

pub fn handle_sync_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::Sync).cloned();
    if let Some(w) = win {
        let left_w = (w.width.saturating_sub(30)) * 55 / 100;
        let right_x = wx + 15 + left_w;
        let right_w = w.width.saturating_sub(left_w + 25);
        let sync_btn_y = wy + 210;

        // Clic en el botón "Sincronizar Malla Ahora"
        if mx >= right_x + 10 && mx <= right_x + right_w.saturating_sub(10) && my >= sync_btn_y && my <= sync_btn_y + 32 {
            crate::drivers::mesh::trigger_mesh_sync();
            crate::drivers::audio::click_feedback();
            render_desktop();
        }
    }
}

// ============================================================================
// BeaverPhotos - Galería y Visor Gráfico de Imágenes (Lord OS Style)
// ============================================================================

pub fn draw_photos_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Photos) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Photos))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "BeaverPhotos - Galeria y Visor de Imagenes", is_active);

    // 1. Barra Lateral Izquierda (Colecciones)
    let sb_w = 125;
    Canvas::fill_rect(x + 1, y + 32, sb_w, h.saturating_sub(33), Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_line_v(x + 1 + sb_w, y + 32, h.saturating_sub(33), Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 10, y + 42, "BIBLIOTECA", Color::BEAVER_TEXT_MUTED, None);
    Canvas::fill_rounded_rect(x + 6, y + 58, sb_w - 12, 22, 6, 0xDBEAFE);
    draw_sidebar_icon(x + 10, y + 60, SidebarIconKind::Photos, Color::BEAVER_TEAL);
    Canvas::draw_string_prop(x + 32, y + 62, "Todas (6)", Color::BEAVER_TEXT_DARK, None);

    draw_sidebar_icon(x + 10, y + 86, SidebarIconKind::Home, Color::BEAVER_TEXT_MUTED);
    Canvas::draw_string_prop(x + 32, y + 88, "Paisajes", Color::BEAVER_TEXT_DARK, None);

    draw_sidebar_icon(x + 10, y + 112, SidebarIconKind::Mesh, Color::BEAVER_TEXT_MUTED);
    Canvas::draw_string_prop(x + 32, y + 114, "Malla P2P", Color::BEAVER_TEXT_DARK, None);

    // 2. Contenido Principal (Galería Cuadrícula)
    let main_x = x + sb_w + 12;
    let main_w = w.saturating_sub(sb_w + 20);

    Canvas::draw_string_prop(main_x, y + 42, "Galeria de ejemplo (miniaturas generadas por codigo)", Color::BEAVER_TEXT_DARK, None);
    // Nota: el control de zoom decorativo se retiró (no tenía funcionalidad). Se añadirá cuando exista visor real de BMP.

    Canvas::draw_line_h(main_x, y + 62, main_w, Color::BEAVER_BORDER_LIGHT);

    // 6 Miniaturas de Fotos Artísticas Procedurales (2 filas de 3 columnas)
    let thumb_w = 135;
    let thumb_h = 115;
    let thumbs_meta = [
        ("Event Horizon", 0x1E1B4B, 0x7C3AED, 0xF59E0B),
        ("Northern Lights", 0x064E3B, 0x0D9488, 0x6EE7B7),
        ("Silicon Dawn", 0x1E293B, 0xB45309, 0xFCD34D),
        ("Beaver River", 0x0C4A6E, 0x0284C7, 0xBAE6FD),
        ("Supernova 2026", 0x450A0A, 0xDC2626, 0xFDE047),
        ("Starfield Cluster", 0x020617, 0x38BDF8, 0xFFFFFF),
    ];

    for (i, (name, bg, mid, glow)) in thumbs_meta.iter().enumerate() {
        let col = i % 3;
        let row = i / 3;
        let tx = main_x + 6 + col * (thumb_w + 14);
        let ty = y + 72 + row * (thumb_h + 30);

        if ty + thumb_h <= y + h {
            // Marco de foto redondeado
            Canvas::fill_rounded_rect(tx, ty, thumb_w, thumb_h, 8, *bg);
            Canvas::draw_rounded_rect(tx, ty, thumb_w, thumb_h, 8, Color::BEAVER_BORDER_LIGHT);

            // Efecto gráfico procedural de la foto
            match i {
                0 => {
                    Canvas::fill_circle(tx + thumb_w / 2, ty + thumb_h / 2, 28, *mid);
                    Canvas::fill_circle(tx + thumb_w / 2, ty + thumb_h / 2, 18, *bg);
                    Canvas::draw_circle(tx + thumb_w / 2, ty + thumb_h / 2, 34, *glow);
                }
                1 => {
                    Canvas::fill_rounded_rect(tx + 4, ty + 20, thumb_w - 8, 30, 4, *mid);
                    Canvas::draw_line_h(tx + 8, ty + 30, thumb_w - 16, *glow);
                    Canvas::fill_rect(tx + 10, ty + thumb_h - 25, 45, 20, 0x022C22);
                    Canvas::fill_rect(tx + 45, ty + thumb_h - 35, 60, 30, 0x022C22);
                }
                2 => {
                    Canvas::fill_rounded_rect(tx + thumb_w / 2 - 20, ty + thumb_h / 2 - 20, 40, 40, 4, 0x0F172A);
                    Canvas::draw_line_h(tx + 10, ty + thumb_h / 2, thumb_w - 20, *mid);
                    Canvas::draw_line_v(tx + thumb_w / 2, ty + 10, thumb_h - 20, *mid);
                    Canvas::fill_circle(tx + thumb_w / 2, ty + thumb_h / 2, 8, *glow);
                }
                3 => {
                    Canvas::fill_rect(tx + 2, ty + thumb_h / 2, thumb_w - 4, thumb_h / 2 - 2, *mid);
                    Canvas::draw_line_h(tx + 10, ty + thumb_h / 2 + 10, thumb_w - 20, *glow);
                    Canvas::draw_line_h(tx + 25, ty + thumb_h / 2 + 20, thumb_w - 50, *glow);
                }
                4 => {
                    Canvas::fill_circle(tx + thumb_w / 2, ty + thumb_h / 2, 24, *mid);
                    Canvas::fill_circle(tx + thumb_w / 2, ty + thumb_h / 2, 12, *glow);
                    Canvas::draw_line_h(tx + 20, ty + thumb_h / 2, thumb_w - 40, Color::WHITE);
                    Canvas::draw_line_v(tx + thumb_w / 2, ty + 15, thumb_h - 30, Color::WHITE);
                }
                5 => {
                    let stars = [(20, 20), (45, 15), (70, 30), (100, 25), (35, 65), (85, 75), (115, 80), (60, 95)];
                    for &(sx, sy) in &stars {
                        Canvas::fill_circle(tx + sx, ty + sy, 2, *glow);
                    }
                    Canvas::fill_circle(tx + thumb_w / 2, ty + thumb_h / 2, 5, *mid);
                }
                _ => {}
            }

            // Pie de foto con tipografía proporcional
            Canvas::draw_string_prop(tx + 4, ty + thumb_h + 4, name, Color::BEAVER_TEXT_DARK, None);
            Canvas::draw_string_prop(tx + 4, ty + thumb_h + 16, "1920x1080 TrueColor", Color::BEAVER_TEXT_MUTED, None);
        }
    }
}

pub fn handle_photos_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::Photos).cloned();
    if let Some(_w) = win {
        let sb_w = 125;
        let main_x = wx + sb_w + 12;
        let thumb_w = 135;
        let thumb_h = 115;

        for i in 0..6 {
            let col = i % 3;
            let row = i / 3;
            let tx = main_x + 6 + col * (thumb_w + 14);
            let ty = wy + 72 + row * (thumb_h + 30);
            if mx >= tx && mx <= tx + thumb_w && my >= ty && my <= ty + thumb_h {
                crate::drivers::audio::click_feedback();
                if let Some(mut state) = GUI_TERMINAL.try_lock() {
                    let photo_names = ["Event Horizon", "Northern Lights", "Silicon Dawn", "Beaver River", "Supernova 2026", "Starfield Cluster"];
                    state.lines.push(format!("[FOTOS][DEMO] '{}' es una miniatura generada por codigo; no existe archivo en disco.", photo_names[i]));
                }
                render_desktop();
                return;
            }
        }
    }
}

// ============================================================================
// BeaverCalendar - Agenda y Calendario Integrado con Chip RTC
// ============================================================================

fn month_name(month: u8) -> &'static str {
    match month {
        1 => "Enero",
        2 => "Febrero",
        3 => "Marzo",
        4 => "Abril",
        5 => "Mayo",
        6 => "Junio",
        7 => "Julio",
        8 => "Agosto",
        9 => "Septiembre",
        10 => "Octubre",
        11 => "Noviembre",
        12 => "Diciembre",
        _ => "Mes",
    }
}

fn days_in_month(month: u8, year: u16) -> u8 {
    crate::drivers::timer::days_in_month(month, year)
}

pub fn draw_calendar_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Calendar) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Calendar))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "BeaverCalendar - Agenda y Calendario RTC", is_active);

    // Leer fecha y hora LOCAL (RTC en UTC + desfase de zona horaria)
    let rtc = crate::drivers::timer::local_datetime();

    // 1. Cabecera del Calendario
    let cal_title = format!("{} {}", month_name(rtc.month), rtc.year);
    Canvas::draw_string_prop(x + 18, y + 42, &cal_title, Color::BEAVER_TEXT_DARK, None);

    // Selector de vistas estilo píldora [ Dia ] [ Semana ] [ Mes (Activo) ]
    let pill_x = x + w.saturating_sub(190);
    Canvas::fill_rounded_rect(pill_x, y + 38, 175, 24, 6, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_string_prop(pill_x + 10, y + 43, "Dia", Color::BEAVER_TEXT_MUTED, None);
    Canvas::draw_string_prop(pill_x + 50, y + 43, "Semana", Color::BEAVER_TEXT_MUTED, None);
    Canvas::fill_rounded_rect(pill_x + 115, y + 40, 56, 20, 4, Color::WHITE);
    Canvas::draw_string_prop(pill_x + 125, y + 43, "Mes", Color::BEAVER_TEXT_DARK, None);

    Canvas::draw_line_h(x + 15, y + 68, w.saturating_sub(30), Color::BEAVER_BORDER_LIGHT);

    // 2. Días de la Semana
    let day_names = ["Lun", "Mar", "Mie", "Jue", "Vie", "Sab", "Dom"];
    let cell_w = (w.saturating_sub(36)) / 7;
    for (i, dname) in day_names.iter().enumerate() {
        let dx = x + 18 + i * cell_w;
        let col = if i >= 5 { Color::BEAVER_AMBER_DARK } else { Color::BEAVER_TEXT_MUTED };
        Canvas::draw_string_prop(dx + cell_w / 2 - 10, y + 74, dname, col, None);
    }

    // 3. Cuadrícula de Días del Mes. Columna inicial calculada con el algoritmo de Sakamoto
    //    day_of_week: 0=Domingo..6=Sábado  ->  columna con Lunes=0..Domingo=6
    let dow_first = crate::drivers::timer::day_of_week(rtc.year, rtc.month, 1) as usize;
    let start_col = (dow_first + 6) % 7;
    let total_days = days_in_month(rtc.month, rtc.year);
    let today = rtc.day;

    let mut row = 0;
    let mut col = start_col;
    for day_num in 1..=total_days {
        let cx = x + 18 + col * cell_w + cell_w / 2;
        let cy = y + 96 + row * 24;

        if day_num == today {
            Canvas::fill_circle(cx, cy + 6, 11, Color::BEAVER_RUBY);
            let num_str = format!("{}", day_num);
            let tw = Canvas::string_width_prop(&num_str);
            Canvas::draw_string_prop(cx.saturating_sub(tw / 2), cy + 1, &num_str, Color::WHITE, None);
        } else {
            let num_str = format!("{}", day_num);
            let tw = Canvas::string_width_prop(&num_str);
            Canvas::draw_string_prop(cx.saturating_sub(tw / 2), cy + 1, &num_str, Color::BEAVER_TEXT_DARK, None);
        }

        col += 1;
        if col == 7 {
            col = 0;
            row += 1;
        }
    }

    // 4. Panel de Eventos Programados de Beaver OS
    let events_y = y + 250;
    Canvas::fill_rounded_rect(x + 15, events_y, w.saturating_sub(30), h.saturating_sub(262), 8, Color::WHITE);
    Canvas::draw_rounded_rect(x + 15, events_y, w.saturating_sub(30), h.saturating_sub(262), 8, Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 25, events_y + 8, "[DEMO] AGENDA DE EJEMPLO (no hay tareas programadas reales):", Color::BEAVER_AMBER_DARK, None);

    Canvas::fill_circle(x + 32, events_y + 32, 4, Color::BEAVER_EMERALD);
    Canvas::draw_string_prop(x + 44, events_y + 26, "09:00 - Sincronizacion Automatica Malla P2P (/net)", Color::BEAVER_TEXT_DARK, None);

    Canvas::fill_circle(x + 32, events_y + 52, 4, Color::BEAVER_AMBER);
    Canvas::draw_string_prop(x + 44, events_y + 46, "14:30 - Verificacion de Paridad BeaverFS (Espejo LBA 16)", Color::BEAVER_TEXT_DARK, None);

    Canvas::fill_circle(x + 32, events_y + 72, 4, Color::BEAVER_BLUE);
    Canvas::draw_string_prop(x + 44, events_y + 66, "18:00 - Auditoria y Respaldo Slot A/B Anti-Brickeo", Color::BEAVER_TEXT_DARK, None);
}

pub fn handle_calendar_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::Calendar).cloned();
    if let Some(w) = win {
        let pill_x = wx + w.width.saturating_sub(190);
        if mx >= pill_x && mx <= pill_x + 175 && my >= wy + 38 && my <= wy + 62 {
            crate::drivers::audio::click_feedback();
            render_desktop();
        }
    }
}

// ============================================================================
// Spotlight / Quick Search Modal Overlay
// ============================================================================

pub fn draw_spotlight_overlay(screen_w: usize, _screen_h: usize) {
    let spot = SPOTLIGHT.lock();
    if !spot.is_open {
        return;
    }

    let modal_w = 480;
    let modal_h = 240;
    let modal_x = (screen_w.saturating_sub(modal_w)) / 2;
    let modal_y = 120;

    // Sombra suave proyectada
    Canvas::fill_rounded_rect(modal_x + 6, modal_y + 6, modal_w, modal_h, 14, 0x070B16);
    // Fondo de cristal
    Canvas::fill_rounded_rect(modal_x, modal_y, modal_w, modal_h, 14, 0x0B1222);
    Canvas::draw_rounded_rect(modal_x, modal_y, modal_w, modal_h, 14, 0x475569);

    // Cabecera: accesos rápidos (aún no hay entrada de texto ni búsqueda real)
    Canvas::fill_rounded_rect(modal_x + 14, modal_y + 14, modal_w - 28, 36, 8, 0x1E293B);
    draw_sidebar_icon(modal_x + 22, modal_y + 24, SidebarIconKind::Search, Color::BEAVER_TEAL_BRIGHT);
    Canvas::draw_string_prop(modal_x + 46, modal_y + 24, "Accesos rapidos (busqueda por texto: pendiente)", Color::WHITE, None);

    Canvas::draw_line_h(modal_x + 14, modal_y + 58, modal_w - 28, Color::SLATE_700);

    // Lista fija de aplicaciones
    let results = [
        ("BeaverPhotos", "Galeria de ejemplo con miniaturas generadas", WindowId::Photos),
        ("BeaverCalendar", "Calendario mensual con hora local del RTC", WindowId::Calendar),
        ("BeaverFiles", "Explorador VFS (/disk, /net, /home, /sys)", WindowId::Files),
        ("BeaverSync", "[DEMO] Malla de dispositivos (nodos simulados)", WindowId::Sync),
    ];

    for (i, (title, desc, _win_id)) in results.iter().enumerate() {
        let ry = modal_y + 66 + i * 40;
        Canvas::fill_rounded_rect(modal_x + 14, ry, modal_w - 28, 34, 6, 0x151E2E);
        Canvas::draw_string_prop(modal_x + 24, ry + 4, title, Color::BEAVER_AMBER, None);
        Canvas::draw_string_prop(modal_x + 24, ry + 18, desc, Color::SLATE_400, None);
    }
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

    // Pantalla interior oscura de terminal estilo Lord OS / macOS
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

    // Línea de entrada con cursor parpadeante
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
        Canvas::present_rect(x, y, w, h);
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

    // 1. Fondo de terminal profundo
    Canvas::fill_rect(0, 0, width, height, 0x0A0F1D);

    // 2. Barra de título TTY superior
    Canvas::fill_rect(0, 0, width, 26, Color::SLATE_800);
    Canvas::draw_rect(0, 0, width, 26, Color::SLATE_700);

    Canvas::draw_string(14, 5, "[BEAVER TTY1 - CONSOLA BARE-METAL]", Color::BEAVER_ACCENT_CYAN, None);
    Canvas::draw_string(370, 5, "Escribe 'desktop' para GUI | ESC = alternar", Color::BEAVER_YELLOW, None);

    let (curr_uid, _, curr_user) = SCHEDULER.lock().current_credentials();
    let uptime = get_uptime_seconds();
    let status_str = format!("{} (UID: {}) | Up: {}s", curr_user, curr_uid, uptime);
    let status_x = width.saturating_sub(status_str.len() * 8 + 16);
    Canvas::draw_string(status_x, 5, &status_str, Color::WHITE, None);

    // 3. Renderizado de líneas de texto
    let prompt_symbol = if curr_uid == 0 { '#' } else { '$' };
    let state = GUI_TERMINAL.lock();
    let total_lines = state.lines.len();
    let max_visible = 36;
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

    // 4. Prompt y cursor activo
    let cwd = crate::shell::get_current_dir();
    let prompt_str = format!("{}@beaver:{}{} {}_", curr_user, cwd, prompt_symbol, state.input_buffer);
    Canvas::draw_string(16, y.min(height - 24), &prompt_str, Color::BEAVER_GREEN, None);

    // 5. Cursor de ratón moderno interactivo
    Canvas::invalidate_cursor();
    let (mx, my) = {
        let m = crate::drivers::mouse::MOUSE.lock();
        (m.state.x as usize, m.state.y as usize)
    };
    Canvas::draw_cursor_at(mx, my);

    // 6. Presentación atómica de doble búfer (RAM -> LFB)
    Canvas::present();
}

pub fn handle_gui_char(ch: char) {
    // Tecla ESC: Alternador rápido entre Desktop y FullScreenCli
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

    let (is_desktop, is_note_active) = {
        let is_desk = GUI_TERMINAL.lock().mode == GuiViewMode::Desktop;
        let is_note = WINDOW_MGR.lock().is_active(WindowId::Note);
        (is_desk, is_note)
    };

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
                            state.lines.push(String::from("[SISTEMA] Has salido del entorno multi-ventana a la Consola TTY1 (1024x768)."));
                            state.lines.push(String::from("[SISTEMA] Escribe 'desktop' o presiona ESC para volver a la interfaz grafica."));
                            state.mode = GuiViewMode::FullScreenCli;
                            mode_changed = true;
                        } else {
                            state.lines.push(String::from("[SISTEMA] Ya te encuentras en la consola principal TTY1. Escribe 'desktop' para GUI."));
                        }
                    }
                    "gui-off" => {
                        let mut state = GUI_TERMINAL.lock();
                        state.lines.push(String::from("[SISTEMA] En arquitectura x86_64 bare-metal, la consola nativa de alto rendimiento es TTY1"));
                        state.lines.push(String::from("         (1024x768 TrueColor). Conmutando a TTY1 en pantalla completa..."));
                        state.lines.push(String::from("[SISTEMA] Escribe 'desktop' o presiona ESC para volver a la interfaz grafica."));
                        state.mode = GuiViewMode::FullScreenCli;
                        mode_changed = true;
                    }
                    "clear" => {
                        GUI_TERMINAL.lock().lines.clear();
                    }
                    _ => {
                        // Conexión unificada con el motor completo de comandos de Beaversoft Shell
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
                match mode {
                    GuiViewMode::Desktop => render_terminal_window(),
                    GuiViewMode::FullScreenCli => render_fullscreen_cli(),
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
    let prev_btn = unsafe { PREV_LEFT_BTN };
    let left_pressed_now = state.left_button && !prev_btn;
    let left_released_now = !state.left_button && prev_btn;
    unsafe {
        PREV_LEFT_BTN = state.left_button;
    }

    if mode == GuiViewMode::Desktop {
        if left_pressed_now {
            // Comprobar primero si el clic impactó los botones de ventana (Cerrar, Minimizar, Maximizar)
            if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                if let Some((win_id, btn)) = mgr.hit_test_buttons(mx, my) {
                    crate::drivers::audio::click_feedback();
                    match btn {
                        crate::gui::window::WindowButton::Close | crate::gui::window::WindowButton::Minimize => {
                            mgr.minimize(win_id);
                        }
                        crate::gui::window::WindowButton::Maximize => {
                            let (w, h) = {
                                let vbe = VBE.lock();
                                (vbe.width, vbe.height)
                            };
                            mgr.restore_or_maximize(win_id, w, h);
                        }
                    }
                    drop(mgr);
                    render_desktop();
                    return;
                }

                // Comprobar si el clic fue en la barra de título de alguna ventana para arrastrarla
                if let Some(win_id) = mgr.hit_test_titlebar(mx, my) {
                    mgr.start_drag(win_id, mx, my);
                    drop(mgr);
                    render_desktop();
                    return;
                } else if let Some(win_id) = mgr.hit_test_window(mx, my) {
                    // Clic dentro de la ventana: traer al frente (Z-Order)
                    mgr.bring_to_front(win_id);
                    let win_coords = mgr.get_window(win_id).map(|w| (w.x, w.y));
                    drop(mgr);

                    // Si es BeaverNote, verificar interacción con botones o texto
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

                    render_desktop();
                    return;
                }
            }
            handle_desktop_click(mx, my);
        } else if state.left_button {
            // Manteniendo presionado el botón izquierdo: procesar arrastre dinámico
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
            // Soltar el botón izquierdo: finalizar arrastre
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
            // Movimiento normal sin botones (hovering rápido de cursor)
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
    // 0. Si Spotlight está abierto, evaluar clics en el buscador
    {
        let is_spot_open = SPOTLIGHT.lock().is_open;
        if is_spot_open {
            let modal_w = 480;
            let modal_h = 240;
            let modal_x = (1024_usize.saturating_sub(modal_w)) / 2;
            let modal_y = 120;

            if mx >= modal_x && mx <= modal_x + modal_w && my >= modal_y && my <= modal_y + modal_h {
                for i in 0..4 {
                    let ry = modal_y + 66 + i * 40;
                    if my >= ry && my <= ry + 34 {
                        crate::drivers::audio::click_feedback();
                        let target_win = match i {
                            0 => WindowId::Photos,
                            1 => WindowId::Calendar,
                            2 => WindowId::Files,
                            3 => WindowId::Sync,
                            _ => WindowId::Files,
                        };
                        SPOTLIGHT.lock().is_open = false;
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(target_win);
                        }
                        render_desktop();
                        return;
                    }
                }
                return;
            } else {
                // Clic fuera del modal: cerrar Spotlight
                SPOTLIGHT.lock().is_open = false;
                render_desktop();
                return;
            }
        }
    }

    // 1. Clic en la barra superior (Top Bar)
    if my <= 28 {
        // Clic en botón Buscador / Spotlight (x: 390..475)
        if mx >= 390 && mx <= 475 {
            crate::drivers::audio::click_feedback();
            let mut spot = SPOTLIGHT.lock();
            spot.is_open = !spot.is_open;
            drop(spot);
            render_desktop();
            return;
        }

        // Clic en botón Conmutador de Tema Día / Noche (x: 704..790)
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

    // 2. Clic en la barra Dock (Launcher inferior de 10 aplicaciones)
    let dock_w = 880;
    let dock_x = (1024_usize.saturating_sub(dock_w)) / 2; // 72
    let dock_y = 768 - 62; // 706

    if my >= dock_y + 6 && my <= dock_y + 44 {
        for i in 0..10 {
            let btn_x = dock_x + 8 + i * 86;
            if mx >= btn_x && mx <= btn_x + 82 {
                crate::drivers::audio::click_feedback();
                match i {
                    0 => {
                        // Consola (Terminal)
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Terminal);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    1 => {
                        // Monitor
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Monitor);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    2 => {
                        // Archivos (BeaverFiles)
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Files);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    3 => {
                        // BeaverSync
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Sync);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    4 => {
                        // Audio (Player)
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Player);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    5 => {
                        // BeaverPhotos -> NUEVO!
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Photos);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    6 => {
                        // BeaverCalendar -> NUEVO!
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Calendar);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    7 => {
                        // BeaverNote
                        if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                            mgr.unminimize_and_focus(WindowId::Note);
                            drop(mgr);
                            render_desktop();
                        }
                        return;
                    }
                    8 => {
                        // Instalar
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
                        // Salir / TTY1
                        if let Some(mut state) = GUI_TERMINAL.try_lock() {
                            state.mode = GuiViewMode::FullScreenCli;
                            state.lines.push(String::from("[SISTEMA] Has salido del entorno multi-ventana a la Consola TTY1 (1024x768 TrueColor)."));
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
    // Si hace clic en la barra superior (y <= 26), vuelve al escritorio
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



