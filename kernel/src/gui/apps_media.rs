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
    pub category: usize, // 0 = Canciones, 1 = Efectos, 2 = Sintesis
}

pub static PLAYER_STATE: SpinMutex<PlayerState> = SpinMutex::new(PlayerState {
    is_playing: false,
    track_idx: 0,
    category: 0,
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

    let state = *PLAYER_STATE.lock();

    let (tracks, window_title) = match state.category {
        1 => (
            [
                ("1", "Campana Notificacion",   "Beaver FX Studio",     "0:01"),
                ("2", "Alerta Anti-Crash OK",   "Resilience Lab",       "0:02"),
                ("3", "Handshake BeaverMesh",   "Net Engine :4242",     "0:01"),
                ("4", "Pulsacion Teclado",      "Hardware Haptic",      "0:01"),
            ],
            "Efectos de Sonido - BeaverAudio",
        ),
        2 => (
            [
                ("1", "Tono Senoidal 440 Hz",   "DSP Synthesizer",      "0:05"),
                ("2", "Barrido Audio 1k-4kHz",  "Frequency Generator",  "0:04"),
                ("3", "Ruido Blanco Audio DAC", "Noise Generator",      "0:03"),
                ("4", "Onda Cuadrada PIT 100Hz","Timer Synthesizer",    "0:05"),
            ],
            "Sintesis Acustica DSP - BeaverAudio",
        ),
        _ => (
            [
                ("1", "Beaver Chime (PCM Startup)", "Beaver Audio Lab",  "0:03"),
                ("2", "Northern Lights (Ring3)",   "The Sample Band",   "3:45"),
                ("3", "Silicon Dawn (Waves DSP)",  "MSR Audio Engine",  "4:12"),
                ("4", "Kernel Harmony (HDA Dual)", "Beaver Core Driver","2:30"),
            ],
            "01 Beaver Chime.wav - BeaverAudio",
        ),
    };

    Canvas::draw_window(x, y, w, h, window_title, is_active);

    // 1. Barra Lateral Izquierda (Biblioteca estilo Lord OS)
    let sb_w = 110;
    Canvas::fill_rect(x + 1, y + 32, sb_w, h.saturating_sub(33), Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_line_v(x + 1 + sb_w, y + 32, h.saturating_sub(33), Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 10, y + 42, "BIBLIOTECA", Color::BEAVER_TEXT_MUTED, None);

    let cat_y_offsets = [58, 84, 108];
    let cat_items = [
        ("Canciones", SidebarIconKind::Music),
        ("Efectos", SidebarIconKind::Settings),
        ("Sintesis", SidebarIconKind::Terminal),
    ];
    for (i, (lbl, icon)) in cat_items.iter().enumerate() {
        let is_sel = state.category == i;
        let bg = if is_sel { 0xDBEAFE } else { Color::BEAVER_TITANIUM_BAR };
        let ico_col = if is_sel { Color::BEAVER_AMBER } else { Color::BEAVER_TEXT_MUTED };
        Canvas::fill_rounded_rect(x + 5, y + cat_y_offsets[i], sb_w - 10, 22, 6, bg);
        draw_sidebar_icon(x + 10, y + cat_y_offsets[i] + 2, *icon, ico_col);
        Canvas::draw_string_prop(x + 30, y + cat_y_offsets[i] + 4, lbl, Color::BEAVER_TEXT_DARK, None);
    }

    // 2. Lista de Pistas (Estilo Lord OS)
    let list_x = x + sb_w + 12;
    let list_w = w.saturating_sub(sb_w + 22);

    Canvas::draw_string_prop(list_x, y + 40, "#   Titulo                      Artista               Tiempo", Color::BEAVER_TEXT_MUTED, None);
    Canvas::draw_line_h(list_x, y + 54, list_w, Color::BEAVER_BORDER_LIGHT);

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

    let cur_track_name = tracks[state.track_idx.min(3)].1;
    let (cur_title, cur_status) = if state.is_playing {
        (cur_track_name, "Reproduciendo - HDA Duplex 44.1kHz")
    } else {
        (cur_track_name, "En pausa - Presione Play")
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
        let sb_w = 110;
        // 1. Clic en la barra lateral de categorías
        if mx >= wx && mx <= wx + sb_w {
            let mut state = PLAYER_STATE.lock();
            let mut changed = false;
            if my >= wy + 56 && my <= wy + 80 {
                state.category = 0;
                state.track_idx = 0;
                changed = true;
            } else if my >= wy + 82 && my <= wy + 104 {
                state.category = 1;
                state.track_idx = 0;
                changed = true;
            } else if my >= wy + 106 && my <= wy + 130 {
                state.category = 2;
                state.track_idx = 0;
                changed = true;
            }
            if changed {
                state.is_playing = false;
                drop(state);
                crate::drivers::audio::click_feedback();
                render_player_window();
                return;
            }
        }

        // 2. Clic en las pistas individuales
        let list_x = wx + sb_w + 12;
        let list_w = w.width.saturating_sub(sb_w + 22);
        if mx >= list_x && mx <= list_x + list_w {
            for i in 0..4 {
                let ty = wy + 64 + i * 26;
                if my >= ty - 3 && my <= ty + 22 {
                    let mut state = PLAYER_STATE.lock();
                    state.track_idx = i;
                    state.is_playing = true;
                    let cat = state.category;
                    drop(state);
                    play_track_audio(cat, i);
                    render_player_window();
                    return;
                }
            }
        }

        // 3. Controles inferiores
        let ctrl_y = wy + w.height.saturating_sub(68);
        let ctrl_cx = wx + w.width / 2 + 40;

        // Previous
        if mx >= ctrl_cx.saturating_sub(38) && mx <= ctrl_cx.saturating_sub(18) && my >= ctrl_y + 12 && my <= ctrl_y + 36 {
            let mut state = PLAYER_STATE.lock();
            state.track_idx = (state.track_idx + 3) % 4;
            state.is_playing = true;
            let (cat, idx) = (state.category, state.track_idx);
            drop(state);
            play_track_audio(cat, idx);
            render_player_window();
            return;
        }

        // Play / Pause
        if mx >= ctrl_cx.saturating_sub(16) && mx <= ctrl_cx + 16 && my >= ctrl_y + 10 && my <= ctrl_y + 40 {
            let mut state = PLAYER_STATE.lock();
            state.is_playing = !state.is_playing;
            let (playing, cat, idx) = (state.is_playing, state.category, state.track_idx);
            drop(state);
            if playing {
                play_track_audio(cat, idx);
            }
            render_player_window();
            return;
        }

        // Next
        if mx >= ctrl_cx + 18 && mx <= ctrl_cx + 38 && my >= ctrl_y + 12 && my <= ctrl_y + 36 {
            let mut state = PLAYER_STATE.lock();
            state.track_idx = (state.track_idx + 1) % 4;
            state.is_playing = true;
            let (cat, idx) = (state.category, state.track_idx);
            drop(state);
            play_track_audio(cat, idx);
            render_player_window();
            return;
        }
    }
}

fn play_track_audio(cat: usize, track: usize) {
    match (cat, track) {
        (0, 0) => crate::drivers::audio::play_chime(),
        (0, 1) => crate::drivers::audio::play_tone(523, 150),
        (0, 2) => crate::drivers::audio::play_tone(659, 150),
        (0, 3) => crate::drivers::audio::play_tone(784, 180),
        (1, 0) => crate::drivers::audio::play_tone(880, 90),
        (1, 1) => crate::drivers::audio::play_tone(440, 120),
        (1, 2) => crate::drivers::audio::play_tone(1046, 90),
        (1, 3) => crate::drivers::audio::play_tone(220, 50),
        (2, 0) => crate::drivers::audio::play_tone(440, 200),
        (2, 1) => crate::drivers::audio::play_tone(1200, 150),
        (2, 2) => crate::drivers::audio::play_tone(350, 150),
        (2, 3) => crate::drivers::audio::play_tone(100, 200),
        _ => crate::drivers::audio::click_feedback(),
    }
}

// ============================================================================
// BeaverPhotos - Galería y Visor Gráfico de Imágenes (Lord OS Style)
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PhotosTab {
    Todas = 0,
    Paisajes = 1,
    Malla = 2,
}

pub static PHOTOS_TAB: SpinMutex<PhotosTab> = SpinMutex::new(PhotosTab::Todas);

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

    let tab = *PHOTOS_TAB.lock();
    let is_todas = tab == PhotosTab::Todas;
    let is_paisajes = tab == PhotosTab::Paisajes;
    let is_malla = tab == PhotosTab::Malla;

    let sb_w = 125;
    Canvas::fill_rect(x + 1, y + 32, sb_w, h.saturating_sub(33), Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_line_v(x + 1 + sb_w, y + 32, h.saturating_sub(33), Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 10, y + 42, "BIBLIOTECA", Color::BEAVER_TEXT_MUTED, None);

    Canvas::fill_rounded_rect(x + 6, y + 58, sb_w - 12, 22, 6, if is_todas { 0xDBEAFE } else { Color::BEAVER_TITANIUM_BAR });
    draw_sidebar_icon(x + 10, y + 60, SidebarIconKind::Photos, if is_todas { Color::BEAVER_AMBER } else { Color::BEAVER_TEXT_MUTED });
    Canvas::draw_string_prop(x + 32, y + 62, "Todas (6)", Color::BEAVER_TEXT_DARK, None);

    Canvas::fill_rounded_rect(x + 6, y + 84, sb_w - 12, 22, 6, if is_paisajes { 0xDBEAFE } else { Color::BEAVER_TITANIUM_BAR });
    draw_sidebar_icon(x + 10, y + 86, SidebarIconKind::Home, if is_paisajes { Color::BEAVER_AMBER } else { Color::BEAVER_TEXT_MUTED });
    Canvas::draw_string_prop(x + 32, y + 88, "Paisajes", Color::BEAVER_TEXT_DARK, None);

    Canvas::fill_rounded_rect(x + 6, y + 110, sb_w - 12, 22, 6, if is_malla { 0xDBEAFE } else { Color::BEAVER_TITANIUM_BAR });
    draw_sidebar_icon(x + 10, y + 112, SidebarIconKind::Mesh, if is_malla { Color::BEAVER_AMBER } else { Color::BEAVER_TEXT_MUTED });
    Canvas::draw_string_prop(x + 32, y + 114, "Malla P2P", Color::BEAVER_TEXT_DARK, None);

    let main_x = x + sb_w + 12;
    let main_w = w.saturating_sub(sb_w + 20);

    let header_desc = match tab {
        PhotosTab::Todas => "Galeria completa (6 miniaturas generadas por codigo)",
        PhotosTab::Paisajes => "Directorio: Paisajes Naturales y Horizontes (3 fotos)",
        PhotosTab::Malla => "Directorio: Malla P2P y Cosmos Interconectado (3 fotos)",
    };
    Canvas::draw_string_prop(main_x, y + 42, header_desc, Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_line_h(main_x, y + 62, main_w, Color::BEAVER_BORDER_LIGHT);

    let thumb_w = 135;
    let thumb_h = 115;
    let all_thumbs = [
        ("Event Horizon", 0x1E1B4B, 0x7C3AED, 0xF59E0B, 2),
        ("Northern Lights", 0x064E3B, 0x0D9488, 0x6EE7B7, 1),
        ("Silicon Dawn", 0x1E293B, 0xB45309, 0xFCD34D, 1),
        ("Beaver River", 0x0C4A6E, 0x0284C7, 0xBAE6FD, 1),
        ("Supernova 2026", 0x450A0A, 0xDC2626, 0xFDE047, 2),
        ("Starfield Cluster", 0x020617, 0x38BDF8, 0xFFFFFF, 2),
    ];

    let mut filtered_idx = 0;
    for (i, (name, bg, mid, glow, category)) in all_thumbs.iter().enumerate() {
        let should_show = match tab {
            PhotosTab::Todas => true,
            PhotosTab::Paisajes => *category == 1,
            PhotosTab::Malla => *category == 2,
        };

        if !should_show {
            continue;
        }

        let col = filtered_idx % 3;
        let row = filtered_idx / 3;
        let tx = main_x + 6 + col * (thumb_w + 14);
        let ty = y + 72 + row * (thumb_h + 30);
        filtered_idx += 1;

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
        // Clic en la barra lateral de álbumes
        if mx >= wx && mx <= wx + sb_w {
            let mut changed = false;
            if my >= wy + 56 && my <= wy + 80 {
                *PHOTOS_TAB.lock() = PhotosTab::Todas;
                changed = true;
            } else if my >= wy + 82 && my <= wy + 104 {
                *PHOTOS_TAB.lock() = PhotosTab::Paisajes;
                changed = true;
            } else if my >= wy + 106 && my <= wy + 130 {
                *PHOTOS_TAB.lock() = PhotosTab::Malla;
                changed = true;
            }
            if changed {
                crate::drivers::audio::click_feedback();
                render_photos_window();
                return;
            }
        }

        let main_x = wx + sb_w + 12;
        let thumb_w = 135;
        let thumb_h = 115;
        let tab = *PHOTOS_TAB.lock();

        let all_thumbs = [
            ("Event Horizon", 2),
            ("Northern Lights", 1),
            ("Silicon Dawn", 1),
            ("Beaver River", 1),
            ("Supernova 2026", 2),
            ("Starfield Cluster", 2),
        ];

        let mut filtered_idx = 0;
        for (_name, category) in all_thumbs.iter() {
            let should_show = match tab {
                PhotosTab::Todas => true,
                PhotosTab::Paisajes => *category == 1,
                PhotosTab::Malla => *category == 2,
            };
            if !should_show {
                continue;
            }

            let col = filtered_idx % 3;
            let row = filtered_idx / 3;
            let tx = main_x + 6 + col * (thumb_w + 14);
            let ty = wy + 72 + row * (thumb_h + 30);
            filtered_idx += 1;

            if mx >= tx && mx <= tx + thumb_w && my >= ty && my <= ty + thumb_h {
                crate::drivers::audio::click_feedback();
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

