// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver Smart Hub - 10-Foot UI for Smart TVs, Touch Displays & Kiosks
//              Real Hardware Profile, Mesh, VFS & Resource Telemetry (Zero Mocks)
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::sync::SpinMutex;
use crate::gui::canvas::{Canvas, Color};
use crate::drivers::timer::get_uptime_seconds;
use crate::memory::heap::HEAP_ALLOCATOR;
use crate::drivers::keyboard::{KEY_UP_ARROW, KEY_DOWN_ARROW, KEY_LEFT_ARROW, KEY_RIGHT_ARROW};

pub const TOTAL_TILES: usize = 6;

pub struct SmartHubState {
    pub focused_tile: usize,       // 0..5 (3 columnas x 2 filas)
    pub last_action: String,
}

pub static SMART_HUB: SpinMutex<SmartHubState> = SpinMutex::new(SmartHubState {
    focused_tile: 0,
    last_action: String::new(),
});

/// Conmuta inmediatamente la interfaz al modo Beaver Smart Hub
pub fn enter_smart_hub() {
    {
        let mut term = crate::gui::desktop::GUI_TERMINAL.lock();
        term.mode = crate::gui::desktop::GuiViewMode::SmartHub;
    }
    render_smart_hub();
}

/// Dibuja la interfaz completa de Smart Hub con doble búfer TrueColor usando telemetría real
/// Inspirada en la consola táctil interestelar de la nave Starship Avalon (Passengers 2016)
pub fn render_smart_hub() {
    let (width, height) = Canvas::screen_dimensions();

    // 1. Fondo Oscuro Obsidian Espacial (#040812)
    Canvas::fill_rect(0, 0, width, height, Color::AVALON_BG);

    // Rejilla sutil táctica cian
    for x in (20..width).step_by(32) {
        for y in (70..height).step_by(32) {
            Canvas::put_pixel(x, y, 0x0A2B45);
        }
    }

    // 2. Barra Superior de Estado y Cabecera Institucional Avalon Glass
    Canvas::fill_rect(0, 0, width, 65, Color::AVALON_GLASS);
    Canvas::draw_line_h(0, 64, width, Color::AVALON_CYAN);

    // Emblema Beaver en verde menta bioluminiscente
    Canvas::fill_rounded_rect(30, 14, 36, 36, 8, Color::AVALON_MINT);
    Canvas::draw_string(42, 22, "B", Color::AVALON_BG, None);

    // Título Principal y Pastilla de Modo
    Canvas::draw_string_prop(76, 16, "BEAVER SMART HUB // AVALON HUD", Color::WHITE, None);
    Canvas::fill_rounded_rect(360, 18, 230, 24, 12, 0x092238);
    Canvas::draw_rounded_rect(360, 18, 230, 24, 12, Color::AVALON_MINT);
    Canvas::draw_string_prop(372, 21, "[CONSOLA TACTIL PASSENGERS]", Color::AVALON_MINT, None);

    // Información en Vivo Real (Uptime, Red Mesh, IP Real)
    let uptime = get_uptime_seconds();
    let hours = uptime / 3600;
    let mins = (uptime % 3600) / 60;
    let secs = uptime % 60;
    let clock_str = format!("{:02}:{:02}:{:02}", hours, mins, secs);

    let mesh_count = crate::drivers::mesh::MESH_MANAGER.lock().online_count();
    let local_ip = crate::drivers::net::NET_INTERFACE.lock().ip_address.to_string();
    let mesh_hdr = format!("Malla P2P: :4242 [{} Nodos]", mesh_count);
    let ip_hdr = format!("IP Local: {} | Cero Nube", local_ip);

    Canvas::draw_string_prop(width - 320, 16, &mesh_hdr, Color::AVALON_MINT, None);
    Canvas::draw_string_prop(width - 130, 16, &clock_str, Color::AVALON_GOLD, None);
    Canvas::draw_string_prop(width - 320, 38, &ip_hdr, Color::AVALON_TEXT, None);

    // 3. Renderizado de la Cuadrícula de 6 Mosaicos Inteligentes (3 Cols x 2 Rows)
    let (focused, last_act) = {
        let state = SMART_HUB.lock();
        (state.focused_tile, state.last_action.clone())
    };

    let profile = crate::drivers::profile::detect_device_profile();
    let (vbe_active, vbe_w, vbe_h) = {
        let v = crate::drivers::vbe::VBE.lock();
        (v.is_active, v.width, v.height)
    };
    let mesh_nodes: Vec<(String, String)> = {
        let m = crate::drivers::mesh::MESH_MANAGER.lock();
        m.get_nodes()
            .iter()
            .take(3)
            .map(|n| (n.name.clone(), n.ip.to_string()))
            .collect()
    };
    let disk_entries: Vec<(String, bool, usize)> = {
        let vfs = crate::fs::vfs::VFS.lock();
        match vfs.list_dir("/disk", 0, 0) {
            Ok(list) => list.into_iter().take(3).map(|(n, d, sz, _, _, _)| (n, d, sz)).collect(),
            Err(_) => Vec::new(),
        }
    };

    let start_x = 42;
    let start_y = 85;
    let tile_w = 296;
    let tile_h = 255;
    let gap_x = 22;
    let gap_y = 20;

    for idx in 0..TOTAL_TILES {
        let col = idx % 3;
        let row = idx / 3;
        let tx = start_x + col * (tile_w + gap_x);
        let ty = start_y + row * (tile_h + gap_y);
        let is_focused = idx == focused;

        draw_smart_tile(
            idx,
            tx,
            ty,
            tile_w,
            tile_h,
            is_focused,
            &profile,
            (vbe_active, vbe_w, vbe_h),
            &mesh_nodes,
            &disk_entries,
        );
    }

    // 4. Barra Inferior de Acción Táctil y Teclas de Control Remoto
    Canvas::fill_rect(0, 620, width, 148, Color::AVALON_GLASS);
    Canvas::draw_line_h(0, 620, width, Color::AVALON_CYAN);

    // Línea de Estado / Feedback de Última Acción
    let status_text = if last_act.is_empty() {
        String::from("Consola Avalon lista. Telemetria real activa. Toca o usa D-Pad.")
    } else {
        last_act
    };
    Canvas::fill_circle(45, 638, 5, Color::AVALON_MINT);
    Canvas::draw_string_prop(60, 630, &status_text, Color::AVALON_TEXT, None);

    // Botones Táctiles de Gran Escala en la Parte Inferior
    draw_touch_button(42, 655, 215, 48, "[ ESC ] MODO ESCRITORIO", Color::AVALON_CYAN, 0x071D33);
    draw_touch_button(275, 655, 215, 48, "[ P ] SONDEAR HARDWARE", Color::AVALON_MINT, 0x062822);
    draw_touch_button(508, 655, 215, 48, "[ A ] AUDIO BEAVER", Color::AVALON_GOLD, 0x2A1C06);
    draw_touch_button(741, 655, 240, 48, "[ M ] ANUNCIAR MESH :4242", Color::AVALON_CYAN, 0x071D33);

    // Indicaciones de Navegación D-Pad
    Canvas::draw_string_prop(42, 715, "Controles de Sala:  [FLECHAS / D-PAD / WASD] Mover Foco   |   [ENTER / ESPACIO] Abrir   |   [ESC] Escritorio", Color::AVALON_ACCENT_DIM, None);
    Canvas::draw_string_prop(42, 735, "Beaver OS v0.9.5 | Starship Avalon Edition (Passengers 2016) | Desarrollado por Beaversoft", 0x0E3C60, None);

    // 5. Dibujar puntero de ratón o toque
    let (mx, my) = {
        let m = crate::drivers::mouse::MOUSE.lock();
        (m.state.x as usize, m.state.y as usize)
    };
    Canvas::draw_cursor_at(mx, my);

    // 6. Presentación atómica al framebuffer físico
    Canvas::present();
}

/// Dibuja un mosaico inteligente individual con telemetría real del sistema
#[allow(clippy::too_many_arguments)]
fn draw_smart_tile(
    idx: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    is_focused: bool,
    profile: &crate::drivers::profile::HardwareProfile,
    vbe_info: (bool, usize, usize),
    mesh_nodes: &[(String, String)],
    disk_entries: &[(String, bool, usize)],
) {
    let (bg_color, border_color, is_f) = if is_focused {
        (0x0A223B, Color::AVALON_MINT, true)
    } else {
        (Color::AVALON_GLASS, Color::AVALON_BORDER, false)
    };

    Canvas::fill_rounded_rect(x, y, w, h, 14, bg_color);

    if is_f {
        Canvas::draw_rounded_rect(x, y, w, h, 14, border_color);
        Canvas::draw_rounded_rect(x + 1, y + 1, w - 2, h - 2, 13, border_color);
        Canvas::draw_hud_corners(x, y, w, h, 12, Color::AVALON_CYAN);
        Canvas::fill_rounded_rect(x + w - 105, y + 12, 92, 20, 10, Color::AVALON_MINT);
        Canvas::draw_string_prop(x + w - 98, y + 14, "ENFOCADO", Color::AVALON_BG, None);
    } else {
        Canvas::draw_rounded_rect(x, y, w, h, 14, border_color);
    }

    match idx {
        0 => {
            // [PERFIL DE HARDWARE REAL]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, Color::BEAVER_GREEN);
            Canvas::draw_string(x + 23, y + 20, "H", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "PERFIL DE HARDWARE", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, profile.source, Color::BEAVER_GREEN, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Clase Detectada:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 88, profile.class.as_str(), Color::BEAVER_GREEN, None);

            let cores_str = format!("CPU SMP: {} nucleos activos", profile.cpu_cores);
            Canvas::draw_string_prop(x + 18, y + 120, &cores_str, Color::WHITE, None);

            let cec_str = if profile.has_cec { "HDMI-CEC: DETECTADO" } else { "HDMI-CEC: No detectado" };
            let touch_str = if profile.has_touch { "Tactil I2C: DETECTADO" } else { "Tactil I2C: No detectado" };
            Canvas::draw_string_prop(x + 18, y + 150, cec_str, if profile.has_cec { Color::BEAVER_GREEN } else { Color::SLATE_400 }, None);
            Canvas::draw_string_prop(x + 18, y + 174, touch_str, if profile.has_touch { Color::BEAVER_GREEN } else { Color::SLATE_400 }, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> ENTER o Toca para re-sondear", Color::SLATE_400, None);
        }
        1 => {
            // [CENTRO MULTIMEDIA & DISPLAY]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, Color::BEAVER_PURPLE);
            Canvas::draw_string(x + 23, y + 20, "M", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "CENTRO MULTIMEDIA", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "Audio & Display Nativo", Color::BEAVER_PURPLE, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Salida Grafica Real:", Color::SLATE_200, None);
            let gfx_str = if vbe_info.0 {
                format!("VBE {}x{} TrueColor", vbe_info.1, vbe_info.2)
            } else {
                String::from("Sin framebuffer VBE")
            };
            Canvas::draw_string_prop(x + 18, y + 88, &gfx_str, Color::WHITE, None);

            Canvas::draw_string_prop(x + 18, y + 118, "Controlador HDMI-CEC:", Color::SLATE_200, None);
            let cec_online = crate::drivers::cec::HDMI_CEC.lock().is_online;
            let cec_status = if cec_online { "CEC en linea" } else { "CEC inactivo (sin bus)" };
            Canvas::draw_string_prop(x + 18, y + 138, cec_status, Color::BEAVER_TEAL_BRIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 168, "Aplicacion Asociada:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 188, "BeaverAudio / BeaverPhotos", Color::BEAVER_AMBER_LIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> ENTER o Toca para abrir Audio", Color::SLATE_400, None);
        }
        2 => {
            // [RED DE MALLA BEAVERMESH P2P REAL]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, Color::BEAVER_ACCENT_CYAN);
            Canvas::draw_string(x + 23, y + 20, "N", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "MALLA BEAVERMESH", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "Interconexion P2P :4242", Color::BEAVER_ACCENT_CYAN, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Nodos Descubiertos:", Color::SLATE_200, None);
            if mesh_nodes.is_empty() {
                Canvas::draw_string_prop(x + 18, y + 92, "-- Sin nodos remotos --", Color::SLATE_400, None);
                Canvas::draw_string_prop(x + 18, y + 114, "Presiona [ M ] para anunciar", Color::SLATE_400, None);
            } else {
                for (i, (name, ip)) in mesh_nodes.iter().enumerate() {
                    let line = format!("- {} ({})", name, ip);
                    Canvas::draw_string_prop(x + 18, y + 90 + i * 22, &line, Color::WHITE, None);
                }
            }

            Canvas::draw_string_prop(x + 18, y + 165, "Privacidad Local:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 185, "Red LAN directa (Cero Nube)", Color::BEAVER_GREEN, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> ENTER o Toca para BeaverSync", Color::SLATE_400, None);
        }
        3 => {
            // [MONITOR DE SALUD & RENDIMIENTO REAL]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, Color::BEAVER_AMBER);
            Canvas::draw_string(x + 23, y + 20, "S", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "SISTEMA & HARDWARE", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "Supervisor Anti-Crash", Color::BEAVER_AMBER, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            let (total, used, free, _, _) = HEAP_ALLOCATOR.stats();
            let used_pct = HEAP_ALLOCATOR.usage_percent();
            let free_pct = 100usize.saturating_sub(used_pct as usize);
            let ram_str = format!("Heap Usado: {} / {} KiB", used / 1024, total / 1024);
            let free_str = format!("Heap Libre: {} KiB ({}%)", free / 1024, free_pct);

            Canvas::draw_string_prop(x + 18, y + 68, "Uso de Memoria Kernel:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 90, &ram_str, Color::BEAVER_AMBER_LIGHT, None);
            Canvas::draw_string_prop(x + 18, y + 112, &free_str, Color::BEAVER_GREEN, None);

            Canvas::draw_string_prop(x + 18, y + 145, "Resiliencia de Disco:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 165, "BeaverFS Doble Slot A/B", Color::BEAVER_TEAL_BRIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> ENTER o Toca para Monitor", Color::SLATE_400, None);
        }
        4 => {
            // [ARCHIVOS REALES EN /disk]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, 0xF43F5E);
            Canvas::draw_string(x + 23, y + 20, "D", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "ARCHIVOS EN /disk", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "VFS / BeaverFS Real", 0xF43F5E, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Entradas en /disk:", Color::SLATE_200, None);
            if disk_entries.is_empty() {
                Canvas::draw_string_prop(x + 18, y + 92, "-- Sin entradas en /disk --", Color::SLATE_400, None);
            } else {
                for (i, (name, is_dir, sz)) in disk_entries.iter().enumerate() {
                    let line = if *is_dir {
                        format!("[DIR] {}", name)
                    } else {
                        format!("{} ({} B)", name, sz)
                    };
                    Canvas::draw_string_prop(x + 18, y + 90 + i * 22, &line, Color::WHITE, None);
                }
            }

            Canvas::draw_string_prop(x + 18, y + 165, "Almacenamiento Local:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 185, "Lectura directa de VFS", Color::BEAVER_AMBER_LIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> ENTER o Toca para BeaverNote", Color::SLATE_400, None);
        }
        5 => {
            // [TERMINAL Y RING 3 SYSCALLS]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, 0x6366F1);
            Canvas::draw_string(x + 23, y + 20, "T", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "CONSOLA & SYSCALLS", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "Ring 3 Aislado MSR", 0x6366F1, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Modo de Privilegio:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 90, "Espacio de Usuario CPL=3", Color::BEAVER_GREEN, None);

            Canvas::draw_string_prop(x + 18, y + 120, "Pasarela Rapida:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 140, "IA32_LSTAR (0xC0000082)", Color::BEAVER_ACCENT_CYAN, None);

            Canvas::draw_string_prop(x + 18, y + 175, "Prompt Interactivo: root@beaver:/#", Color::WHITE, None);
            Canvas::draw_string_prop(x + 18, y + 232, ">> ENTER o Toca para Terminal", Color::SLATE_400, None);
        }
        _ => {}
    }
}

/// Dibuja un botón táctil redondeado de gran formato
fn draw_touch_button(x: usize, y: usize, w: usize, h: usize, label: &str, border_color: u32, bg_color: u32) {
    Canvas::fill_rounded_rect(x, y, w, h, 10, bg_color);
    Canvas::draw_rounded_rect(x, y, w, h, 10, border_color);
    Canvas::draw_string_prop(x + 14, y + 16, label, border_color, None);
}

/// Maneja la pulsación de teclas físicas o comandos D-Pad de control remoto
pub fn handle_key(ch: char) -> bool {
    let mut state = SMART_HUB.lock();
    match ch {
        KEY_LEFT_ARROW | 'a' | 'A' | 'h' => {
            if state.focused_tile % 3 > 0 {
                state.focused_tile -= 1;
            } else {
                state.focused_tile += 2;
            }
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        KEY_RIGHT_ARROW | 'd' | 'D' | 'l' => {
            if state.focused_tile % 3 < 2 {
                state.focused_tile += 1;
            } else {
                state.focused_tile -= 2;
            }
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        KEY_UP_ARROW | 'w' | 'W' | 'k' => {
            if state.focused_tile >= 3 {
                state.focused_tile -= 3;
            } else {
                state.focused_tile += 3;
            }
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        KEY_DOWN_ARROW | 's' | 'S' | 'j' => {
            if state.focused_tile < 3 {
                state.focused_tile += 3;
            } else {
                state.focused_tile -= 3;
            }
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        '\t' => {
            state.focused_tile = (state.focused_tile + 1) % TOTAL_TILES;
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        '\n' | ' ' => {
            crate::drivers::audio::click_feedback();
            let cur = state.focused_tile;
            execute_tile_action(&mut state, cur);
            drop(state);
            true
        }
        'p' | 'P' => {
            let p = crate::drivers::profile::detect_device_profile();
            state.last_action = format!("[HARDWARE] {} ({})", p.class.as_str(), p.source);
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        'm' | 'M' => {
            state.last_action = String::from("[BEAVERMESH] Anuncio P2P emitido en puerto :4242.");
            crate::drivers::audio::click_feedback();
            drop(state);
            crate::drivers::mesh::send_mesh_announce_now();
            render_smart_hub();
            true
        }
        '\x1b' | 'q' | 'Q' => {
            crate::drivers::audio::click_feedback();
            drop(state);
            exit_to_desktop();
            true
        }
        _ => false,
    }
}

/// Maneja toques directos en pantalla táctil o clics de ratón
pub fn handle_click(mx: usize, my: usize) -> bool {
    if my >= 655 && my <= 703 {
        if mx >= 42 && mx <= 257 {
            crate::drivers::audio::click_feedback();
            exit_to_desktop();
            return true;
        } else if mx >= 275 && mx <= 490 {
            handle_key('p');
            return true;
        } else if mx >= 508 && mx <= 723 {
            let mut state = SMART_HUB.lock();
            execute_tile_action(&mut state, 1);
            return true;
        } else if mx >= 741 && mx <= 981 {
            handle_key('m');
            return true;
        }
    }

    let start_x = 42;
    let start_y = 85;
    let tile_w = 296;
    let tile_h = 255;
    let gap_x = 22;
    let gap_y = 20;

    for idx in 0..TOTAL_TILES {
        let col = idx % 3;
        let row = idx / 3;
        let tx = start_x + col * (tile_w + gap_x);
        let ty = start_y + row * (tile_h + gap_y);

        if mx >= tx && mx <= tx + tile_w && my >= ty && my <= ty + tile_h {
            crate::drivers::audio::click_feedback();
            let mut state = SMART_HUB.lock();
            state.focused_tile = idx;
            execute_tile_action(&mut state, idx);
            drop(state);
            return true;
        }
    }

    false
}

/// Ejecuta la acción asociada al mosaico seleccionado
fn execute_tile_action(state: &mut SmartHubState, tile_idx: usize) {
    match tile_idx {
        0 => {
            let p = crate::drivers::profile::detect_device_profile();
            state.last_action = format!("[HARDWARE] Perfil: {} (Fuente: {})", p.class.as_str(), p.source);
            render_smart_hub();
        }
        1 => {
            state.last_action = String::from("[MULTIMEDIA] Abriendo BeaverAudio en Beaver Desktop...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Player);
            }
            exit_to_desktop();
        }
        2 => {
            state.last_action = String::from("[BEAVERMESH] Abriendo BeaverSync en Beaver Desktop...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Sync);
            }
            exit_to_desktop();
        }
        3 => {
            state.last_action = String::from("[SISTEMA] Abriendo Monitor de Recursos en Beaver Desktop...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Monitor);
            }
            exit_to_desktop();
        }
        4 => {
            state.last_action = String::from("[ARCHIVOS] Abriendo BeaverNote en Beaver Desktop...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Note);
            }
            exit_to_desktop();
        }
        5 => {
            state.last_action = String::from("[CONSOLA] Abriendo Beaver Terminal...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Terminal);
            }
            exit_to_desktop();
        }
        _ => {}
    }
}

/// Regresa al escritorio tradicional Beaver Desktop
fn exit_to_desktop() {
    {
        let mut term = crate::gui::desktop::GUI_TERMINAL.lock();
        term.mode = crate::gui::desktop::GuiViewMode::Desktop;
    }
    crate::gui::render_desktop();
}

/// Enfoca un mosaico específico desde el control remoto o controlador táctil
pub fn focus_tile(idx: usize) {
    if idx < TOTAL_TILES {
        let mut state = SMART_HUB.lock();
        state.focused_tile = idx;
    }
}

/// Activa la acción del mosaico actualmente enfocado
pub fn activate_focused_tile() {
    let mut state = SMART_HUB.lock();
    let cur = state.focused_tile;
    execute_tile_action(&mut state, cur);
}
