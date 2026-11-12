// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Flattened Device Tree (FDT / DTB) Binary Parser & Mobile
//              SimpleFB (simple-framebuffer) Driver for ARM64 Devices
// ============================================================================

#![allow(dead_code)]

use crate::sync::SpinMutex;

/// Firma mágica estándar del Flattened Device Tree (0xD00DFEED en Big-Endian)
pub const FDT_MAGIC: u32 = 0xD00D_FEED;

/// Tokens estándar de la estructura FDT v17
const FDT_BEGIN_NODE: u32 = 0x0000_0001;
const FDT_END_NODE:   u32 = 0x0000_0002;
const FDT_PROP:       u32 = 0x0000_0003;
const FDT_NOP:        u32 = 0x0000_0004;
const FDT_END:        u32 = 0x0000_0009;

/// Formato de píxel reportado por el bootloader móvil en el nodo `simple-framebuffer`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePixelFormat {
    A8R8G8B8, // 32 bpp (BGRA en Little-Endian)
    R5G6B5,   // 16 bpp (RGB565 común en S-Boot de Samsung antiguos)
    Unknown,
}

/// Metadatos reales de un Framebuffer pre-inicializado por el bootloader (ABL / LK / S-Boot / U-Boot)
#[derive(Debug, Clone, Copy)]
pub struct SimpleFbInfo {
    pub present: bool,
    pub phys_base: u64,
    pub size_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: MobilePixelFormat,
}

impl SimpleFbInfo {
    pub const fn empty() -> Self {
        Self {
            present: false,
            phys_base: 0,
            size_bytes: 0,
            width: 0,
            height: 0,
            stride: 0,
            format: MobilePixelFormat::Unknown,
        }
    }
}

/// Información extraída del árbol de dispositivos físico (DTB)
pub struct DtbState {
    pub valid: bool,
    pub dtb_phys_addr: u64,
    pub total_size: u32,
    pub version: u32,
    pub model_buf: [u8; 64],
    pub model_len: usize,
    pub ram_base: u64,
    pub ram_size: u64,
    pub fb: SimpleFbInfo,
}

impl DtbState {
    pub const fn new() -> Self {
        Self {
            valid: false,
            dtb_phys_addr: 0,
            total_size: 0,
            version: 0,
            model_buf: [0u8; 64],
            model_len: 0,
            ram_base: 0,
            ram_size: 0,
            fb: SimpleFbInfo::empty(),
        }
    }

    pub fn model_str(&self) -> &str {
        if self.model_len == 0 {
            "Desconocido / Sin propiedad model"
        } else {
            core::str::from_utf8(&self.model_buf[..self.model_len]).unwrap_or("UTF-8 invalido")
        }
    }
}

pub static DTB_STATE: SpinMutex<DtbState> = SpinMutex::new(DtbState::new());

/// Lee un entero `u32` en formato Big-Endian desde una dirección alineada a 4 bytes.
#[inline]
unsafe fn read_be32(ptr: *const u8) -> u32 {
    let b0 = core::ptr::read_volatile(ptr) as u32;
    let b1 = core::ptr::read_volatile(ptr.add(1)) as u32;
    let b2 = core::ptr::read_volatile(ptr.add(2)) as u32;
    let b3 = core::ptr::read_volatile(ptr.add(3)) as u32;
    (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
}

/// Lee un entero `u64` en formato Big-Endian desde una dirección.
#[inline]
unsafe fn read_be64(ptr: *const u8) -> u64 {
    let hi = read_be32(ptr) as u64;
    let lo = read_be32(ptr.add(4)) as u64;
    (hi << 32) | lo
}

#[inline]
fn align4(offset: usize) -> usize {
    (offset + 3) & !3
}

/// Compara una cadena C terminada en nulo dentro del bloque de strings del DTB con un `&str`.
unsafe fn cstr_eq(mut c_ptr: *const u8, max_end: *const u8, target: &str) -> bool {
    for &b in target.as_bytes() {
        if c_ptr >= max_end {
            return false;
        }
        let ch = core::ptr::read_volatile(c_ptr);
        if ch != b {
            return false;
        }
        c_ptr = c_ptr.add(1);
    }
    c_ptr < max_end && core::ptr::read_volatile(c_ptr) == 0
}

/// Verifica si un buffer de propiedad (que puede contener múltiples strings separados por `\0`)
/// contiene una subcadena exacta (ej. `"simple-framebuffer"`).
fn prop_contains_str(prop_bytes: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || prop_bytes.len() < needle.len() {
        return false;
    }
    let mut i = 0;
    while i + needle.len() <= prop_bytes.len() {
        if &prop_bytes[i..i + needle.len()] == needle {
            return true;
        }
        i += 1;
    }
    false
}

/// Analiza el Flattened Device Tree (DTB) entregado en `x0` por el bootloader (o en `0x4000_0000` en QEMU virt).
/// Si detecta un nodo `simple-framebuffer` real dejado por el bootloader del teléfono (S-Boot / Fastboot / U-Boot),
/// inicializa el framebuffer y dibuja la cabecera visual de Beaver OS.
pub fn init_and_parse_dtb(x0_dtb_ptr: u64) -> Option<SimpleFbInfo> {
    // Candidatos: el puntero recibido en x0, o 0x4000_0000 (ubicación por defecto del DTB en QEMU virt)
    let candidates = [x0_dtb_ptr, 0x4000_0000];
    let mut base_addr = 0u64;

    for &cand in &candidates {
        if cand == 0 {
            continue;
        }
        // SAFETY: Se realiza lectura volátil de los primeros 4 bytes en la dirección candidata de RAM
        // para comprobar la firma mágica Big-Endian `0xD00DFEED` del estándar Devicetree.
        let magic = unsafe { read_be32(cand as *const u8) };
        if magic == FDT_MAGIC {
            base_addr = cand;
            break;
        }
    }

    if base_addr == 0 {
        return None;
    }

    let base_ptr = base_addr as *const u8;
    // SAFETY: La firma `FDT_MAGIC` ya fue verificada en `base_ptr`. Se leen los campos de cabecera FDT (40 bytes).
    let (total_size, off_struct, off_strings, version) = unsafe {
        let total = read_be32(base_ptr.add(4));
        let st = read_be32(base_ptr.add(8));
        let sr = read_be32(base_ptr.add(12));
        let ver = read_be32(base_ptr.add(20));
        (total, st as usize, sr as usize, ver)
    };

    // Límite defensivo contra DTBs corruptos (máximo 8 MiB)
    if total_size < 40 || total_size > 8 * 1024 * 1024 {
        return None;
    }

    let max_end = unsafe { base_ptr.add(total_size as usize) };
    let strings_base = unsafe { base_ptr.add(off_strings) };

    let mut state = DtbState::new();
    state.valid = true;
    state.dtb_phys_addr = base_addr;
    state.total_size = total_size;
    state.version = version;

    let mut cursor = off_struct;
    let mut depth: usize = 0;
    let mut in_memory_node = false;
    let mut in_fb_node = false;

    let mut cur_fb = SimpleFbInfo::empty();

    // Recorrer secuencia de tokens FDT
    while cursor + 4 <= total_size as usize {
        // SAFETY: `cursor + 4 <= total_size` garantiza que la lectura permanece dentro del bloque DTB.
        let token = unsafe { read_be32(base_ptr.add(cursor)) };
        cursor += 4;

        match token {
            FDT_BEGIN_NODE => {
                // Leer nombre del nodo terminado en '\0'
                let name_start = cursor;
                while cursor < total_size as usize {
                    // SAFETY: `cursor < total_size` verificado en la condición del bucle.
                    let b = unsafe { core::ptr::read_volatile(base_ptr.add(cursor)) };
                    cursor += 1;
                    if b == 0 {
                        break;
                    }
                }
                let name_len = cursor.saturating_sub(name_start + 1);
                // SAFETY: El rango `[name_start .. name_start + name_len]` pertenece al bloque estructura validado.
                let node_name = unsafe {
                    core::slice::from_raw_parts(base_ptr.add(name_start), name_len)
                };
                cursor = align4(cursor);
                depth += 1;

                if prop_contains_str(node_name, b"memory") {
                    in_memory_node = true;
                } else if prop_contains_str(node_name, b"framebuffer") || prop_contains_str(node_name, b"simplefb") {
                    in_fb_node = true;
                    cur_fb = SimpleFbInfo::empty();
                }
            }
            FDT_END_NODE => {
                if in_fb_node && cur_fb.phys_base != 0 && cur_fb.width > 0 && cur_fb.height > 0 {
                    cur_fb.present = true;
                    state.fb = cur_fb;
                }
                in_memory_node = false;
                in_fb_node = false;
                depth = depth.saturating_sub(1);
            }
            FDT_PROP => {
                if cursor + 8 > total_size as usize {
                    break;
                }
                // SAFETY: `cursor + 8 <= total_size` asegura lectura válida de `len` y `nameoff`.
                let (prop_len, name_off) = unsafe {
                    let l = read_be32(base_ptr.add(cursor)) as usize;
                    let n = read_be32(base_ptr.add(cursor + 4)) as usize;
                    (l, n)
                };
                cursor += 8;
                if cursor + prop_len > total_size as usize {
                    break;
                }

                let prop_ptr = unsafe { base_ptr.add(cursor) };
                // SAFETY: `cursor + prop_len <= total_size` verificado arriba.
                let prop_slice = unsafe { core::slice::from_raw_parts(prop_ptr, prop_len) };
                let name_ptr = unsafe { strings_base.add(name_off) };

                // 1. Propiedad `model` o `compatible` en el nodo raíz (depth == 1)
                // SAFETY: `name_ptr` se valida contra `max_end` dentro de `cstr_eq`.
                unsafe {
                    if depth == 1 && state.model_len == 0 && (cstr_eq(name_ptr, max_end, "model") || cstr_eq(name_ptr, max_end, "compatible")) {
                        let copy_len = prop_slice.iter().position(|&c| c == 0).unwrap_or(prop_len).min(state.model_buf.len());
                        state.model_buf[..copy_len].copy_from_slice(&prop_slice[..copy_len]);
                        state.model_len = copy_len;
                    }

                    // 2. Detectar `compatible = "simple-framebuffer"` en cualquier nodo
                    if cstr_eq(name_ptr, max_end, "compatible") && prop_contains_str(prop_slice, b"simple-framebuffer") {
                        in_fb_node = true;
                    }

                    // 3. Extraer base y tamaño de RAM en nodo `/memory`
                    if in_memory_node && cstr_eq(name_ptr, max_end, "reg") {
                        if prop_len >= 16 {
                            state.ram_base = read_be64(prop_ptr);
                            state.ram_size = read_be64(prop_ptr.add(8));
                        } else if prop_len >= 8 {
                            state.ram_base = read_be32(prop_ptr) as u64;
                            state.ram_size = read_be32(prop_ptr.add(4)) as u64;
                        }
                    }

                    // 4. Extraer parámetros de `simple-framebuffer` (Samsung S-Boot / Fastboot ABL / U-Boot)
                    if in_fb_node {
                        if cstr_eq(name_ptr, max_end, "reg") {
                            if prop_len >= 16 {
                                cur_fb.phys_base = read_be64(prop_ptr);
                                cur_fb.size_bytes = read_be64(prop_ptr.add(8));
                            } else if prop_len >= 8 {
                                cur_fb.phys_base = read_be32(prop_ptr) as u64;
                                cur_fb.size_bytes = read_be32(prop_ptr.add(4)) as u64;
                            }
                        } else if cstr_eq(name_ptr, max_end, "width") && prop_len >= 4 {
                            cur_fb.width = read_be32(prop_ptr);
                        } else if cstr_eq(name_ptr, max_end, "height") && prop_len >= 4 {
                            cur_fb.height = read_be32(prop_ptr);
                        } else if cstr_eq(name_ptr, max_end, "stride") && prop_len >= 4 {
                            cur_fb.stride = read_be32(prop_ptr);
                        } else if cstr_eq(name_ptr, max_end, "format") {
                            if prop_contains_str(prop_slice, b"a8r8g8b8") || prop_contains_str(prop_slice, b"x8r8g8b8") {
                                cur_fb.format = MobilePixelFormat::A8R8G8B8;
                            } else if prop_contains_str(prop_slice, b"r5g6b5") {
                                cur_fb.format = MobilePixelFormat::R5G6B5;
                            }
                        }
                    }
                }

                cursor = align4(cursor + prop_len);
            }
            FDT_NOP => {}
            FDT_END => break,
            _ => break,
        }
    }

    let detected_fb = if state.fb.present { Some(state.fb) } else { None };
    *DTB_STATE.lock() = state;

    if let Some(ref fb) = detected_fb {
        render_simplefb_banner(fb);
    }

    detected_fb
}

/// Renderiza un banner visual en el Framebuffer lineal dejado por el bootloader móvil (`simple-framebuffer`).
pub fn render_simplefb_banner(fb: &SimpleFbInfo) {
    if !fb.present || fb.phys_base == 0 || fb.width == 0 || fb.height == 0 {
        return;
    }

    let bpp_bytes = match fb.format {
        MobilePixelFormat::R5G6B5 => 2u32,
        _ => 4u32,
    };
    let stride = if fb.stride >= fb.width * bpp_bytes {
        fb.stride
    } else {
        fb.width * bpp_bytes
    };

    // Dibujar fondo oscuro estilizado (Beaver OS Dark Slate) y franja superior dorada/azul
    for y in 0..fb.height {
        let row_addr = fb.phys_base + (y as u64) * (stride as u64);
        for x in 0..fb.width {
            let is_header = y < 64;
            let is_accent = (64..68).contains(&y);
            let (r, g, b): (u8, u8, u8) = if is_header {
                (0x14, 0x24, 0x3E) // Azul profundo Beaver OS
            } else if is_accent {
                (0xE5, 0xA0, 0x24) // Ámbar dorado
            } else {
                (0x0B, 0x10, 0x1B) // Fondo oscuro OLED-friendly
            };

            // SAFETY: `fb.phys_base` proviene del nodo `simple-framebuffer` del DTB entregado por el bootloader
            // con dimensiones `width` x `height` y `stride` verificados.
            unsafe {
                if bpp_bytes == 4 {
                    let pixel_ptr = (row_addr + (x as u64) * 4) as *mut u32;
                    let color = 0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                    core::ptr::write_volatile(pixel_ptr, color);
                } else {
                    let pixel_ptr = (row_addr + (x as u64) * 2) as *mut u16;
                    let r5 = ((r as u16) >> 3) & 0x1F;
                    let g6 = ((g as u16) >> 2) & 0x3F;
                    let b5 = ((b as u16) >> 3) & 0x1F;
                    let color = (r5 << 11) | (g6 << 5) | b5;
                    core::ptr::write_volatile(pixel_ptr, color);
                }
            }
        }
    }
}

