// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Native Windows BMP Image Decoder & Bitmap Framebuffer Renderer
//              Supports 24-bit RGB, 32-bit RGBA, Scanline Padding & VFS Persistence
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use crate::gui::canvas::Canvas;
use crate::fs::vfs::VFS;

#[derive(Debug, Clone)]
pub struct BmpImage {
    pub width: usize,
    pub height: usize,
    pub bpp: u16,
    pub pixels: Vec<u32>, // Pixeles en formato ARGB 0x00RRGGBB
}

impl BmpImage {
    /// Parsea un array de bytes en formato Windows Bitmap (.bmp) estándar
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        // Un BMP mínimo requiere BITMAPFILEHEADER (14 bytes) + BITMAPINFOHEADER (40 bytes) = 54 bytes
        if bytes.len() < 54 {
            return Err("Archivo BMP demasiado pequeño (menor a 54 bytes)");
        }

        // 1. Verificar firma mágica 'BM' (0x42, 0x4D)
        if bytes[0] != 0x42 || bytes[1] != 0x4D {
            return Err("Firma BMP inválida (no contiene el identificador 'BM')");
        }

        // 2. Extraer offset de datos de píxeles
        let data_offset = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
        if data_offset >= bytes.len() {
            return Err("Offset de píxeles fuera de los límites del archivo BMP");
        }

        // 3. Parsear BITMAPINFOHEADER
        let header_size = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]) as usize;
        if header_size < 40 {
            return Err("Cabecera DIB no soportada (se requiere BITMAPINFOHEADER de 40+ bytes)");
        }

        let width_raw = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
        let height_raw = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
        if width_raw <= 0 || height_raw == 0 {
            return Err("Dimensiones de imagen inválidas en cabecera BMP");
        }

        let width = width_raw as usize;
        let is_top_down = height_raw < 0;
        let height = height_raw.unsigned_abs() as usize;

        let planes = u16::from_le_bytes([bytes[26], bytes[27]]);
        if planes != 1 {
            return Err("Planes de color inválidos (debe ser 1)");
        }

        let bpp = u16::from_le_bytes([bytes[28], bytes[29]]);
        if bpp != 24 && bpp != 32 {
            return Err("Profundidad de color no soportada (solo 24-bit RGB y 32-bit RGBA)");
        }

        let compression = u32::from_le_bytes([bytes[30], bytes[31], bytes[32], bytes[33]]);
        if compression != 0 {
            return Err("Compresión no soportada (solo BI_RGB sin comprimir)");
        }

        // 4. Decodificar filas de píxeles respetando el padding de 4 bytes
        let bytes_per_pixel = (bpp / 8) as usize;
        let row_stride = ((width * bytes_per_pixel + 3) / 4) * 4;
        let mut pixels = alloc::vec![0u32; width * height];

        for row in 0..height {
            let src_row = if is_top_down {
                row
            } else {
                height - 1 - row // Bottom-up (estándar Windows)
            };

            let row_start = data_offset + src_row * row_stride;
            if row_start + width * bytes_per_pixel > bytes.len() {
                break;
            }

            for col in 0..width {
                let px_idx = row_start + col * bytes_per_pixel;
                let b = bytes[px_idx] as u32;
                let g = bytes[px_idx + 1] as u32;
                let r = bytes[px_idx + 2] as u32;
                let color = (r << 16) | (g << 8) | b;
                pixels[row * width + col] = color;
            }
        }

        Ok(Self {
            width,
            height,
            bpp,
            pixels,
        })
    }

    /// Renderiza la imagen directamente sobre el Framebuffer gráfico en las coordenadas especificadas
    pub fn render_at(&self, start_x: usize, start_y: usize) {
        for row in 0..self.height {
            let y = start_y + row;
            for col in 0..self.width {
                let x = start_x + col;
                let color = self.pixels[row * self.width + col];
                Canvas::put_pixel(x, y, color);
            }
        }
    }
}

/// Genera un archivo binario BMP de 24 bits auténtico con el logotipo de Beaver OS (32x32)
pub fn create_beaver_logo_bmp() -> Vec<u8> {
    let width: usize = 32;
    let height: usize = 32;
    let bytes_per_pixel = 3; // 24 bits (BGR)
    let row_stride = ((width * bytes_per_pixel + 3) / 4) * 4; // 96 bytes por fila (exacto)
    let image_data_size = row_stride * height;
    let file_size = 54 + image_data_size;

    let mut bmp = Vec::with_capacity(file_size);

    // 1. BITMAPFILEHEADER (14 bytes)
    bmp.push(0x42); // 'B'
    bmp.push(0x4D); // 'M'
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&0_u16.to_le_bytes()); // Reserved 1
    bmp.extend_from_slice(&0_u16.to_le_bytes()); // Reserved 2
    bmp.extend_from_slice(&54_u32.to_le_bytes()); // Offset de píxeles

    // 2. BITMAPINFOHEADER (40 bytes)
    bmp.extend_from_slice(&40_u32.to_le_bytes()); // biSize
    bmp.extend_from_slice(&(width as i32).to_le_bytes()); // biWidth
    bmp.extend_from_slice(&(height as i32).to_le_bytes()); // biHeight (positivo = bottom-up)
    bmp.extend_from_slice(&1_u16.to_le_bytes()); // biPlanes
    bmp.extend_from_slice(&24_u16.to_le_bytes()); // biBitCount
    bmp.extend_from_slice(&0_u32.to_le_bytes()); // biCompression (BI_RGB)
    bmp.extend_from_slice(&(image_data_size as u32).to_le_bytes()); // biSizeImage
    bmp.extend_from_slice(&2835_i32.to_le_bytes()); // biXPelsPerMeter (72 DPI)
    bmp.extend_from_slice(&2835_i32.to_le_bytes()); // biYPelsPerMeter
    bmp.extend_from_slice(&0_u32.to_le_bytes()); // biClrUsed
    bmp.extend_from_slice(&0_u32.to_le_bytes()); // biClrImportant

    // 3. Píxeles de la imagen (de abajo hacia arriba)
    let padding_bytes = row_stride - (width * bytes_per_pixel);
    for y in 0..height {
        let visual_y = height - 1 - y;

        for x in 0..width {
            let factor = (visual_y * 255) / height;
            let mut r = ((10 * (255 - factor)) + (6 * factor)) / 255;
            let mut g = ((25 * (255 - factor)) + (182 * factor)) / 255;
            let mut b = ((47 * (255 - factor)) + (212 * factor)) / 255;

            // Dibujar silueta estilizada 'B' de Beaver en blanco brillante
            let in_logo = (x >= 9 && x <= 13 && visual_y >= 7 && visual_y <= 25)
                || (x >= 13 && x <= 22 && (visual_y == 7 || visual_y == 16 || visual_y == 25))
                || (x >= 21 && x <= 24 && ((visual_y >= 7 && visual_y <= 16) || (visual_y >= 16 && visual_y <= 25)));

            if in_logo {
                r = 255;
                g = 255;
                b = 255;
            }

            bmp.push(b as u8);
            bmp.push(g as u8);
            bmp.push(r as u8);
        }

        for _ in 0..padding_bytes {
            bmp.push(0);
        }
    }

    bmp
}

/// Inicializa las imágenes predeterminadas en el disco ATA (/disk/logo.bmp)
pub fn init_disk_images() {
    let mut vfs = VFS.lock();
    if !vfs.exists("/disk/logo.bmp") {
        let logo_data = create_beaver_logo_bmp();
        let _ = vfs.create_file("/disk/logo.bmp", &logo_data, 0, 0);
    }
}
