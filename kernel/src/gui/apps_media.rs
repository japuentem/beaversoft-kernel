// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Media & Productivity GUI Applications (BeaverAudio, BeaverPhotos, BeaverCalendar)
// ============================================================================

extern crate alloc;

use alloc::format;
use crate::sync::SpinMutex;
use crate::gui::canvas::{Canvas, Color};
use crate::gui::window::{WINDOW_MGR, WindowId};
use crate::gui::icons::*;

#[derive(Clone, Copy)]
pub struct PlayerState {
    pub is_playing: bool,
    pub track_idx: usize,
}

pub static PLAYER_STATE: SpinMutex<PlayerState> = SpinMutex::new(PlayerState {
    is_playing: false,
    track_idx: 0,
});

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
        ("1", "Beaver Chime (PCM Startup)",  "Beaver Audio Lab",     "0:03"),
        ("2", "Northern Lights (Ring3)",    "The Sample Band",      "3:45"),
        ("3", "Silicon Dawn (Waves DSP)",   "MSR Audio Engine",     "4:12"),
        ("4", "Kernel Harmony (HDA Dual)",  "Beaver Core Driver",   "2:30"),
    ];

    for (i, (num, title, artist, duration)) in tracks.iter().enumerate() {
        let ty = y + 64 + i * 26;
        if i == state.track_idx {
            Canvas::fill_rounded_rect(list_x - 4, ty - 3, list_w, 22, 5, 0xFEF3C7);
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

    // 3. Panel de Control Inferior Estilo Lord OS
    let ctrl_y = y + h.saturating_sub(68);
    Canvas::fill_rect(x + 1, ctrl_y, w - 2, 67, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_line_h(x + 1, ctrl_y, w - 2, Color::BEAVER_BORDER_LIGHT);

    let art_x = x + 12;
    let art_y = ctrl_y + 12;
    Canvas::fill_rounded_rect(art_x, art_y, 42, 42, 8, 0x1E1B4B);
    Canvas::draw_rounded_rect(art_x, art_y, 42, 42, 8, Color::SLATE_600);
    Canvas::draw_string(art_x + 17, art_y + 13, "B", Color::BEAVER_AMBER, None);

    let (cur_title, cur_status) = if state.is_playing {
        ("Beaver Chime (PCM Startup)", "Reproduciendo - HDA Duplex 44.1kHz")
    } else {
        ("Beaver Chime (PCM Startup)", "En pausa - Presione Play")
    };
    Canvas::draw_string_prop(x + 62, ctrl_y + 14, cur_title, Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 62, ctrl_y + 30, cur_status, Color::BEAVER_TEXT_MUTED, None);

    let ctrl_cx = x + w / 2 + 40;
    let btn_cy = ctrl_y + 24;

    draw_media_button(ctrl_cx.saturating_sub(56), btn_cy, 10, MediaBtnKind::Shuffle, false);
    draw_media_button(ctrl_cx.saturating_sub(28), btn_cy, 11, MediaBtnKind::Previous, false);
    draw_media_button(ctrl_cx, btn_cy, 15, if state.is_playing { MediaBtnKind::Pause } else { MediaBtnKind::Play }, true);
    draw_media_button(ctrl_cx + 28, btn_cy, 11, MediaBtnKind::Next, false);
    draw_media_button(ctrl_cx + 56, btn_cy, 10, MediaBtnKind::Repeat, false);

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
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
    }
}

pub fn handle_player_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::Player).cloned();
    if let Some(w) = win {
        let ctrl_y = wy + w.height.saturating_sub(68);
        let ctrl_cx = wx + w.width / 2 + 40;
        if mx >= ctrl_cx.saturating_sub(18) && mx <= ctrl_cx + 18 && my >= ctrl_y + 10 && my <= ctrl_y + 40 {
            let mut state = PLAYER_STATE.lock();
            state.is_playing = !state.is_playing;
            let playing = state.is_playing;
            drop(state);
            if playing {
                crate::drivers::audio::play_chime();
            }
            render_player_window();
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

    let main_x = x + sb_w + 12;
    let main_w = w.saturating_sub(sb_w + 20);

    Canvas::draw_string_prop(main_x, y + 42, "Galeria de ejemplo (miniaturas generadas por codigo)", Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_line_h(main_x, y + 62, main_w, Color::BEAVER_BORDER_LIGHT);

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
            Canvas::fill_rounded_rect(tx, ty, thumb_w, thumb_h, 8, *bg);
            Canvas::draw_rounded_rect(tx, ty, thumb_w, thumb_h, 8, Color::BEAVER_BORDER_LIGHT);

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

            Canvas::draw_string_prop(tx + 4, ty + thumb_h + 4, name, Color::BEAVER_TEXT_DARK, None);
            Canvas::draw_string_prop(tx + 4, ty + thumb_h + 16, "1920x1080 TrueColor", Color::BEAVER_TEXT_MUTED, None);
        }
    }
}

pub fn render_photos_window() {
    draw_photos_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Photos) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
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
                if let Some(mut state) = crate::gui::desktop::GUI_TERMINAL.try_lock() {
                    let photo_names = ["Event Horizon", "Northern Lights", "Silicon Dawn", "Beaver River", "Supernova 2026", "Starfield Cluster"];
                    state.lines.push(format!("[FOTOS][DEMO] '{}' es una miniatura generada por codigo; no existe archivo en disco.", photo_names[i]));
                }
                render_photos_window();
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

    let rtc = crate::drivers::timer::local_datetime();

    let cal_title = format!("{} {}", month_name(rtc.month), rtc.year);
    Canvas::draw_string_prop(x + 18, y + 42, &cal_title, Color::BEAVER_TEXT_DARK, None);

    let pill_x = x + w.saturating_sub(190);
    Canvas::fill_rounded_rect(pill_x, y + 38, 175, 24, 6, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_string_prop(pill_x + 10, y + 43, "Dia", Color::BEAVER_TEXT_MUTED, None);
    Canvas::draw_string_prop(pill_x + 50, y + 43, "Semana", Color::BEAVER_TEXT_MUTED, None);
    Canvas::fill_rounded_rect(pill_x + 115, y + 40, 56, 20, 4, Color::WHITE);
    Canvas::draw_string_prop(pill_x + 125, y + 43, "Mes", Color::BEAVER_TEXT_DARK, None);

    Canvas::draw_line_h(x + 15, y + 68, w.saturating_sub(30), Color::BEAVER_BORDER_LIGHT);

    let day_names = ["Lun", "Mar", "Mie", "Jue", "Vie", "Sab", "Dom"];
    let cell_w = (w.saturating_sub(36)) / 7;
    for (i, dname) in day_names.iter().enumerate() {
        let dx = x + 18 + i * cell_w;
        let col = if i >= 5 { Color::BEAVER_AMBER_DARK } else { Color::BEAVER_TEXT_MUTED };
        Canvas::draw_string_prop(dx + cell_w / 2 - 10, y + 74, dname, col, None);
    }

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

pub fn render_calendar_window() {
    draw_calendar_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Calendar) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
    }
}

pub fn handle_calendar_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::Calendar).cloned();
    if let Some(w) = win {
        let pill_x = wx + w.width.saturating_sub(190);
        if mx >= pill_x && mx <= pill_x + 175 && my >= wy + 38 && my <= wy + 62 {
            crate::drivers::audio::click_feedback();
            render_calendar_window();
        }
    }
}

