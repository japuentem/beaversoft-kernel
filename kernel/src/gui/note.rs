// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: BeaverNote Graphical Persistent Text Editor Application (/disk)
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use crate::gui::canvas::{Canvas, Color};
use crate::gui::window::{WindowId, WINDOW_MGR};
use crate::sync::SpinMutex;

pub struct BeaverNoteState {
    pub content: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub file_path: &'static str,
    pub status_msg: String,
    pub is_dirty: bool,
}

impl BeaverNoteState {
    pub fn new() -> Self {
        Self {
            content: vec![
                String::from("// BeaverNote v1.0 - Bloc de Notas Persistente"),
                String::from("// Guardado y sincronizacion directa con disco ATA (/disk)"),
                String::from(""),
                String::from("Hola, bienvenido al entorno de escritorio Beaver!"),
                String::from("Este archivo se almacena en BeaverFS sobre el sector LBA 256."),
            ],
            cursor_row: 3,
            cursor_col: 0,
            file_path: "/disk/notas.txt",
            status_msg: String::from("Listo. Presiona [S] para guardar en /disk."),
            is_dirty: false,
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.content.is_empty() {
            self.content.push(String::new());
        }
        if self.cursor_row >= self.content.len() {
            self.cursor_row = self.content.len() - 1;
        }

        let line = &mut self.content[self.cursor_row];
        let col = self.cursor_col.min(line.len());
        line.insert(col, ch);
        self.cursor_col = col + 1;
        self.is_dirty = true;
        self.status_msg = String::from("Modificado (sin guardar en ATA)...");
    }

    pub fn insert_newline(&mut self) {
        if self.content.is_empty() {
            self.content.push(String::new());
        }
        if self.cursor_row >= self.content.len() {
            self.cursor_row = self.content.len() - 1;
        }

        // Límite práctico de 12 líneas para la ventana visual
        if self.content.len() >= 12 {
            self.status_msg = String::from("Limite visual de lineas alcanzado en ventana.");
            return;
        }

        let current_line = &mut self.content[self.cursor_row];
        let col = self.cursor_col.min(current_line.len());
        let rest = current_line.split_off(col);

        self.content.insert(self.cursor_row + 1, rest);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.is_dirty = true;
        self.status_msg = String::from("Modificado (nueva linea)...");
    }

    pub fn delete_char(&mut self) {
        if self.content.is_empty() {
            return;
        }
        if self.cursor_row >= self.content.len() {
            self.cursor_row = self.content.len() - 1;
        }

        if self.cursor_col > 0 {
            let line = &mut self.content[self.cursor_row];
            let col = self.cursor_col.min(line.len());
            if col > 0 {
                line.remove(col - 1);
                self.cursor_col = col - 1;
                self.is_dirty = true;
                self.status_msg = String::from("Modificado (sin guardar)...");
            }
        } else if self.cursor_row > 0 {
            // Unir la línea actual con la anterior
            let current = self.content.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_line = &mut self.content[self.cursor_row];
            self.cursor_col = prev_line.len();
            prev_line.push_str(&current);
            self.is_dirty = true;
            self.status_msg = String::from("Modificado (linea unida)...");
        }
    }

    pub fn save_to_disk(&mut self) -> Result<(), &'static str> {
        let full_text = self.content.join("\n");
        let mut vfs = crate::fs::vfs::VFS.lock();
        match vfs.create_file(self.file_path, full_text.as_bytes(), 0, 0) {
            Ok(()) => {
                self.is_dirty = false;
                self.status_msg = format!("Guardado OK en /disk ({} bytes en ATA)", full_text.len());
                Ok(())
            }
            Err(e) => {
                self.status_msg = format!("Error al guardar: {}", e);
                Err(e)
            }
        }
    }

    pub fn load_from_disk(&mut self) -> Result<(), &'static str> {
        let vfs = crate::fs::vfs::VFS.lock();
        match vfs.read_file(self.file_path, 0, 0) {
            Ok(text) => {
                let lines: Vec<String> = text.lines().map(String::from).collect();
                if lines.is_empty() {
                    self.content = vec![String::new()];
                } else {
                    self.content = lines;
                }
                self.cursor_row = 0;
                self.cursor_col = 0;
                self.is_dirty = false;
                self.status_msg = format!("Cargado OK desde ATA ({} lineas)", self.content.len());
                Ok(())
            }
            Err(e) => {
                self.status_msg = format!("Error al cargar /disk: {}", e);
                Err(e)
            }
        }
    }

    pub fn clear(&mut self) {
        self.content = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.is_dirty = false;
        self.status_msg = String::from("Documento limpiado.");
    }
}

pub static BEAVER_NOTE: SpinMutex<BeaverNoteState> = SpinMutex::new(BeaverNoteState {
    content: Vec::new(),
    cursor_row: 0,
    cursor_col: 0,
    file_path: "/disk/notas.txt",
    status_msg: String::new(),
    is_dirty: false,
});

pub fn init_beaver_note() {
    *BEAVER_NOTE.lock() = BeaverNoteState::new();
}

pub fn draw_note_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Note) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Note))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "BeaverNote v1.0 - Bloc de Notas Persistente (/disk)", is_active);

    let note = BEAVER_NOTE.lock();

    // 1. Barra de Herramientas Superior (Botones interactivos redondeados estilo Lord OS)
    let btn_y = y + 34;

    // Botón Guardar
    let btn_save_x = x + 12;
    Canvas::fill_rounded_rect(btn_save_x, btn_y, 82, 22, 4, 0x0D9488);
    Canvas::draw_string(btn_save_x + 8, btn_y + 4, "Guardar", Color::WHITE, None);

    // Botón Cargar
    let btn_load_x = x + 98;
    Canvas::fill_rounded_rect(btn_load_x, btn_y, 80, 22, 4, Color::BEAVER_AMBER);
    Canvas::draw_string(btn_load_x + 12, btn_y + 4, "Cargar", Color::WHITE, None);

    // Botón Limpiar
    let btn_clear_x = x + 182;
    Canvas::fill_rounded_rect(btn_clear_x, btn_y, 80, 22, 4, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_rounded_rect(btn_clear_x, btn_y, 80, 22, 4, Color::BEAVER_BORDER_LIGHT);
    Canvas::draw_string(btn_clear_x + 10, btn_y + 4, "Limpiar", Color::BEAVER_TEXT_DARK, None);

    // Botón Cerrar
    let btn_close_x = x + 266;
    Canvas::fill_rounded_rect(btn_close_x, btn_y, 76, 22, 4, Color::BEAVER_RUBY);
    Canvas::draw_string(btn_close_x + 12, btn_y + 4, "Cerrar", Color::WHITE, None);

    // Indicador de Estado en cápsula dedicada derecha
    let status_pill_x = x + 348;
    let status_pill_w = w.saturating_sub(360);
    Canvas::fill_rounded_rect(status_pill_x, btn_y, status_pill_w, 22, 4, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_rounded_rect(status_pill_x, btn_y, status_pill_w, 22, 4, Color::BEAVER_BORDER_LIGHT);

    let status_color = if note.is_dirty { Color::BEAVER_AMBER_DARK } else { Color::BEAVER_TEAL };
    let max_status_chars = (status_pill_w.saturating_sub(12)) / 8;
    let disp_status = if note.status_msg.len() > max_status_chars {
        &note.status_msg[..max_status_chars]
    } else {
        &note.status_msg
    };
    Canvas::draw_string(status_pill_x + 8, btn_y + 4, disp_status, status_color, None);

    // 2. Área de Edición de Texto
    let edit_x = x + 12;
    let edit_y = y + 60;
    let edit_w = w.saturating_sub(24);
    let edit_h = h.saturating_sub(86);

    Canvas::fill_rounded_rect(edit_x, edit_y, edit_w, edit_h, 6, Color::SLATE_900);
    Canvas::draw_rounded_rect(edit_x, edit_y, edit_w, edit_h, 6, Color::SLATE_700);

    // Columna de números de línea
    let gutter_w = 34;
    Canvas::fill_rect(edit_x, edit_y, gutter_w, edit_h, Color::SLATE_800);
    Canvas::fill_rect(edit_x + gutter_w, edit_y, 1, edit_h, Color::SLATE_700);

    // Renderizar líneas de texto con resaltado de sintaxis en tiempo real
    let max_display_lines = (edit_h.saturating_sub(12)) / 18;
    for (i, line) in note.content.iter().enumerate().take(max_display_lines) {
        let line_y = edit_y + 8 + i * 18;

        // Número de línea (ej: 01, 02)
        let num_str = format!("{:>2}", i + 1);
        Canvas::draw_string(edit_x + 8, line_y, &num_str, Color::SLATE_500, None);

        // Contenido coloreado sintácticamente según tokens
        render_syntax_highlighted_line(edit_x + gutter_w + 8, line_y, line);

        // Cursor interactivo en la línea activa
        if is_active && i == note.cursor_row {
            let cursor_x = edit_x + gutter_w + 8 + note.cursor_col * 8;
            if cursor_x + 8 <= edit_x + edit_w {
                Canvas::fill_rect(cursor_x, line_y + 14, 8, 2, Color::BEAVER_ACCENT_CYAN);
            }
        }
    }

    // 3. Barra de Estado Inferior de la Ventana
    let footer_y = y + h - 22;
    let footer_text = format!(
        "Ruta: {} | Lineas: {} | Modificado: {} | Persistencia: BeaverFS ATA",
        note.file_path,
        note.content.len(),
        if note.is_dirty { "SI*" } else { "NO" }
    );
    Canvas::draw_string(x + 16, footer_y, &footer_text, Color::BEAVER_TEXT_MUTED, None);
}

pub fn render_note_window() {
    draw_note_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Note) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        // Presentación directa de región sucia (Dirty Rect Presentation solo para edición de texto)
        Canvas::present_rect(x, y, w, h);
    }
}

/// Tokenizador visual ultrarrápido para resaltado de sintaxis sin asignaciones dinámicas pesadas
fn render_syntax_highlighted_line(start_x: usize, y: usize, line: &str) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Ignorar espacios en blanco iniciales
        if bytes[i] == b' ' || bytes[i] == b'\t' {
            i += 1;
            continue;
        }

        // Comentarios directos: // hasta el final de la línea
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            let comment_str = &line[i..];
            Canvas::draw_string(start_x + i * 8, y, comment_str, Color::SLATE_400, None);
            break;
        }

        // Cadenas de texto literales entre comillas "..."
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'"' {
                i += 1;
            }
            if i < len && bytes[i] == b'"' {
                i += 1;
            }
            let s_str = &line[start..i];
            Canvas::draw_string(start_x + start * 8, y, s_str, Color::BEAVER_GREEN, None);
            continue;
        }

        // Números enteros y literales numéricos
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let num_str = &line[start..i];
            Canvas::draw_string(start_x + start * 8, y, num_str, Color::BEAVER_YELLOW, None);
            continue;
        }

        // Palabras clave, tipos e identificadores
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &line[start..i];
            let color = match word {
                "let" | "fn" | "loop" | "if" | "else" | "return" | "print" | "println" | "beep"
                | "call" | "exit" | "true" | "false" | "for" | "while" | "mut" => {
                    Color::BEAVER_ACCENT_CYAN
                }
                "file_write" | "file_read" | "file_append" | "Number" | "Str" | "Bool" | "Array" => {
                    Color::BEAVER_PURPLE
                }
                _ => Color::WHITE,
            };
            Canvas::draw_string(start_x + start * 8, y, word, color, None);
            continue;
        }

        // Símbolos, delimitadores y operadores aritméticos
        let start = i;
        i += 1;
        let sym_str = &line[start..i];
        let sym_color = match bytes[start] {
            b'=' | b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>' | b'!' | b'{' | b'}' | b'(' | b')' | b'[' | b']' => {
                Color::BEAVER_PURPLE
            }
            _ => Color::SLATE_200,
        };
        Canvas::draw_string(start_x + start * 8, y, sym_str, sym_color, None);
    }
}

pub fn handle_note_click(mx: usize, my: usize, win_x: usize, win_y: usize) -> bool {
    // Clic en el botón de cerrar de la barra de título (esquina superior derecha)
    if my >= win_y + 6 && my <= win_y + 24 && mx >= win_x + 620 - 30 && mx <= win_x + 620 - 6 {
        WINDOW_MGR.lock().minimize(WindowId::Note);
        return true;
    }

    let btn_y = win_y + 32;
    if my >= btn_y && my <= btn_y + 22 {
        // Clic en [S] Guardar
        if mx >= win_x + 12 && mx <= win_x + 94 {
            let _ = BEAVER_NOTE.lock().save_to_disk();
            return true;
        }
        // Clic en [L] Cargar
        if mx >= win_x + 98 && mx <= win_x + 178 {
            let _ = BEAVER_NOTE.lock().load_from_disk();
            return true;
        }
        // Clic en [C] Limpiar
        if mx >= win_x + 182 && mx <= win_x + 262 {
            BEAVER_NOTE.lock().clear();
            return true;
        }
        // Clic en [X] Cerrar
        if mx >= win_x + 266 && mx <= win_x + 342 {
            WINDOW_MGR.lock().minimize(WindowId::Note);
            return true;
        }
    }

    // Clic en el área de edición de texto para posicionar el cursor
    let edit_y = win_y + 58;
    if my >= edit_y + 8 && my <= win_y + 280 && mx >= win_x + 46 {
        let clicked_row = (my.saturating_sub(edit_y + 8)) / 18;
        let clicked_col = (mx.saturating_sub(win_x + 46)) / 8;
        let mut note = BEAVER_NOTE.lock();
        if clicked_row < note.content.len() {
            note.cursor_row = clicked_row;
            note.cursor_col = clicked_col.min(note.content[clicked_row].len());
            return true;
        }
    }

    false
}
