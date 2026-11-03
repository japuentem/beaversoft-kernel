// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: 2D Graphics Compositor with RAM Backbuffer & Double Buffering
//              Zero-flicker 60 Hz presentation, alpha blending & window primitives
// ============================================================================

use crate::drivers::vbe::VBE;
use crate::gui::font::get_glyph;
use crate::memory::BACKBUFFER_PHYS_ADDR;

pub const SCREEN_WIDTH: usize = 1024;
pub const SCREEN_HEIGHT: usize = 768;
pub const TOTAL_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

pub struct Color;

impl Color {
    pub const BLACK: u32              = 0x000000;
    pub const WHITE: u32              = 0xFFFFFF;
    pub const SLATE_900: u32          = 0x0B0F19;
    pub const SLATE_800: u32          = 0x151E2E;
    pub const SLATE_700: u32          = 0x1F2937;
    pub const SLATE_600: u32          = 0x374151;
    pub const SLATE_500: u32          = 0x6B7280;
    pub const SLATE_400: u32          = 0x9CA3AF;
    pub const SLATE_200: u32          = 0xE5E7EB;

    pub const BEAVER_NAVY: u32        = 0x0A192F;
    pub const BEAVER_BLUE: u32        = 0x1E3A8A;
    pub const BEAVER_ACCENT_CYAN: u32 = 0x06B6D4;
    pub const BEAVER_ACCENT_TEAL: u32 = 0x14B8A6;
    pub const BEAVER_GREEN: u32       = 0x10B981;
    pub const BEAVER_YELLOW: u32      = 0xF59E0B;
    pub const BEAVER_RED: u32         = 0xEF4444;
    pub const BEAVER_PURPLE: u32      = 0x8B5CF6;

    // --- Identidad Visual Exclusiva Beaver OS (Ámbar Dorado, Glacial Teal y Titanio Nórdico) ---
    pub const BEAVER_AMBER: u32       = 0xF59E0B; // Ámbar Noble (Madera de Castor)
    pub const BEAVER_AMBER_DARK: u32  = 0xB45309; // Ámbar Sombra
    pub const BEAVER_AMBER_LIGHT: u32 = 0xFDE68A; // Ámbar Resplandor
    pub const BEAVER_AMBER_TABS: u32  = 0xD97706; // Solapa de Carpetas
    pub const BEAVER_TEAL: u32        = 0x0D9488; // Glacial Teal (Agua de río)
    pub const BEAVER_TEAL_BRIGHT: u32 = 0x14B8A6; // Glacial Teal Acento
    pub const BEAVER_OBSIDIAN: u32    = 0x0B132B; // Obsidiana Profunda (Barra y Menús)
    pub const BEAVER_TITANIUM: u32    = 0xF8FAFC; // Titanio Cálido (Fondo de Ventana Lord OS)
    pub const BEAVER_TITANIUM_BAR: u32= 0xEEF2F6; // Titanio Barra Lateral
    pub const BEAVER_TEXT_DARK: u32   = 0x0F172A; // Texto Carbón Suave (Alta Legibilidad)
    pub const BEAVER_TEXT_MUTED: u32  = 0x64748B; // Texto Secundario Elegante
    pub const BEAVER_BORDER_LIGHT: u32= 0xCBD5E1; // Borde Suave de Ventana
    pub const BEAVER_RUBY: u32        = 0xEF4444; // Semáforo Cerrar
    pub const BEAVER_EMERALD: u32     = 0x10B981; // Semáforo Maximizar

    pub const WINDOW_BG: u32          = 0xF8FAFC;
    pub const WINDOW_TITLEBAR: u32    = 0xEEF2F6;
    pub const WINDOW_TITLEBAR_ACT: u32= 0xE2E8F0;
    pub const WINDOW_BORDER: u32      = 0xCBD5E1;

    #[inline(always)]
    pub fn rgb(r: u8, g: u8, b: u8) -> u32 {
        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    pub fn blend(c1: u32, c2: u32, factor: u8) -> u32 {
        let f = factor as u32;
        let inv = 255 - f;

        let r1 = (c1 >> 16) & 0xFF;
        let g1 = (c1 >> 8) & 0xFF;
        let b1 = c1 & 0xFF;

        let r2 = (c2 >> 16) & 0xFF;
        let g2 = (c2 >> 8) & 0xFF;
        let b2 = c2 & 0xFF;

        let r = ((r1 * inv) + (r2 * f)) / 255;
        let g = ((g1 * inv) + (g2 * f)) / 255;
        let b = ((b1 * inv) + (b2 * f)) / 255;

        (r << 16) | (g << 8) | b
    }
}

pub struct Canvas;

impl Canvas {
    /// Puntero directo al búfer secundario en RAM física (Double Buffer)
    #[inline(always)]
    pub fn backbuffer_ptr() -> *mut u32 {
        BACKBUFFER_PHYS_ADDR as *mut u32
    }

    /// Dibuja un píxel individual en el búfer secundario de RAM
    #[inline(always)]
    pub fn put_pixel(x: usize, y: usize, color: u32) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            unsafe {
                *Self::backbuffer_ptr().add(y * SCREEN_WIDTH + x) = color;
            }
        }
    }

    /// Rellena un rectángulo directamente en el búfer secundario (sin tocar video bus)
    pub fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
        let max_x = (x + w).min(SCREEN_WIDTH);
        let max_y = (y + h).min(SCREEN_HEIGHT);
        if max_x <= x || max_y <= y {
            return;
        }

        let ptr = Self::backbuffer_ptr();
        let row_w = max_x - x;
        for py in y..max_y {
            let row_offset = py * SCREEN_WIDTH + x;
            unsafe {
                let slice = core::slice::from_raw_parts_mut(ptr.add(row_offset), row_w);
                slice.fill(color);
            }
        }
    }

    /// Dibuja una línea horizontal en el búfer secundario
    pub fn draw_line_h(x: usize, y: usize, w: usize, color: u32) {
        Self::fill_rect(x, y, w, 1, color);
    }

    /// Dibuja una línea vertical en el búfer secundario
    pub fn draw_line_v(x: usize, y: usize, h: usize, color: u32) {
        Self::fill_rect(x, y, 1, h, color);
    }

    /// Dibuja los bordes de un rectángulo hueco
    pub fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
        if w == 0 || h == 0 {
            return;
        }
        Self::fill_rect(x, y, w, 1, color);
        Self::fill_rect(x, y + h - 1, w, 1, color);
        Self::fill_rect(x, y, 1, h, color);
        Self::fill_rect(x + w - 1, y, 1, h, color);
    }

    /// Raíz cuadrada entera rápida (100% bare-metal sin FPU ni punto flotante)
    #[inline(always)]
    pub fn isqrt(n: usize) -> usize {
        let mut x = n;
        let mut c = 0;
        let mut d = 1 << 30;
        while d > n {
            d >>= 2;
        }
        while d != 0 {
            if x >= c + d {
                x -= c + d;
                c = (c >> 1) + d;
            } else {
                c >>= 1;
            }
            d >>= 2;
        }
        c
    }

    /// Rellena un círculo sólido con centro en (cx, cy) y radio r
    pub fn fill_circle(cx: usize, cy: usize, r: usize, color: u32) {
        if r == 0 {
            Self::put_pixel(cx, cy, color);
            return;
        }
        let r_i = r as isize;
        for dy in -r_i..=r_i {
            let dy_sq = (dy * dy) as usize;
            if dy_sq <= r * r {
                let dx = Self::isqrt(r * r - dy_sq);
                let py = (cy as isize + dy) as usize;
                let px = (cx as isize - dx as isize).max(0) as usize;
                let w = dx * 2 + 1;
                Self::fill_rect(px, py, w, 1, color);
            }
        }
    }

    /// Dibuja el contorno de un círculo hueco
    pub fn draw_circle(cx: usize, cy: usize, r: usize, color: u32) {
        if r == 0 {
            Self::put_pixel(cx, cy, color);
            return;
        }
        let r_i = r as isize;
        for dy in -r_i..=r_i {
            let dy_sq = (dy * dy) as usize;
            if dy_sq <= r * r {
                let dx = Self::isqrt(r * r - dy_sq);
                let py = (cy as isize + dy) as usize;
                let p1 = (cx as isize - dx as isize).max(0) as usize;
                let p2 = (cx as isize + dx as isize).min((SCREEN_WIDTH - 1) as isize) as usize;
                Self::put_pixel(p1, py, color);
                Self::put_pixel(p2, py, color);
            }
        }
    }

    /// Rellena un rectángulo con esquinas redondeadas (Border-Radius moderno)
    pub fn fill_rounded_rect(x: usize, y: usize, w: usize, h: usize, r: usize, color: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let rad = r.min(w / 2).min(h / 2);
        if rad == 0 {
            Self::fill_rect(x, y, w, h, color);
            return;
        }

        // Centro vertical y horizontal
        Self::fill_rect(x, y + rad, w, h.saturating_sub(2 * rad), color);
        let mid_w = w.saturating_sub(2 * rad);
        if mid_w > 0 {
            Self::fill_rect(x + rad, y, mid_w, rad, color);
            Self::fill_rect(x + rad, y + h.saturating_sub(rad), mid_w, rad, color);
        }

        // Rellenar las 4 esquinas circulares
        for dy in 0..rad {
            let offset_y = rad - 1 - dy;
            let dx = Self::isqrt(rad * rad - offset_y * offset_y);
            // Sup-izq
            Self::fill_rect(x + rad - dx, y + dy, dx, 1, color);
            // Sup-der
            Self::fill_rect(x + w - rad, y + dy, dx, 1, color);
            // Inf-izq
            Self::fill_rect(x + rad - dx, y + h - 1 - dy, dx, 1, color);
            // Inf-der
            Self::fill_rect(x + w - rad, y + h - 1 - dy, dx, 1, color);
        }
    }

    /// Dibuja el contorno de un rectángulo con esquinas redondeadas
    pub fn draw_rounded_rect(x: usize, y: usize, w: usize, h: usize, r: usize, color: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let rad = r.min(w / 2).min(h / 2);
        if rad == 0 {
            Self::draw_rect(x, y, w, h, color);
            return;
        }
        // Segmentos rectos
        Self::fill_rect(x + rad, y, w.saturating_sub(2 * rad), 1, color);
        Self::fill_rect(x + rad, y + h - 1, w.saturating_sub(2 * rad), 1, color);
        Self::fill_rect(x, y + rad, 1, h.saturating_sub(2 * rad), color);
        Self::fill_rect(x + w - 1, y + rad, 1, h.saturating_sub(2 * rad), color);

        // Arcos de las 4 esquinas
        for dy in 0..rad {
            let offset_y = rad - 1 - dy;
            let dx = Self::isqrt(rad * rad - offset_y * offset_y);
            let prev_dx = if dy > 0 {
                let p_off = rad - dy;
                Self::isqrt(rad * rad - p_off * p_off)
            } else {
                0
            };
            let seg_w = (dx.saturating_sub(prev_dx)).max(1);

            Self::fill_rect(x + rad - dx, y + dy, seg_w, 1, color);
            Self::fill_rect(x + w - rad + dx - seg_w, y + dy, seg_w, 1, color);
            Self::fill_rect(x + rad - dx, y + h - 1 - dy, seg_w, 1, color);
            Self::fill_rect(x + w - rad + dx - seg_w, y + h - 1 - dy, seg_w, 1, color);
        }
    }

    /// Dibuja un gradiente vertical uniforme optimizado con rep stosd en RAM
    pub fn draw_gradient_v(x: usize, y: usize, w: usize, h: usize, top: u32, bottom: u32) {
        if h == 0 || w == 0 {
            return;
        }
        let max_x = (x + w).min(SCREEN_WIDTH);
        let max_y = (y + h).min(SCREEN_HEIGHT);
        if max_x <= x || max_y <= y {
            return;
        }
        let row_w = max_x - x;
        let ptr = Self::backbuffer_ptr();
        for i in 0..(max_y - y) {
            let factor = ((i * 255) / h) as u8;
            let c = Color::blend(top, bottom, factor);
            let row_offset = (y + i) * SCREEN_WIDTH + x;
            unsafe {
                let slice = core::slice::from_raw_parts_mut(ptr.add(row_offset), row_w);
                slice.fill(c);
            }
        }
    }

    /// Renderiza una imagen BMP en el búfer secundario
    pub fn draw_bmp(x: usize, y: usize, bmp: &crate::gui::bmp::BmpImage) {
        bmp.render_at(x, y);
    }

    /// Renderiza un carácter tipográfico usando el mapa de bits 8x16 en RAM
    pub fn draw_char(x: usize, y: usize, c: char, fg: u32, bg: Option<u32>) {
        let glyph = get_glyph(c);
        let ptr = Self::backbuffer_ptr();

        for row in 0..16 {
            let py = y + row;
            if py >= SCREEN_HEIGHT {
                break;
            }
            let byte = glyph[row];
            let row_offset = py * SCREEN_WIDTH;

            for col in 0..8 {
                let px = x + col;
                if px >= SCREEN_WIDTH {
                    break;
                }
                let bit = (byte >> (7 - col)) & 1;
                if bit == 1 {
                    unsafe {
                        *ptr.add(row_offset + px) = fg;
                    }
                } else if let Some(bg_color) = bg {
                    unsafe {
                        *ptr.add(row_offset + px) = bg_color;
                    }
                }
            }
        }
    }

    /// Dibuja una cadena de texto en el búfer secundario (monoespaciado 8px estándar)
    pub fn draw_string(x: usize, y: usize, text: &str, fg: u32, bg: Option<u32>) {
        let mut curr_x = x;
        for c in text.chars() {
            if c == '\n' {
                break;
            }
            Self::draw_char(curr_x, y, c, fg, bg);
            curr_x += 8;
        }
    }

    /// Dibuja un carácter con ancho proporcional recortando los espacios vacíos
    pub fn draw_char_prop(x: usize, y: usize, c: char, fg: u32, bg: Option<u32>) -> usize {
        let (min_col, glyph_w, advance) = crate::gui::font::get_glyph_metrics(c);
        if glyph_w == 0 {
            if let Some(bg_color) = bg {
                Self::fill_rect(x, y, advance, 16, bg_color);
            }
            return advance;
        }

        let glyph = get_glyph(c);
        let ptr = Self::backbuffer_ptr();

        if let Some(bg_color) = bg {
            Self::fill_rect(x, y, advance, 16, bg_color);
        }

        for row in 0..16 {
            let py = y + row;
            if py >= SCREEN_HEIGHT {
                break;
            }
            let byte = glyph[row];
            if byte == 0 {
                continue;
            }
            let row_offset = py * SCREEN_WIDTH;

            for col in min_col..=min_col + glyph_w - 1 {
                let bit = (byte >> (7 - col)) & 1;
                if bit == 1 {
                    let px = x + (col - min_col);
                    if px < SCREEN_WIDTH {
                        unsafe {
                            *ptr.add(row_offset + px) = fg;
                        }
                    }
                }
            }
        }

        advance
    }

    /// Dibuja una cadena con tipografía proporcional de alta definición y kerning natural (Estilo Lord OS)
    pub fn draw_string_prop(x: usize, y: usize, text: &str, fg: u32, bg: Option<u32>) -> usize {
        let mut curr_x = x;
        for c in text.chars() {
            if c == '\n' {
                break;
            }
            curr_x += Self::draw_char_prop(curr_x, y, c, fg, bg);
        }
        curr_x - x
    }

    /// Calcula la longitud horizontal en píxeles de una cadena con tipografía proporcional
    pub fn string_width_prop(text: &str) -> usize {
        let mut total_w = 0;
        for c in text.chars() {
            if c == '\n' {
                break;
            }
            let (_, _, adv) = crate::gui::font::get_glyph_metrics(c);
            total_w += adv;
        }
        total_w
    }

    /// Renderiza un Wallpaper Cósmico y Espacial de Alta Fidelidad (Lord OS / Deep Space Style)
    /// Combina gradiente atmosférico, campo estelar determinista y nubes de nebulosa con alpha blending
    pub fn draw_cosmic_wallpaper(w: usize, h: usize) {
        // 1. Gradiente base cósmico (Obsidiana -> Azul Zafiro -> Índigo Profundo)
        Self::draw_gradient_v(0, 0, w, h, 0x050814, 0x141634);

        // 2. Campo estelar determinista (LCG PRNG rápido sin flotantes)
        let mut seed: u32 = 0xBEA7E8;
        for _ in 0..140 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let sx = (seed as usize) % w;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let sy = ((seed as usize) % (h.saturating_sub(60))).max(30);
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let star_type = seed % 10;

            match star_type {
                0..=6 => {
                    // Estrella distante suave (1px)
                    Self::put_pixel(sx, sy, 0x64748B);
                }
                7..=8 => {
                    // Estrella media brillante (1px blanco azulado)
                    Self::put_pixel(sx, sy, 0xBAE6FD);
                }
                _ => {
                    // Estrella gigante con halo / cruz (diffraction spike)
                    Self::put_pixel(sx, sy, 0xFFFFFF);
                    if sx > 0 && sx + 1 < w {
                        Self::put_pixel(sx - 1, sy, 0x38BDF8);
                        Self::put_pixel(sx + 1, sy, 0x38BDF8);
                    }
                    if sy > 0 && sy + 1 < h {
                        Self::put_pixel(sx, sy - 1, 0x38BDF8);
                        Self::put_pixel(sx, sy + 1, 0x38BDF8);
                    }
                }
            }
        }

        // 3. Nubes de Nebulosa y Aurora en el espacio (Glacial Teal y Violeta Cósmico)
        // Nebulosa Teal 1: Centro (280, 180)
        let cx1: isize = 280;
        let cy1: isize = 180;
        let r1: isize = 120;
        for dy in -r1..=r1 {
            let py = (cy1 + dy) as usize;
            if py >= h { continue; }
            let dy_sq = dy * dy;
            let max_dx = Self::isqrt((r1 * r1 - dy_sq) as usize) as isize;
            for dx in -max_dx..=max_dx {
                let px = (cx1 + dx) as usize;
                if px >= w { continue; }
                let dist_sq = (dx * dx + dy_sq) as usize;
                let factor = 35 - ((dist_sq * 35) / (r1 * r1 as isize) as usize) as u8;
                let orig = unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) };
                let blended = Color::blend(orig, Color::BEAVER_TEAL, factor);
                unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) = blended; }
            }
        }

        // Nebulosa Violeta 2: Centro (760, 240)
        let cx2: isize = 760;
        let cy2: isize = 240;
        let r2: isize = 150;
        for dy in -r2..=r2 {
            let py = (cy2 + dy) as usize;
            if py >= h { continue; }
            let dy_sq = dy * dy;
            let max_dx = Self::isqrt((r2 * r2 - dy_sq) as usize) as isize;
            for dx in -max_dx..=max_dx {
                let px = (cx2 + dx) as usize;
                if px >= w { continue; }
                let dist_sq = (dx * dx + dy_sq) as usize;
                let factor = 30 - ((dist_sq * 30) / (r2 * r2 as isize) as usize) as u8;
                let orig = unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) };
                let blended = Color::blend(orig, 0x7C3AED, factor); // Violeta brillante
                unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) = blended; }
            }
        }

        // Nebulosa Ámbar 3: Centro (500, 320)
        let cx3: isize = 500;
        let cy3: isize = 320;
        let r3: isize = 100;
        for dy in -r3..=r3 {
            let py = (cy3 + dy) as usize;
            if py >= h { continue; }
            let dy_sq = dy * dy;
            let max_dx = Self::isqrt((r3 * r3 - dy_sq) as usize) as isize;
            for dx in -max_dx..=max_dx {
                let px = (cx3 + dx) as usize;
                if px >= w { continue; }
                let dist_sq = (dx * dx + dy_sq) as usize;
                let factor = 22 - ((dist_sq * 22) / (r3 * r3 as isize) as usize) as u8;
                let orig = unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) };
                let blended = Color::blend(orig, Color::BEAVER_AMBER, factor);
                unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) = blended; }
            }
        }

        // Marca de agua sutil y elegante con tipografía proporcional
        let wm_text = "BEAVER OS  *  UNIVERSAL MESH SYSTEM";
        let wm_w = Self::string_width_prop(wm_text);
        Self::draw_string_prop((w.saturating_sub(wm_w)) / 2, h / 2 - 10, wm_text, 0x1E2B4C, None);
    }

    /// Dibuja el Fondo de Pantalla Nórdico Claro de Alta Fidelidad (Modo Día / Lord OS Light Theme)
    pub fn draw_nordic_wallpaper(w: usize, h: usize) {
        // 1. Degradado vertical suave de Blanco Perla a Gris Glacial Nórdico
        let top_color = 0xF8FAFC;
        let bot_color = 0xCBD5E1;
        for y in 0..h {
            let factor = ((y * 255) / h) as u8;
            let row_color = Color::blend(top_color, bot_color, factor);
            Self::fill_rect(0, y, w, 1, row_color);
        }

        // 2. Ondas y esferas de luz con acentos sutiles Beaver (Ámbar y Glacial Teal)
        let cx1: isize = 240;
        let cy1: isize = 200;
        let r1: isize = 160;
        for dy in -r1..=r1 {
            let py = (cy1 + dy) as usize;
            if py >= h { continue; }
            let dy_sq = dy * dy;
            let max_dx = Self::isqrt((r1 * r1 - dy_sq) as usize) as isize;
            for dx in -max_dx..=max_dx {
                let px = (cx1 + dx) as usize;
                if px >= w { continue; }
                let dist_sq = (dx * dx + dy_sq) as usize;
                let factor = 22 - ((dist_sq * 22) / (r1 * r1 as isize) as usize) as u8;
                let orig = unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) };
                let blended = Color::blend(orig, Color::BEAVER_TEAL_BRIGHT, factor);
                unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) = blended; }
            }
        }

        let cx2: isize = 780;
        let cy2: isize = 260;
        let r2: isize = 180;
        for dy in -r2..=r2 {
            let py = (cy2 + dy) as usize;
            if py >= h { continue; }
            let dy_sq = dy * dy;
            let max_dx = Self::isqrt((r2 * r2 - dy_sq) as usize) as isize;
            for dx in -max_dx..=max_dx {
                let px = (cx2 + dx) as usize;
                if px >= w { continue; }
                let dist_sq = (dx * dx + dy_sq) as usize;
                let factor = 20 - ((dist_sq * 20) / (r2 * r2 as isize) as usize) as u8;
                let orig = unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) };
                let blended = Color::blend(orig, Color::BEAVER_AMBER_LIGHT, factor);
                unsafe { *Self::backbuffer_ptr().add(py * SCREEN_WIDTH + px) = blended; }
            }
        }

        let wm_text = "BEAVER OS  *  NORDIC LIGHT DESKTOP";
        let wm_w = Self::string_width_prop(wm_text);
        Self::draw_string_prop((w.saturating_sub(wm_w)) / 2, h / 2 - 10, wm_text, 0x94A3B8, None);
    }

    /// Dibuja el marco completo de una ventana del sistema operativo estilo Lord OS
    /// Incorpora esquinas redondeadas, sombra suave multi-capa y semáforos circulares con brillo de gema
    pub fn draw_window(x: usize, y: usize, w: usize, h: usize, title: &str, active: bool) {
        if w < 60 || h < 40 {
            return;
        }

        // 1. Sombra suave difusa (Drop Shadow)
        Self::fill_rounded_rect(x + 6, y + 6, w, h, 12, 0x070B16);
        Self::fill_rounded_rect(x + 3, y + 3, w, h, 11, 0x0F172A);

        // 2. Fondo principal de la ventana (Titanio Nórdico)
        Self::fill_rounded_rect(x, y, w, h, 10, Color::BEAVER_TITANIUM);

        // 3. Barra de Título (Titlebar) unificada con esquinas superiores redondeadas
        let tb_color = if active { Color::WINDOW_TITLEBAR_ACT } else { Color::WINDOW_TITLEBAR };
        Self::fill_rounded_rect(x, y, w, 32, 10, tb_color);
        // Aplana la base de la barra de título para conectar limpiamente con el cuerpo
        Self::fill_rect(x, y + 20, w, 12, tb_color);
        Self::draw_line_h(x + 1, y + 31, w.saturating_sub(2), Color::BEAVER_BORDER_LIGHT);

        // 4. Semáforos circulares con reflejos especulares estilo Lord OS / Mac
        let cy = y + 15;

        // Botón Cerrar (Ruby)
        Self::fill_circle(x + 16, cy, 6, Color::BEAVER_RUBY);
        Self::draw_circle(x + 16, cy, 6, 0xDC2626);
        Self::put_pixel(x + 15, cy.saturating_sub(1), Color::WHITE);

        // Botón Minimizar (Amber)
        Self::fill_circle(x + 32, cy, 6, Color::BEAVER_AMBER);
        Self::draw_circle(x + 32, cy, 6, Color::BEAVER_AMBER_DARK);
        Self::put_pixel(x + 31, cy.saturating_sub(1), Color::WHITE);

        // Botón Maximizar (Emerald)
        Self::fill_circle(x + 48, cy, 6, Color::BEAVER_EMERALD);
        Self::draw_circle(x + 48, cy, 6, 0x059669);
        Self::put_pixel(x + 47, cy.saturating_sub(1), Color::WHITE);

        // 5. Contorno perimetral redondeado de la ventana
        let border_color = if active { Color::BEAVER_ACCENT_TEAL } else { Color::BEAVER_BORDER_LIGHT };
        Self::draw_rounded_rect(x, y, w, h, 10, border_color);

        // 6. Título de la ventana con alta legibilidad tipográfica proporcional (Carbón Suave)
        Self::draw_string_prop(x + 64, y + 8, title, Color::BEAVER_TEXT_DARK, None);
    }

    /// Dibuja una barra de progreso porcentual
    pub fn draw_progress_bar(x: usize, y: usize, w: usize, h: usize, pct: usize, fill_color: u32, bg_color: u32) {
        Self::fill_rect(x, y, w, h, bg_color);
        Self::draw_rect(x, y, w, h, Color::SLATE_600);
        let clamped_pct = pct.min(100);
        let fill_w = ((w - 2) * clamped_pct) / 100;
        if fill_w > 0 {
            Self::fill_rect(x + 1, y + 1, fill_w, h - 2, fill_color);
        }
    }

    /// Page Flipping / Presentación atómica del Búfer Secundario al Framebuffer VBE/GOP
    /// Transfiere 1024x768 píxeles respetando la alineación de scanline de UEFI GOP o VBE
    pub fn present() {
        let vbe = VBE.lock();
        if !vbe.is_active {
            return;
        }
        let src = Self::backbuffer_ptr();
        let dst = vbe.lfb_ptr();
        let stride = if vbe.width > 0 { vbe.width } else { SCREEN_WIDTH };

        if stride == SCREEN_WIDTH {
            unsafe {
                core::ptr::copy_nonoverlapping(src as *const u64, dst as *mut u64, TOTAL_PIXELS / 2);
            }
        } else {
            let row_w = SCREEN_WIDTH.min(vbe.width);
            let total_rows = SCREEN_HEIGHT.min(vbe.height);
            for y in 0..total_rows {
                unsafe {
                    core::ptr::copy_nonoverlapping(src.add(y * SCREEN_WIDTH), dst.add(y * stride), row_w);
                }
            }
        }
    }

    /// Presentación rápida de una región sucia rectangular (Dirty Rect Presentation)
    pub fn present_rect(x: usize, y: usize, w: usize, h: usize) {
        let vbe = VBE.lock();
        if !vbe.is_active {
            return;
        }
        let max_x = (x + w).min(SCREEN_WIDTH).min(vbe.width);
        let max_y = (y + h).min(SCREEN_HEIGHT).min(vbe.height);
        if max_x <= x || max_y <= y {
            return;
        }

        let src = Self::backbuffer_ptr();
        let dst = vbe.lfb_ptr();
        let row_count = max_x - x;
        let stride = if vbe.width > 0 { vbe.width } else { SCREEN_WIDTH };

        for py in y..max_y {
            let src_offset = py * SCREEN_WIDTH + x;
            let dst_offset = py * stride + x;
            unsafe {
                core::ptr::copy_nonoverlapping(src.add(src_offset), dst.add(dst_offset), row_count);
            }
        }
    }

    /// Alias ergonómico para presentación de doble búfer
    pub fn swap_buffers() {
        Self::present();
    }

    pub fn invalidate_cursor() {
        unsafe {
            CURSOR_ACTIVE = false;
        }
    }

    pub fn restore_cursor() {
        unsafe {
            if !CURSOR_ACTIVE {
                return;
            }
            let vbe = VBE.lock();
            if !vbe.is_active {
                CURSOR_ACTIVE = false;
                return;
            }
            let back_ptr = Self::backbuffer_ptr();
            let lfb_ptr = vbe.lfb_ptr();
            let prev_x = CURSOR_PREV_X;
            let prev_y = CURSOR_PREV_Y;

            for r in 0..CURSOR_H {
                let py = prev_y + r;
                if py >= SCREEN_HEIGHT {
                    break;
                }
                let row_offset = py * SCREEN_WIDTH;
                for c in 0..CURSOR_W {
                    let px = prev_x + c;
                    if px >= SCREEN_WIDTH {
                        break;
                    }
                    let restored = CURSOR_BG[r * CURSOR_W + c];
                    *back_ptr.add(row_offset + px) = restored;
                    *lfb_ptr.add(row_offset + px) = restored;
                }
            }
            CURSOR_ACTIVE = false;
        }
    }

    pub fn draw_cursor_at(x: usize, y: usize) {
        let vbe = VBE.lock();
        if !vbe.is_active {
            return;
        }
        let back_ptr = Self::backbuffer_ptr();
        let lfb_ptr = vbe.lfb_ptr();

        unsafe {
            if CURSOR_ACTIVE {
                let prev_x = CURSOR_PREV_X;
                let prev_y = CURSOR_PREV_Y;
                for r in 0..CURSOR_H {
                    let py = prev_y + r;
                    if py >= SCREEN_HEIGHT {
                        break;
                    }
                    let row_offset = py * SCREEN_WIDTH;
                    for c in 0..CURSOR_W {
                        let px = prev_x + c;
                        if px >= SCREEN_WIDTH {
                            break;
                        }
                        let restored = CURSOR_BG[r * CURSOR_W + c];
                        *back_ptr.add(row_offset + px) = restored;
                        *lfb_ptr.add(row_offset + px) = restored;
                    }
                }
            }

            for r in 0..CURSOR_H {
                let py = y + r;
                if py >= SCREEN_HEIGHT {
                    break;
                }
                let row_offset = py * SCREEN_WIDTH;
                for c in 0..CURSOR_W {
                    let px = x + c;
                    if px >= SCREEN_WIDTH {
                        break;
                    }
                    CURSOR_BG[r * CURSOR_W + c] = *back_ptr.add(row_offset + px);
                }
            }

            for r in 0..CURSOR_H {
                let py = y + r;
                if py >= SCREEN_HEIGHT {
                    break;
                }
                let row_offset = py * SCREEN_WIDTH;
                for c in 0..CURSOR_W {
                    let px = x + c;
                    if px >= SCREEN_WIDTH {
                        break;
                    }
                    let col = match CURSOR_SHAPE[r][c] {
                        1 => Some(Color::BLACK),
                        2 => Some(Color::WHITE),
                        _ => None,
                    };
                    if let Some(color) = col {
                        *back_ptr.add(row_offset + px) = color;
                        *lfb_ptr.add(row_offset + px) = color;
                    }
                }
            }

            CURSOR_PREV_X = x;
            CURSOR_PREV_Y = y;
            CURSOR_ACTIVE = true;
        }
    }

    pub fn try_draw_cursor_at(x: usize, y: usize) -> bool {
        if let Some(vbe) = VBE.try_lock() {
            if !vbe.is_active {
                return false;
            }
            let back_ptr = Self::backbuffer_ptr();
            let lfb_ptr = vbe.lfb_ptr();

            unsafe {
                if CURSOR_ACTIVE {
                    let prev_x = CURSOR_PREV_X;
                    let prev_y = CURSOR_PREV_Y;
                    for r in 0..CURSOR_H {
                        let py = prev_y + r;
                        if py >= SCREEN_HEIGHT {
                            break;
                        }
                        let row_offset = py * SCREEN_WIDTH;
                        for c in 0..CURSOR_W {
                            let px = prev_x + c;
                            if px >= SCREEN_WIDTH {
                                break;
                            }
                            let restored = CURSOR_BG[r * CURSOR_W + c];
                            *back_ptr.add(row_offset + px) = restored;
                            *lfb_ptr.add(row_offset + px) = restored;
                        }
                    }
                }

                for r in 0..CURSOR_H {
                    let py = y + r;
                    if py >= SCREEN_HEIGHT {
                        break;
                    }
                    let row_offset = py * SCREEN_WIDTH;
                    for c in 0..CURSOR_W {
                        let px = x + c;
                        if px >= SCREEN_WIDTH {
                            break;
                        }
                        CURSOR_BG[r * CURSOR_W + c] = *back_ptr.add(row_offset + px);
                    }
                }

                for r in 0..CURSOR_H {
                    let py = y + r;
                    if py >= SCREEN_HEIGHT {
                        break;
                    }
                    let row_offset = py * SCREEN_WIDTH;
                    for c in 0..CURSOR_W {
                        let px = x + c;
                        if px >= SCREEN_WIDTH {
                            break;
                        }
                        let col = match CURSOR_SHAPE[r][c] {
                            1 => Some(Color::BLACK),
                            2 => Some(Color::WHITE),
                            _ => None,
                        };
                        if let Some(color) = col {
                            *back_ptr.add(row_offset + px) = color;
                            *lfb_ptr.add(row_offset + px) = color;
                        }
                    }
                }

                CURSOR_PREV_X = x;
                CURSOR_PREV_Y = y;
                CURSOR_ACTIVE = true;
            }
            true
        } else {
            false
        }
    }

    pub fn draw_cursor(x: usize, y: usize) {
        Self::draw_cursor_at(x, y);
    }
}

pub const CURSOR_W: usize = 11;
pub const CURSOR_H: usize = 16;

static mut CURSOR_BG: [u32; CURSOR_W * CURSOR_H] = [0; CURSOR_W * CURSOR_H];
static mut CURSOR_PREV_X: usize = 300;
static mut CURSOR_PREV_Y: usize = 200;
static mut CURSOR_ACTIVE: bool = false;

pub const CURSOR_SHAPE: [[u8; CURSOR_W]; CURSOR_H] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0],
    [1, 2, 2, 2, 2, 1, 1, 1, 1, 1, 0],
    [1, 2, 1, 2, 2, 1, 0, 0, 0, 0, 0],
    [1, 1, 0, 1, 2, 2, 1, 0, 0, 0, 0],
    [1, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0],
    [0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];
