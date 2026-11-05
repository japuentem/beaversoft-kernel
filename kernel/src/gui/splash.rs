// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Product: Beaver OS
// Description: Pantalla Grafica de Arranque y Splash Screen (UEFI GOP 1024x768)
// ============================================================================

use crate::gui::canvas::Canvas;
use crate::drivers::vbe::VBE;

static SPLASH_BANNER_BYTES: &[u8] = include_bytes!("../../../assets/splash_banner.bin");

/// Renderiza la pantalla gráfica de presentación (Splash Screen) en modo UEFI GOP
pub fn show_boot_splash() {
    let (width, height) = {
        let vbe = VBE.lock();
        if !vbe.is_active {
            return;
        }
        (vbe.width, vbe.height)
    };

    if SPLASH_BANNER_BYTES.len() < 4 {
        return;
    }

    let img_w = u16::from_le_bytes([SPLASH_BANNER_BYTES[0], SPLASH_BANNER_BYTES[1]]) as usize;
    let img_h = u16::from_le_bytes([SPLASH_BANNER_BYTES[2], SPLASH_BANNER_BYTES[3]]) as usize;

    // 1. Fondo atmosférico de inicio (Deep Obsidian Cosmic Gradient)
    Canvas::draw_gradient_v(0, 0, width, height, 0x050814, 0x0D1117);

    // 2. Renderizar el logotipo centrado
    let start_x = if width > img_w { (width - img_w) / 2 } else { 0 };
    let start_y = if height > img_h { (height.saturating_sub(img_h + 120)) / 2 } else { 20 };

    let mut px = 0;
    let mut py = 0;
    let mut offset = 4;

    while offset + 3 < SPLASH_BANNER_BYTES.len() && py < img_h {
        let count = SPLASH_BANNER_BYTES[offset] as usize;
        let r = SPLASH_BANNER_BYTES[offset + 1] as u32;
        let g = SPLASH_BANNER_BYTES[offset + 2] as u32;
        let b = SPLASH_BANNER_BYTES[offset + 3] as u32;
        let color = (r << 16) | (g << 8) | b;
        offset += 4;

        for _ in 0..count {
            if color != 0 {
                Canvas::put_pixel(start_x + px, start_y + py, color);
            }
            px += 1;
            if px >= img_w {
                px = 0;
                py += 1;
                if py >= img_h {
                    break;
                }
            }
        }
    }

    // 3. Indicadores de telemetría y bienvenida
    let center_x = width / 2;
    let status_y = start_y + img_h + 30;

    // Subtítulo estilizado
    Canvas::draw_string_prop(center_x.saturating_sub(115), status_y, "INICIANDO BEAVER OS v0.9.0", 0x38BDF8, None);
    Canvas::draw_string_prop(center_x.saturating_sub(165), status_y + 22, "Arquitectura x86_64 Long Mode | SMP Cores Activos", 0x94A3B8, None);

    // Barra de carga de progreso sutil
    let bar_w = 340;
    let bar_x = center_x.saturating_sub(bar_w / 2);
    let bar_y = status_y + 48;
    Canvas::fill_rounded_rect(bar_x, bar_y, bar_w, 4, 2, 0x1E293B);
    Canvas::fill_rounded_rect(bar_x, bar_y, bar_w, 4, 2, 0x38BDF8);

    // Pausa breve para apreciación visual (aprox 30-40 ms en hardware)
    for _ in 0..12_000_000 {
        core::hint::spin_loop();
    }
}
