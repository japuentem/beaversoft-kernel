// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver Smart Hub - 10-Foot UI for Smart TVs, Touch Displays & Kiosks
//              Phase 1 Implementation: Ubiquitous Touch & D-Pad Remote Navigation
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::string::String;
use crate::sync::SpinMutex;
use crate::gui::canvas::{Canvas, Color};
use crate::drivers::timer::get_uptime_seconds;
use crate::memory::heap::HEAP_ALLOCATOR;
use crate::drivers::keyboard::{KEY_UP_ARROW, KEY_DOWN_ARROW, KEY_LEFT_ARROW, KEY_RIGHT_ARROW};

pub const TOTAL_TILES: usize = 6;

pub struct SmartHubState {
    pub focused_tile: usize,       // 0..5 (3 columnas x 2 filas)
    pub fridge_temp_tenths: i32,   // Ej: 38 = +3.8 °C
    pub fridge_door_open: bool,
    pub ambient_light_on: bool,
    pub last_action: String,
}

pub static SMART_HUB: SpinMutex<SmartHubState> = SpinMutex::new(SmartHubState {
    focused_tile: 0,
    fridge_temp_tenths: 38,
    fridge_door_open: false,
    ambient_light_on: true,
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

/// Dibuja la interfaz completa de Smart Hub con doble búfer TrueColor
pub fn render_smart_hub() {
    let (width, height) = Canvas::screen_dimensions();

    // 1. Fondo Oscuro Obsidian Keynote (Atmósfera Cinematográfica para TV)
    Canvas::fill_rect(0, 0, width, height, 0x07090E);

    // Rejilla sutil nanométrica para textura visual de calidad
    for x in (30..width).step_by(40) {
        for y in (70..height).step_by(40) {
            Canvas::put_pixel(x, y, 0x172033);
        }
    }

    // 2. Barra Superior de Estado y Cabecera Institucional
    Canvas::fill_rect(0, 0, width, 65, 0x0B132B);
    Canvas::draw_line_h(0, 64, width, Color::SLATE_700);

    // Emblema Beaver
    Canvas::fill_rounded_rect(30, 14, 36, 36, 8, Color::BEAVER_AMBER);
    Canvas::draw_string(42, 22, "B", Color::WHITE, None);

    // Título Principal y Pastilla de Modo
    Canvas::draw_string_prop(76, 16, "BEAVER SMART HUB", Color::WHITE, None);
    Canvas::fill_rounded_rect(240, 18, 230, 24, 12, 0x111E38);
    Canvas::draw_rounded_rect(240, 18, 230, 24, 12, Color::BEAVER_ACCENT_CYAN);
    Canvas::draw_string_prop(252, 21, "[MODO TV & KIOSCO DOMÓTICO]", Color::BEAVER_ACCENT_CYAN, None);

    // Información en Vivo (Uptime, Red Mesh, Reloj)
    let uptime = get_uptime_seconds();
    let hours = uptime / 3600;
    let mins = (uptime % 3600) / 60;
    let secs = uptime % 60;
    let clock_str = format!("{:02}:{:02}:{:02}", hours, mins, secs);

    Canvas::draw_string_prop(width - 320, 16, "Malla P2P: :4242 [3 Nodos]", Color::BEAVER_TEAL_BRIGHT, None);
    Canvas::draw_string_prop(width - 130, 16, &clock_str, Color::BEAVER_AMBER_LIGHT, None);
    Canvas::draw_string_prop(width - 320, 38, "IP Local: 192.168.1.50 • Cero Nube", Color::SLATE_400, None);

    // 3. Renderizado de la Cuadrícula de 6 Mosaicos Inteligentes (3 Cols x 2 Rows)
    let state = SMART_HUB.lock();
    let focused = state.focused_tile;
    let fridge_temp_str = format!("{}.{} °C", state.fridge_temp_tenths / 10, (state.fridge_temp_tenths % 10).abs());
    let fridge_door_str = if state.fridge_door_open { "ABIERTA (Alerta)" } else { "CERRADA (Segura)" };
    let light_str = if state.ambient_light_on { "ENCENDIDA (Cálida)" } else { "APAGADA (Ahorro)" };
    let last_act = state.last_action.clone();
    drop(state);

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

        draw_smart_tile(idx, tx, ty, tile_w, tile_h, is_focused, &fridge_temp_str, fridge_door_str, light_str);
    }

    // 4. Barra Inferior de Acción Táctil y Teclas de Control Remoto
    Canvas::fill_rect(0, 620, width, 148, 0x090D1A);
    Canvas::draw_line_h(0, 620, width, 0x1E293B);

    // Línea de Estado / Feedback de Última Acción
    let status_text = if last_act.is_empty() {
        String::from("Listo. Usa las flechas del control remoto o toca directamente cualquier mosaico.")
    } else {
        last_act
    };
    Canvas::fill_circle(45, 638, 5, Color::BEAVER_GREEN);
    Canvas::draw_string_prop(60, 630, &status_text, Color::BEAVER_AMBER_LIGHT, None);

    // Botones Táctiles de Gran Escala en la Parte Inferior
    draw_touch_button(42, 655, 215, 48, "[ ESC ] MODO ESCRITORIO", Color::BEAVER_ACCENT_CYAN, 0x131E33);
    draw_touch_button(275, 655, 215, 48, "[ T ] AJUSTAR FRÍO (-1°C)", Color::BEAVER_GREEN, 0x10261E);
    draw_touch_button(508, 655, 215, 48, "[ P ] PUERTA REFRIGERADOR", Color::BEAVER_AMBER, 0x2E2010);
    draw_touch_button(741, 655, 240, 48, "[ M ] SINCRONIZAR MESH :4242", Color::BEAVER_PURPLE, 0x221538);

    // Indicaciones de Navegación D-Pad
    Canvas::draw_string_prop(42, 715, "Controles de Sala:  [FLECHAS / D-PAD / WASD] Mover Foco   •   [ENTER / ESPACIO] Abrir   •   [ESC] Escritorio", Color::SLATE_400, None);
    Canvas::draw_string_prop(42, 735, "Beaver OS v0.9.0 • Arquitectura Determinista Anti-Bloqueos • Creado por José Alberto Puente Maldonado", 0x475569, None);

    // 5. Dibujar puntero de ratón o toque
    let (mx, my) = {
        let m = crate::drivers::mouse::MOUSE.lock();
        (m.state.x as usize, m.state.y as usize)
    };
    Canvas::draw_cursor_at(mx, my);

    // 6. Presentación atómica al framebuffer físico
    Canvas::present();
}

/// Dibuja un mosaico inteligente individual con formato optimizado para visualización a distancia (10-Foot UI)
fn draw_smart_tile(
    idx: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    is_focused: bool,
    fridge_temp: &str,
    fridge_door: &str,
    light_status: &str,
) {
    let (bg_color, border_color, border_thickness) = if is_focused {
        (0x152238, Color::BEAVER_ACCENT_CYAN, 3)
    } else {
        (0x0E1626, Color::SLATE_700, 1)
    };

    // Fondo del Mosaico con esquinas redondeadas
    Canvas::fill_rounded_rect(x, y, w, h, 14, bg_color);

    // Borde (Simple o resaltado en caso de foco D-Pad)
    if border_thickness == 3 {
        Canvas::draw_rounded_rect(x, y, w, h, 14, border_color);
        Canvas::draw_rounded_rect(x + 1, y + 1, w - 2, h - 2, 13, border_color);
        Canvas::draw_rounded_rect(x + 2, y + 2, w - 4, h - 4, 12, border_color);
        // Indicador de Foco Brillante en esquina superior derecha
        Canvas::fill_rounded_rect(x + w - 105, y + 12, 92, 20, 10, Color::BEAVER_ACCENT_CYAN);
        Canvas::draw_string_prop(x + w - 98, y + 14, "ENFOCADO", Color::BEAVER_NAVY, None);
    } else {
        Canvas::draw_rounded_rect(x, y, w, h, 14, border_color);
    }

    match idx {
        0 => {
            // [REFRIGERADOR Y DOMÓTICA]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, Color::BEAVER_GREEN);
            Canvas::draw_string(x + 23, y + 20, "F", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "REFRIGERADOR SMART", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "Control de Electrodoméstico", Color::BEAVER_GREEN, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Temperatura Interior:", Color::SLATE_200, None);
            Canvas::fill_rounded_rect(x + 18, y + 90, 150, 42, 8, 0x0A2E1F);
            Canvas::draw_string_prop(x + 30, y + 98, fridge_temp, Color::BEAVER_GREEN, None);

            Canvas::draw_string_prop(x + 18, y + 145, "Estado Puerta:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 165, fridge_door, if fridge_door.starts_with("A") { Color::BEAVER_RED } else { Color::BEAVER_GREEN }, None);

            Canvas::draw_string_prop(x + 18, y + 190, "Iluminación:", Color::SLATE_400, None);
            Canvas::draw_string_prop(x + 18, y + 208, light_status, Color::BEAVER_AMBER_LIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> Presiona ENTER o Toca para ajustar", Color::SLATE_400, None);
        }
        1 => {
            // [CENTRO MULTIMEDIA & STREAMING TV]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, Color::BEAVER_PURPLE);
            Canvas::draw_string(x + 23, y + 20, "M", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "CENTRO MULTIMEDIA", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "Audio & TV TrueColor", Color::BEAVER_PURPLE, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Salida Gráfica:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 88, "VESA / GOP TrueColor 60 Hz", Color::WHITE, None);

            Canvas::draw_string_prop(x + 18, y + 118, "Subsistema de Audio:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 138, "AC97 / Intel HDA Nativo", Color::BEAVER_TEAL_BRIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 168, "Fotos Familiares:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 188, "Galería Local BeaverPhotos", Color::BEAVER_AMBER_LIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> Presiona ENTER o Toca para reproducir", Color::SLATE_400, None);
        }
        2 => {
            // [RED DE MALLA BEAVERMESH P2P]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, Color::BEAVER_ACCENT_CYAN);
            Canvas::draw_string(x + 23, y + 20, "N", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "MALLA BEAVERMESH", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "Interconexión P2P :4242", Color::BEAVER_ACCENT_CYAN, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Nodos Activos en Casa:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 90, "• Nodo 1: Smart TV (Sala)", Color::WHITE, None);
            Canvas::draw_string_prop(x + 18, y + 112, "• Nodo 2: Smart Fridge (Cocina)", Color::WHITE, None);
            Canvas::draw_string_prop(x + 18, y + 134, "• Nodo 3: Laptop (Despacho)", Color::WHITE, None);

            Canvas::draw_string_prop(x + 18, y + 165, "Privacidad del Hogar:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 185, "Cero datos enviados a internet", Color::BEAVER_GREEN, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> Presiona ENTER o Toca para sincronizar", Color::SLATE_400, None);
        }
        3 => {
            // [MONITOR DE SALUD & RENDIMIENTO]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, Color::BEAVER_AMBER);
            Canvas::draw_string(x + 23, y + 20, "S", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "SISTEMA & HARDWARE", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "Supervisor Anti-Crash", Color::BEAVER_AMBER, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            let (_total, used, free, _, _) = HEAP_ALLOCATOR.stats();
            let ram_str = format!("RAM Usada: {} KiB (~14 MB SO)", used / 1024);
            let free_str = format!("RAM Libre: {} KiB (99% Libre)", free / 1024);

            Canvas::draw_string_prop(x + 18, y + 68, "Uso de Memoria:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 90, &ram_str, Color::BEAVER_AMBER_LIGHT, None);
            Canvas::draw_string_prop(x + 18, y + 112, &free_str, Color::BEAVER_GREEN, None);

            Canvas::draw_string_prop(x + 18, y + 145, "Resiliencia de Disco:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 165, "BeaverFS Doble Slot A/B Anti-Corte", Color::BEAVER_TEAL_BRIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 195, "CPU / Estado Térmico: Fresco (HLT)", Color::SLATE_400, None);
            Canvas::draw_string_prop(x + 18, y + 232, ">> Presiona ENTER o Toca para abrir Monitor", Color::SLATE_400, None);
        }
        4 => {
            // [NOTAS & RECETAS DE COCINA]
            Canvas::fill_rounded_rect(x + 14, y + 14, 28, 28, 6, 0xF43F5E);
            Canvas::draw_string(x + 23, y + 20, "R", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 16, "RECETAS & NOTAS", Color::WHITE, None);
            Canvas::draw_string_prop(x + 50, y + 34, "BeaverNote Domótico", 0xF43F5E, None);
            Canvas::draw_line_h(x + 14, y + 52, w - 28, Color::SLATE_700);

            Canvas::draw_string_prop(x + 18, y + 68, "Notas Familiares en Disco:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 90, "1. Lista de supermercado (6 items)", Color::WHITE, None);
            Canvas::draw_string_prop(x + 18, y + 112, "2. Receta: Crema de verduras", Color::WHITE, None);
            Canvas::draw_string_prop(x + 18, y + 134, "3. Horario de medicamentos", Color::WHITE, None);

            Canvas::draw_string_prop(x + 18, y + 165, "Almacenamiento Local:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 185, "Guardado transaccional en /disk", Color::BEAVER_AMBER_LIGHT, None);

            Canvas::draw_string_prop(x + 18, y + 232, ">> Presiona ENTER o Toca para escribir", Color::SLATE_400, None);
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

            Canvas::draw_string_prop(x + 18, y + 120, "Pasarela Rápida:", Color::SLATE_200, None);
            Canvas::draw_string_prop(x + 18, y + 140, "IA32_LSTAR (0xC0000082) ~42 ciclos", Color::BEAVER_ACCENT_CYAN, None);

            Canvas::draw_string_prop(x + 18, y + 175, "Prompt Interactivo: root@beaver:/#", Color::WHITE, None);
            Canvas::draw_string_prop(x + 18, y + 232, ">> Presiona ENTER o Toca para abrir Shell", Color::SLATE_400, None);
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
        // Navegación Izquierda
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
        // Navegación Derecha
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
        // Navegación Arriba
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
        // Navegación Abajo
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
        // Siguiente mosaico con TAB
        '\t' => {
            state.focused_tile = (state.focused_tile + 1) % TOTAL_TILES;
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        // Activación de la tarjeta seleccionada con ENTER o ESPACIO
        '\n' | ' ' => {
            crate::drivers::audio::click_feedback();
            let cur = state.focused_tile;
            execute_tile_action(&mut state, cur);
            drop(state);
            true
        }
        // Tecla de acceso rápido 't' para ajustar temperatura del refrigerador
        't' | 'T' => {
            state.fridge_temp_tenths -= 5;
            if state.fridge_temp_tenths < 10 {
                state.fridge_temp_tenths = 50; // Cliclo de +1.0 a +5.0 °C
            }
            let temp_str = format!("{}.{} °C", state.fridge_temp_tenths / 10, state.fridge_temp_tenths % 10);
            state.last_action = format!("[REFRIGERADOR] Temperatura fijada a {}", temp_str);
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        // Tecla de acceso rápido 'p' para puerta
        'p' | 'P' => {
            state.fridge_door_open = !state.fridge_door_open;
            let status = if state.fridge_door_open { "ABIERTA" } else { "CERRADA" };
            state.last_action = format!("[REFRIGERADOR] Sensor de puerta: {}", status);
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        // Tecla de acceso rápido 'm' para malla BeaverMesh
        'm' | 'M' => {
            state.last_action = String::from("[BEAVERMESH] Sincronizando 3 nodos en red local (:4242) sin nube...");
            crate::drivers::audio::click_feedback();
            drop(state);
            render_smart_hub();
            true
        }
        // ESC o 'q': Salir y retornar a Beaver Desktop
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
    // 1. Comprobar botones táctiles inferiores
    if my >= 655 && my <= 703 {
        if mx >= 42 && mx <= 257 {
            // [ ESC ] MODO ESCRITORIO
            crate::drivers::audio::click_feedback();
            exit_to_desktop();
            return true;
        } else if mx >= 275 && mx <= 490 {
            // [ T ] AJUSTAR FRÍO
            handle_key('t');
            return true;
        } else if mx >= 508 && mx <= 723 {
            // [ P ] PUERTA
            handle_key('p');
            return true;
        } else if mx >= 741 && mx <= 981 {
            // [ M ] MESH
            handle_key('m');
            return true;
        }
    }

    // 2. Comprobar clic/toque en los 6 mosaicos
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
            // Refrigerador: cicla temperatura y modo
            state.fridge_temp_tenths -= 2;
            if state.fridge_temp_tenths < 15 {
                state.fridge_temp_tenths = 45;
            }
            let temp_str = format!("{}.{} °C", state.fridge_temp_tenths / 10, state.fridge_temp_tenths % 10);
            state.last_action = format!("[REFRIGERADOR] Frío ajustado a {}. Compresor operando normalmente en Ring 3.", temp_str);
            render_smart_hub();
        }
        1 => {
            // Multimedia: cambia a Beaver Desktop y abre BeaverAudio
            state.last_action = String::from("[MULTIMEDIA] Abriendo BeaverAudio en Beaver Desktop...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Player);
            }
            exit_to_desktop();
        }
        2 => {
            // Mesh: cambia a Beaver Desktop y abre BeaverSync
            state.last_action = String::from("[BEAVERMESH] Abriendo BeaverSync en Beaver Desktop...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Sync);
            }
            exit_to_desktop();
        }
        3 => {
            // Monitor: cambia a Beaver Desktop y abre Monitor
            state.last_action = String::from("[SISTEMA] Abriendo Monitor de Recursos en Beaver Desktop...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Monitor);
            }
            exit_to_desktop();
        }
        4 => {
            // Notas / Recetas: abre BeaverNote
            state.last_action = String::from("[RECETAS] Abriendo BeaverNote en Beaver Desktop...");
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Note);
            }
            exit_to_desktop();
        }
        5 => {
            // Terminal: abre consola interactiva en Beaver Desktop
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
