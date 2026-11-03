// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Vector & Procedural Graphic Icons Engine (Lord OS Style)
//              3D Folders, Badged Documents, Media Controls, and Device Glyphs
// ============================================================================

use crate::gui::canvas::{Canvas, Color};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SidebarIconKind {
    Home,
    Disk,
    Mesh,
    Music,
    Terminal,
    Settings,
    Trash,
    Photos,
    Calendar,
    Search,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MediaBtnKind {
    Play,
    Pause,
    Previous,
    Next,
    Shuffle,
    Repeat,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DeviceKind {
    Pc,
    Laptop,
    Server,
}

/// Dibuja una carpeta tridimensional estilo Beaver OS / Lord OS
/// Presenta pestaña posterior, documentos blancos asomando y solapa frontal en relieve ámbar/dorado
pub fn draw_beaver_folder(x: usize, y: usize, w: usize, h: usize, is_selected: bool) {
    if w < 24 || h < 20 {
        return;
    }

    let tab_w = (w * 45) / 100;
    let tab_h = (h * 28) / 100;

    let tab_color = if is_selected { Color::BEAVER_TEAL_BRIGHT } else { Color::BEAVER_AMBER_TABS };
    let flap_color = if is_selected { Color::BEAVER_TEAL_BRIGHT } else { Color::BEAVER_AMBER };
    let dark_edge = if is_selected { 0x0F766E } else { Color::BEAVER_AMBER_DARK };
    let highlight = if is_selected { 0x99F6E4 } else { Color::BEAVER_AMBER_LIGHT };

    // Sombra suave proyectada bajo la carpeta
    Canvas::fill_rounded_rect(x + 2, y + h - 3, w - 4, 5, 2, 0x1A202C);

    // 1. Pestaña posterior (esquina superior izquierda)
    Canvas::fill_rounded_rect(x, y, tab_w, tab_h + 4, 3, tab_color);

    // 2. Fondo de la carpeta (cuerpo trasero)
    Canvas::fill_rounded_rect(x, y + tab_h - 2, w, h - tab_h + 2, 4, dark_edge);

    // 3. Documentos u hojas blancas asomando dentro de la carpeta
    let sheet_x = x + 3;
    let sheet_y = y + tab_h - 1;
    let sheet_w = w.saturating_sub(6);
    Canvas::fill_rect(sheet_x, sheet_y, sheet_w, 6, Color::WHITE);
    Canvas::draw_line_h(sheet_x + 2, sheet_y + 2, sheet_w.saturating_sub(4), Color::SLATE_400);

    // 4. Solapa frontal con volumen tridimensional
    let front_y = y + tab_h + 3;
    let front_h = h.saturating_sub(tab_h + 3);
    Canvas::fill_rounded_rect(x, front_y, w, front_h, 4, flap_color);

    // Resplandor superior en la solapa frontal
    Canvas::fill_rect(x + 2, front_y + 1, w.saturating_sub(4), 1, highlight);

    // Borde inferior sutil de relieve
    Canvas::draw_line_h(x + 2, front_y + front_h - 1, w.saturating_sub(4), dark_edge);
}

/// Dibuja un documento con esquina superior derecha doblada (Dog-Ear Fold) y badge coloreado
pub fn draw_file_document(x: usize, y: usize, w: usize, h: usize, badge: &str, badge_color: u32) {
    if w < 20 || h < 24 {
        return;
    }

    let fold = 7; // Tamaño del pliegue de la esquina

    // Sombra suave
    Canvas::fill_rounded_rect(x + 2, y + 2, w, h, 3, 0x1A202C);

    // Cuerpo principal de la hoja (blanco marfil)
    Canvas::fill_rounded_rect(x, y, w, h, 3, 0xFFFFFF);

    // Pliegue de la esquina superior derecha
    // Rellenamos el corte con el fondo de la ventana
    for i in 0..fold {
        let py = y + i;
        let cut_w = fold - i;
        Canvas::fill_rect(x + w - cut_w, py, cut_w, 1, Color::BEAVER_TITANIUM);
    }
    // Dibuja la solapa doblada triangular
    for i in 0..fold {
        let py = y + i;
        let px = x + w - fold;
        Canvas::fill_rect(px, py, i + 1, 1, Color::SLATE_200);
    }
    Canvas::draw_line_v(x + w - fold, y, fold, Color::SLATE_400);
    Canvas::draw_line_h(x + w - fold, y + fold, fold, Color::SLATE_400);

    // Líneas simuladas de contenido de texto
    let line_w = w.saturating_sub(12);
    Canvas::fill_rect(x + 6, y + 10, line_w - 4, 2, Color::SLATE_200);
    Canvas::fill_rect(x + 6, y + 15, line_w, 2, Color::SLATE_200);
    Canvas::fill_rect(x + 6, y + 20, line_w - 6, 2, Color::SLATE_200);

    // Badge rectangular inferior con la extensión (ej: WAV, BMP, TXT, ELF)
    let badge_h = 13;
    let badge_y = y + h - badge_h - 3;
    let badge_w = w.saturating_sub(6);
    Canvas::fill_rounded_rect(x + 3, badge_y, badge_w, badge_h, 2, badge_color);

    // Texto de la extensión centrado en el badge
    let text_x = x + 3 + (badge_w.saturating_sub(badge.len() * 8)) / 2;
    Canvas::draw_string(text_x, badge_y + 2, badge, Color::WHITE, None);
}

/// Dibuja un botón de reproducción multimedia circular con glifos vectoriales nítidos
pub fn draw_media_button(cx: usize, cy: usize, r: usize, kind: MediaBtnKind, is_hover: bool) {
    let bg_color = if is_hover {
        Color::BEAVER_AMBER
    } else {
        Color::BEAVER_TITANIUM_BAR
    };
    let glyph_color = if is_hover {
        Color::WHITE
    } else {
        Color::BEAVER_TEXT_DARK
    };

    // Fondo circular con sutil contorno
    Canvas::fill_circle(cx, cy, r, bg_color);
    Canvas::draw_circle(cx, cy, r, Color::BEAVER_BORDER_LIGHT);

    match kind {
        MediaBtnKind::Play => {
            // Triángulo apuntando a la derecha
            let size = r / 2;
            let start_x = cx.saturating_sub(size / 3);
            for dx in 0..size {
                let h = size - dx;
                let py = cy.saturating_sub(h);
                Canvas::fill_rect(start_x + dx, py, 1, h * 2 + 1, glyph_color);
            }
        }
        MediaBtnKind::Pause => {
            // Dos barras verticales
            let bar_w = (r / 4).max(2);
            let bar_h = r - 2;
            let sep = (r / 4).max(2);
            let top_y = cy.saturating_sub(bar_h / 2);
            Canvas::fill_rect(cx.saturating_sub(bar_w + sep / 2), top_y, bar_w, bar_h, glyph_color);
            Canvas::fill_rect(cx + sep / 2, top_y, bar_w, bar_h, glyph_color);
        }
        MediaBtnKind::Previous => {
            // Barra vertical + Triángulo a la izquierda
            let size = r / 2;
            let start_x = cx + size / 2;
            Canvas::fill_rect(cx.saturating_sub(size), cy.saturating_sub(size), 2, size * 2 + 1, glyph_color);
            for dx in 0..size {
                let h = dx;
                let py = cy.saturating_sub(h);
                Canvas::fill_rect(start_x.saturating_sub(dx), py, 1, h * 2 + 1, glyph_color);
            }
        }
        MediaBtnKind::Next => {
            // Triángulo a la derecha + Barra vertical
            let size = r / 2;
            let start_x = cx.saturating_sub(size / 2);
            for dx in 0..size {
                let h = size - dx;
                let py = cy.saturating_sub(h);
                Canvas::fill_rect(start_x + dx, py, 1, h * 2 + 1, glyph_color);
            }
            Canvas::fill_rect(cx + size, cy.saturating_sub(size), 2, size * 2 + 1, glyph_color);
        }
        MediaBtnKind::Shuffle => {
            // Flechas cruzadas simplificadas
            Canvas::draw_line_h(cx.saturating_sub(4), cy.saturating_sub(2), 8, glyph_color);
            Canvas::draw_line_h(cx.saturating_sub(4), cy + 2, 8, glyph_color);
            Canvas::put_pixel(cx + 4, cy.saturating_sub(3), glyph_color);
            Canvas::put_pixel(cx + 4, cy + 3, glyph_color);
        }
        MediaBtnKind::Repeat => {
            // Bucle circular abierto
            Canvas::draw_circle(cx, cy, 4, glyph_color);
            Canvas::put_pixel(cx + 4, cy.saturating_sub(1), bg_color);
            Canvas::put_pixel(cx + 3, cy.saturating_sub(2), glyph_color);
        }
    }
}

/// Dibuja iconos para las barras laterales estilo Lord OS (16x16)
pub fn draw_sidebar_icon(x: usize, y: usize, kind: SidebarIconKind, color: u32) {
    match kind {
        SidebarIconKind::Home => {
            // Casita con tejado a 2 aguas
            for i in 0..6 {
                let w = i * 2 + 1;
                Canvas::fill_rect(x + 7 - i, y + 2 + i, w, 1, color);
            }
            Canvas::fill_rect(x + 3, y + 8, 9, 6, color);
            // Puerta
            Canvas::fill_rect(x + 6, y + 10, 3, 4, Color::BEAVER_TITANIUM_BAR);
        }
        SidebarIconKind::Disk => {
            // Disco rígido / Cilindro
            Canvas::fill_rounded_rect(x + 1, y + 3, 14, 10, 2, color);
            Canvas::fill_circle(x + 11, y + 8, 1, Color::BEAVER_EMERALD); // LED verde
            Canvas::draw_line_h(x + 3, y + 6, 6, Color::WHITE);
        }
        SidebarIconKind::Mesh => {
            // Red de 3 nodos interconectados
            Canvas::fill_circle(x + 8, y + 3, 2, color);
            Canvas::fill_circle(x + 3, y + 11, 2, color);
            Canvas::fill_circle(x + 13, y + 11, 2, color);
            Canvas::draw_line_h(x + 4, y + 11, 8, color);
            Canvas::draw_line_v(x + 8, y + 5, 4, color);
        }
        SidebarIconKind::Music => {
            // Nota musical (corchea doble)
            Canvas::fill_circle(x + 4, y + 11, 2, color);
            Canvas::fill_circle(x + 11, y + 9, 2, color);
            Canvas::fill_rect(x + 5, y + 3, 2, 9, color);
            Canvas::fill_rect(x + 12, y + 1, 2, 9, color);
            Canvas::fill_rect(x + 5, y + 1, 8, 3, color);
        }
        SidebarIconKind::Terminal => {
            // Ventana de consola prompt `>_`
            Canvas::fill_rounded_rect(x + 1, y + 2, 14, 12, 2, Color::SLATE_800);
            Canvas::draw_string(x + 3, y + 3, ">", Color::BEAVER_AMBER, None);
            Canvas::fill_rect(x + 9, y + 10, 4, 2, Color::WHITE);
        }
        SidebarIconKind::Settings => {
            // Engranaje
            Canvas::fill_circle(x + 8, y + 8, 4, color);
            Canvas::fill_circle(x + 8, y + 8, 2, Color::BEAVER_TITANIUM_BAR);
            Canvas::fill_rect(x + 7, y + 2, 2, 12, color);
            Canvas::fill_rect(x + 2, y + 7, 12, 2, color);
        }
        SidebarIconKind::Trash => {
            // Papelera
            Canvas::fill_rect(x + 3, y + 4, 10, 2, color);
            Canvas::fill_rect(x + 6, y + 2, 4, 2, color);
            Canvas::fill_rounded_rect(x + 4, y + 6, 8, 8, 1, color);
            Canvas::draw_line_v(x + 6, y + 8, 4, Color::BEAVER_TITANIUM_BAR);
            Canvas::draw_line_v(x + 9, y + 8, 4, Color::BEAVER_TITANIUM_BAR);
        }
        SidebarIconKind::Photos => {
            // Marco de foto con montañas y sol
            Canvas::fill_rounded_rect(x + 1, y + 2, 14, 12, 2, color);
            Canvas::fill_rect(x + 3, y + 4, 10, 8, Color::SLATE_800);
            Canvas::fill_circle(x + 5, y + 6, 1, Color::BEAVER_AMBER); // Sol
            Canvas::fill_rect(x + 4, y + 9, 3, 3, Color::BEAVER_TEAL); // Montaña 1
            Canvas::fill_rect(x + 7, y + 8, 5, 4, Color::BEAVER_EMERALD); // Montaña 2
        }
        SidebarIconKind::Calendar => {
            // Hoja de calendario con anillos superiores y fecha roja
            Canvas::fill_rounded_rect(x + 1, y + 2, 14, 12, 2, color);
            Canvas::fill_rect(x + 1, y + 2, 14, 4, Color::BEAVER_RUBY); // Cabecera roja
            Canvas::fill_rect(x + 3, y + 8, 2, 2, Color::SLATE_800);
            Canvas::fill_rect(x + 7, y + 8, 2, 2, Color::SLATE_800);
            Canvas::fill_rect(x + 11, y + 8, 2, 2, Color::SLATE_800);
            // Anillos
            Canvas::draw_line_v(x + 4, y + 1, 2, Color::WHITE);
            Canvas::draw_line_v(x + 11, y + 1, 2, Color::WHITE);
        }
        SidebarIconKind::Search => {
            // Lupa
            Canvas::draw_circle(x + 6, y + 6, 4, color);
            Canvas::draw_line_h(x + 9, y + 9, 4, color);
            Canvas::draw_line_v(x + 12, y + 10, 3, color);
        }
    }
}

/// Dibuja insignias gráficas para dispositivos interconectados en BeaverSync (PC, Laptop, Server)
pub fn draw_device_icon(x: usize, y: usize, kind: DeviceKind, color: u32) {
    match kind {
        DeviceKind::Pc => {
            // Pantalla de escritorio con base
            Canvas::fill_rounded_rect(x, y, 22, 15, 2, color);
            Canvas::fill_rect(x + 2, y + 2, 18, 11, Color::SLATE_900);
            // Barra de soporte y peana
            Canvas::fill_rect(x + 9, y + 15, 4, 3, color);
            Canvas::fill_rect(x + 6, y + 18, 10, 2, color);
        }
        DeviceKind::Laptop => {
            // Pantalla de portátil abierta y teclado
            Canvas::fill_rounded_rect(x + 2, y, 18, 12, 2, color);
            Canvas::fill_rect(x + 4, y + 2, 14, 8, Color::SLATE_900);
            // Teclado inferior inclinado
            Canvas::fill_rounded_rect(x, y + 13, 22, 5, 1, color);
            Canvas::fill_rect(x + 8, y + 14, 6, 2, Color::SLATE_700); // Trackpad
        }
        DeviceKind::Server => {
            // Rack de servidor con bahías y luces LED
            Canvas::fill_rounded_rect(x + 2, y, 18, 19, 2, color);
            Canvas::draw_line_h(x + 4, y + 6, 14, Color::SLATE_700);
            Canvas::draw_line_h(x + 4, y + 12, 14, Color::SLATE_700);
            // Luces indicadoras de actividad verde y ámbar
            Canvas::fill_circle(x + 5, y + 3, 1, Color::BEAVER_EMERALD);
            Canvas::fill_circle(x + 8, y + 3, 1, Color::BEAVER_AMBER);
            Canvas::fill_circle(x + 5, y + 9, 1, Color::BEAVER_EMERALD);
            Canvas::fill_circle(x + 5, y + 15, 1, Color::BEAVER_EMERALD);
        }
    }
}
