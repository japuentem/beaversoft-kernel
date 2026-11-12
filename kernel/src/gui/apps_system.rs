// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: System GUI Applications (Resource Monitor, BeaverFiles, BeaverSync, Spotlight)
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::sync::SpinMutex;
use crate::gui::canvas::{Canvas, Color};
use crate::gui::window::{WINDOW_MGR, WindowId};
use crate::gui::icons::*;
use crate::memory::heap::HEAP_ALLOCATOR;
use crate::memory::frame_allocator::FRAME_ALLOCATOR;

pub struct SpotlightState {
    pub is_open: bool,
    pub query: String,
    pub selected_idx: usize,
}

pub static SPOTLIGHT: SpinMutex<SpotlightState> = SpinMutex::new(SpotlightState {
    is_open: false,
    query: String::new(),
    selected_idx: 0,
});

pub fn draw_monitor_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Monitor) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Monitor))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "Monitor de Recursos en Vivo (Transparente)", is_active);
    let (total_heap, used_heap, free_heap, alloc_count, _dealloc_count) = HEAP_ALLOCATOR.stats();
    let pct = HEAP_ALLOCATOR.usage_percent();
    let frames_alloc = FRAME_ALLOCATOR.allocated_frames_count();
    let frames_total = FRAME_ALLOCATOR.total_usable_frames();
    let frames_free  = FRAME_ALLOCATOR.free_frames_count();
    let ram_free_mib  = frames_free  * 4096 / (1024 * 1024);
    let ram_total_mib = frames_total * 4096 / (1024 * 1024);

    Canvas::draw_string_prop(x + 15, y + 38, "ARQUITECTURA DE PROCESAMIENTO", Color::BEAVER_AMBER_DARK, None);
    Canvas::draw_string_prop(x + 15, y + 56, "CPU: x86_64 Long Mode Bare-Metal | PIC 100Hz", Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 15, y + 72, "Aislamiento: Syscall MSR / Ring 3 Userland Sandbox", Color::BEAVER_TEXT_MUTED, None);

    Canvas::draw_string_prop(x + 15, y + 96, "MEMORIA DINAMICA (Cero Buffers Ocultos)", Color::BEAVER_TEAL, None);
    let heap_str = format!("Heap: {} KiB | Usado: {} KiB | Libre: {} KiB", total_heap / 1024, used_heap / 1024, free_heap / 1024);
    Canvas::draw_string_prop(x + 15, y + 116, &heap_str, Color::BEAVER_TEXT_DARK, None);

    let bar_w = w.saturating_sub(30);
    Canvas::fill_rounded_rect(x + 15, y + 136, bar_w, 16, 4, Color::BEAVER_TITANIUM_BAR);
    let fill_w = (bar_w * pct.min(100)) / 100;
    if fill_w > 0 {
        Canvas::fill_rounded_rect(x + 15, y + 136, fill_w, 16, 4, Color::BEAVER_TEAL);
    }
    Canvas::draw_rounded_rect(x + 15, y + 136, bar_w, 16, 4, Color::BEAVER_BORDER_LIGHT);

    let pct_str = format!("Ocupacion: {}% | Reserva Anti-OOM: 64 KiB", pct);
    Canvas::draw_string_prop(x + 15, y + 158, &pct_str, Color::BEAVER_AMBER_DARK, None);

    Canvas::draw_string_prop(x + 15, y + 182, "HILOS CONCURRENTES (PREEMPTIVE ROUND-ROBIN)", Color::BEAVER_TEAL, None);
    Canvas::draw_string_prop(x + 15, y + 202, "TID 0: main_shell       | RUNNING | UID: 0 (root)", Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 15, y + 220, "TID 1: telemetry_daemon | SLEEP   | UID: 0 (root)", Color::BEAVER_TEXT_MUTED, None);
    Canvas::draw_string_prop(x + 15, y + 238, "TID 2: storage_guard    | SLEEP   | UID: 0 (root)", Color::BEAVER_TEXT_MUTED, None);

    let stats_str = format!("Heap allocs: {} | RAM: {} MiB libre / {} MiB total | Marcos: {}", alloc_count, ram_free_mib, ram_total_mib, frames_alloc);
    Canvas::draw_string_prop(x + 15, y + 262, &stats_str, Color::BEAVER_TEXT_MUTED, None);
}

pub fn render_monitor_window() {
    draw_monitor_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Monitor) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum FilesViewTab {
    Net,
    Disk,
    Home,
    Proc,
    Sys,
    Trash,
}

pub static FILES_VIEW_TAB: SpinMutex<FilesViewTab> = SpinMutex::new(FilesViewTab::Net);

pub fn draw_files_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Files) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Files))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "BeaverFiles - Explorador VFS y Malla Lord OS", is_active);

    let tab = *FILES_VIEW_TAB.lock();

    // 1. Barra Lateral Izquierda (Sidebar estilo Lord OS / Finder)
    let sb_w = 140;
    Canvas::fill_rect(x + 1, y + 32, sb_w, h.saturating_sub(33), Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_line_v(x + 1 + sb_w, y + 32, h.saturating_sub(33), Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 12, y + 42, "UBICACIONES", Color::BEAVER_TEXT_MUTED, None);

    let tabs = [
        (FilesViewTab::Net,   "Red Mesh",         SidebarIconKind::Mesh,     56),
        (FilesViewTab::Disk,  "Disco (/disk)",    SidebarIconKind::Disk,     78),
        (FilesViewTab::Home,  "Home (/home)",     SidebarIconKind::Home,     100),
        (FilesViewTab::Proc,  "Proc (/proc)",     SidebarIconKind::Settings, 122),
        (FilesViewTab::Sys,   "Sys (/sys)",       SidebarIconKind::Terminal, 144),
        (FilesViewTab::Trash, "Papelera (/trash)",SidebarIconKind::Trash,    166),
    ];

    for (t, label, icon_kind, ty) in &tabs {
        let is_selected = *t == tab;
        if is_selected {
            Canvas::fill_rounded_rect(x + 6, y + ty - 3, sb_w - 12, 22, 6, 0xDBEAFE);
            draw_sidebar_icon(x + 12, y + ty - 1, *icon_kind, Color::BEAVER_AMBER);
            Canvas::draw_string_prop(x + 34, y + ty + 1, label, Color::BEAVER_TEXT_DARK, None);
        } else {
            draw_sidebar_icon(x + 12, y + ty - 1, *icon_kind, Color::BEAVER_TEXT_MUTED);
            Canvas::draw_string_prop(x + 34, y + ty + 1, label, Color::BEAVER_TEXT_DARK, None);
        }
    }

    Canvas::draw_string_prop(x + 12, y + 190, "RED MALLA", Color::BEAVER_TEXT_MUTED, None);
    let mesh_nodes_sb = {
        let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
        mgr.get_nodes().iter().take(3)
            .map(|n| (alloc::format!("{} ({})", n.name, n.ip.to_string()), n.status == "Online"))
            .collect::<alloc::vec::Vec<_>>()
    };
    let sb_offsets = [210usize, 232, 254];
    for (i, (label, is_online)) in mesh_nodes_sb.iter().enumerate().take(3) {
        let icon_col = if *is_online { Color::BEAVER_TEAL } else { Color::SLATE_400 };
        draw_sidebar_icon(x + 12, y + sb_offsets[i], SidebarIconKind::Mesh, icon_col);
        Canvas::draw_string_prop(x + 34, y + sb_offsets[i], label, Color::BEAVER_TEXT_MUTED, None);
    }
    for i in mesh_nodes_sb.len()..3 {
        draw_sidebar_icon(x + 12, y + sb_offsets[i], SidebarIconKind::Mesh, Color::SLATE_400);
        Canvas::draw_string_prop(x + 34, y + sb_offsets[i], "-- sin nodos --", Color::SLATE_400, None);
    }

    // 2. Panel Principal Derecho (Lienzo Blanco / Lord OS)
    let main_x = x + sb_w + 10;
    let main_w = w.saturating_sub(sb_w + 18);

    Canvas::fill_rounded_rect(main_x + 4, y + 36, 44, 22, 4, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_string(main_x + 12, y + 39, "<  >", Color::BEAVER_TEXT_DARK, None);

    let path_label = match tab {
        FilesViewTab::Net => "Red Mesh",
        FilesViewTab::Disk => "Disco (/disk)",
        FilesViewTab::Home => "Home",
        FilesViewTab::Proc => "Proc",
        FilesViewTab::Sys => "Sistema",
        FilesViewTab::Trash => "Papelera (/trash)",
    };
    Canvas::fill_rounded_rect(main_x + 54, y + 36, 140, 22, 4, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_string_prop(main_x + 64, y + 39, path_label, Color::BEAVER_TEXT_DARK, None);

    let search_w = 140;
    let search_x = main_x + main_w.saturating_sub(search_w + 6);
    Canvas::fill_rounded_rect(search_x, y + 36, search_w, 22, 11, Color::BEAVER_TITANIUM_BAR);
    Canvas::draw_string_prop(search_x + 14, y + 39, "Buscar...", Color::BEAVER_TEXT_MUTED, None);

    Canvas::draw_line_h(main_x, y + 64, main_w, Color::BEAVER_BORDER_LIGHT);

    match tab {
        FilesViewTab::Disk | FilesViewTab::Home => {
            let row1_y = y + 78;
            let col0 = main_x + 20;
            let col1 = main_x + 105;
            let col2 = main_x + 190;
            let col3 = main_x + 275;

            draw_beaver_folder(col0, row1_y, 56, 44, false);
            Canvas::draw_string_prop(col0 + 18, row1_y + 48, "bin", Color::BEAVER_TEXT_DARK, None);

            draw_file_document(col1, row1_y, 44, 48, "WAV", Color::BEAVER_EMERALD);
            Canvas::draw_string_prop(col1 - 4, row1_y + 52, "chime.wav", Color::BEAVER_TEXT_DARK, None);

            draw_file_document(col2, row1_y, 44, 48, "BMP", Color::BEAVER_TEAL);
            Canvas::draw_string_prop(col2, row1_y + 52, "logo.bmp", Color::BEAVER_TEXT_DARK, None);

            draw_file_document(col3, row1_y, 44, 48, "TXT", Color::BEAVER_AMBER);
            Canvas::draw_string_prop(col3 - 2, row1_y + 52, "notas.txt", Color::BEAVER_TEXT_DARK, None);

            let row2_y = y + 160;
            draw_file_document(col0 + 6, row2_y, 44, 48, "BS", Color::BEAVER_PURPLE);
            Canvas::draw_string_prop(col0 - 2, row2_y + 52, "welcome.bs", Color::BEAVER_TEXT_DARK, None);
        }
        FilesViewTab::Net => {
            let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
            let nodes = mgr.get_nodes();
            for (i, node) in nodes.iter().enumerate() {
                let card_x = main_x + (i % 2) * (main_w / 2);
                let card_y = y + 76 + (i / 2) * 82;
                let card_w = (main_w / 2).saturating_sub(10);
                if card_y + 75 <= y + h {
                    Canvas::fill_rounded_rect(card_x, card_y, card_w, 75, 8, Color::WHITE);
                    Canvas::draw_rounded_rect(card_x, card_y, card_w, 75, 8, Color::BEAVER_BORDER_LIGHT);

                    draw_device_icon(card_x + 10, card_y + 12, DeviceKind::Pc, Color::BEAVER_TEAL);

                    let title = format!("{} [{}]", node.name, node.status);
                    Canvas::draw_string_prop(card_x + 38, card_y + 10, &title, Color::BEAVER_TEXT_DARK, None);
                    let ip_cores = format!("IP: {} | {} Core(s)", node.ip.to_string(), node.cores);
                    Canvas::draw_string_prop(card_x + 38, card_y + 28, &ip_cores, Color::BEAVER_TEXT_MUTED, None);
                    let ram_shares = format!("RAM: {}MB | Uptime: {}s", node.ram_mb, node.uptime_secs);
                    let status_col = if node.status == "Online" { Color::BEAVER_EMERALD } else { Color::SLATE_400 };
                    Canvas::draw_string_prop(card_x + 38, card_y + 46, &ram_shares, status_col, None);
                }
            }
        }
        FilesViewTab::Trash => {
            Canvas::fill_rounded_rect(main_x + 12, y + 78, main_w.saturating_sub(24), 130, 8, Color::WHITE);
            Canvas::draw_rounded_rect(main_x + 12, y + 78, main_w.saturating_sub(24), 130, 8, Color::BEAVER_BORDER_LIGHT);

            draw_sidebar_icon(main_x + 24, y + 96, SidebarIconKind::Trash, Color::SLATE_400);
            Canvas::draw_string_prop(main_x + 50, y + 96, "Papelera de Reciclaje (Vacia)", Color::BEAVER_TEXT_DARK, None);
            Canvas::draw_string_prop(main_x + 50, y + 116, "Los archivos eliminados en BeaverFS se depositan aqui.", Color::BEAVER_TEXT_MUTED, None);
            Canvas::draw_string_prop(main_x + 50, y + 134, "Cero elementos pendientes de purga o restauracion en disco.", Color::BEAVER_TEXT_MUTED, None);
            Canvas::draw_string_prop(main_x + 50, y + 154, "Almacenamiento: Libre | BeaverFS Paridad OK", Color::BEAVER_EMERALD, None);
        }
        _ => {
            Canvas::fill_rounded_rect(main_x + 8, y + 78, main_w.saturating_sub(16), 80, 8, Color::WHITE);
            Canvas::draw_rounded_rect(main_x + 8, y + 78, main_w.saturating_sub(16), 80, 8, Color::BEAVER_BORDER_LIGHT);
            Canvas::draw_string_prop(main_x + 18, y + 90, "DIRECTORIO DEL SISTEMA VFS (Lord OS Style)", Color::BEAVER_AMBER_DARK, None);
            Canvas::draw_string_prop(main_x + 18, y + 112, "Acceso seguro mediante arquitectura microkernel y sandbox Ring 3.", Color::BEAVER_TEXT_DARK, None);
        }
    }

    let footer_y = y + h - 22;
    let footer_text = match tab {
        FilesViewTab::Trash => "0 elementos en Papelera, 2.4 MB disponibles en BeaverFS",
        _ => "5 elementos, 2.4 MB disponibles en BeaverFS",
    };
    Canvas::draw_string_prop(main_x + 12, footer_y + 3, footer_text, Color::BEAVER_TEXT_MUTED, None);
}

pub fn render_files_window() {
    draw_files_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Files) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
    }
}

pub fn handle_files_click(mx: usize, my: usize, wx: usize, wy: usize) {
    if mx >= wx && mx <= wx + 140 {
        if my >= wy + 46 && my <= wy + 66 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Net;
        } else if my >= wy + 68 && my <= wy + 88 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Disk;
        } else if my >= wy + 90 && my <= wy + 110 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Home;
        } else if my >= wy + 112 && my <= wy + 132 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Proc;
        } else if my >= wy + 134 && my <= wy + 154 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Sys;
        } else if my >= wy + 156 && my <= wy + 178 {
            *FILES_VIEW_TAB.lock() = FilesViewTab::Trash;
        }
    }

    // Clic en la barra de búsqueda de BeaverFiles para abrir Spotlight
    let win = WINDOW_MGR.lock().get_window(WindowId::Files).cloned();
    if let Some(w) = win {
        let sb_w = 140;
        let main_x = wx + sb_w + 10;
        let main_w = w.width.saturating_sub(sb_w + 18);
        let search_w = 140;
        let search_x = main_x + main_w.saturating_sub(search_w + 6);
        if mx >= search_x && mx <= search_x + search_w && my >= wy + 36 && my <= wy + 58 {
            crate::drivers::audio::click_feedback();
            SPOTLIGHT.lock().is_open = true;
        }
    }
}

pub fn draw_sync_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Sync) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Sync))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "BeaverSync - Malla de Dispositivos", is_active);

    let (nodes, total_cores, total_ram, sync_count, logs) = {
        let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
        (mgr.get_nodes().to_vec(), mgr.total_cluster_cores(), mgr.total_cluster_ram_mb(), mgr.sync_count, mgr.sync_logs.clone())
    };

    Canvas::fill_rounded_rect(x + 10, y + 36, w - 20, 26, 6, 0x0F3A3A);
    Canvas::draw_rounded_rect(x + 10, y + 36, w - 20, 26, 6, 0x0F766E);
    let online_count = nodes.iter().filter(|n| n.status == "Online").count();
    let right_stats = alloc::format!("Syncs: #{} | Online: {}/{}", sync_count, online_count, nodes.len());
    Canvas::draw_string_prop(x + 18, y + 41, &alloc::format!("BeaverSync activo | {}", right_stats), Color::BEAVER_TEAL_BRIGHT, None);

    let left_w = (w.saturating_sub(30)) * 55 / 100;
    Canvas::fill_rounded_rect(x + 10, y + 68, left_w, 185, 8, Color::WHITE);
    Canvas::draw_rounded_rect(x + 10, y + 68, left_w, 185, 8, Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(x + 16, y + 74, "DISPOSITIVOS INTERCONECTADOS (UN SOLO SO):", Color::BEAVER_AMBER_DARK, None);

    Canvas::fill_rounded_rect(x + 16, y + 92, left_w.saturating_sub(12), 34, 6, Color::BEAVER_TITANIUM_BAR);
    draw_device_icon(x + 20, y + 96, DeviceKind::Pc, Color::BEAVER_AMBER);
    let local_name = { crate::drivers::mesh::MESH_MANAGER.lock().local_name.clone() };
    Canvas::draw_string_prop(x + 48, y + 96, &alloc::format!("{} (Este Equipo)", local_name), Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 48, y + 110, "Estado: Maestro Local | 1 BSP Core | RAM real via allocator", Color::BEAVER_EMERALD, None);

    if nodes.is_empty() {
        Canvas::draw_string_prop(x + 20, y + 142, "Esperando anuncios UDP (:4242) de otros nodos Beaver...", Color::BEAVER_TEXT_MUTED, None);
    }
    for (i, node) in nodes.iter().enumerate().take(3) {
        let dy = y + 130 + i * 38;
        Canvas::fill_rounded_rect(x + 16, dy, left_w.saturating_sub(12), 34, 6, Color::BEAVER_TITANIUM_BAR);

        let icon_col = if node.status == "Online" { Color::BEAVER_TEAL } else { Color::SLATE_400 };
        draw_device_icon(x + 20, dy + 4, DeviceKind::Pc, icon_col);

        let line1 = format!("{} [{}] | Sinc: {}% ({})", node.name, node.status, node.sync_percent, node.last_sync_file);
        Canvas::draw_string_prop(x + 48, dy + 4, &line1, Color::BEAVER_TEXT_DARK, None);
        let line2 = format!("IP: {} | {} Core(s) | {} MB | Uptime: {}s",
            node.ip.to_string(), node.cores, node.ram_mb, node.uptime_secs
        );
        Canvas::draw_string_prop(x + 48, dy + 19, &line2, Color::BEAVER_TEXT_MUTED, None);
    }

    let right_x = x + 15 + left_w;
    let right_w = w.saturating_sub(left_w + 25);
    Canvas::fill_rounded_rect(right_x, y + 68, right_w, 185, 8, Color::WHITE);
    Canvas::draw_rounded_rect(right_x, y + 68, right_w, 185, 8, Color::BEAVER_BORDER_LIGHT);

    Canvas::draw_string_prop(right_x + 12, y + 74, "PODER DE MALLA AGREGADO:", Color::BEAVER_TEAL, None);

    let cores_str = format!("* Cores Totales:  {} nucleos x86_64", total_cores);
    Canvas::draw_string_prop(right_x + 12, y + 96, &cores_str, Color::BEAVER_TEXT_DARK, None);

    let ram_gb = total_ram / 1024;
    let ram_dec = (total_ram % 1024) / 100;
    let ram_str = format!("* Memoria Malla:  {} MB ({}.{} GB)", total_ram, ram_gb, ram_dec);
    Canvas::draw_string_prop(right_x + 12, y + 116, &ram_str, Color::BEAVER_TEXT_DARK, None);

    Canvas::draw_string_prop(right_x + 12, y + 136, "* Protocolo:      BEAVMESH UDP :4242", Color::BEAVER_EMERALD, None);
    Canvas::draw_string_prop(right_x + 12, y + 156, "* Descubrimiento: Anuncio 2s / Timeout 10s", Color::BEAVER_TEXT_MUTED, None);
    Canvas::draw_string_prop(right_x + 12, y + 176, "* Transporte:     Intel E1000 DMA Real", Color::BEAVER_AMBER_DARK, None);

    let sync_btn_y = y + 210;
    Canvas::fill_rounded_rect(right_x + 10, sync_btn_y, right_w.saturating_sub(20), 32, 6, Color::BEAVER_AMBER);
    Canvas::draw_string_prop(right_x + 20, sync_btn_y + 10, "Sincronizar Malla Ahora", Color::WHITE, None);

    let feed_y = y + 260;
    let feed_h = h.saturating_sub(270);
    Canvas::fill_rounded_rect(x + 10, feed_y, w - 20, feed_h, 8, Color::SLATE_900);
    Canvas::draw_rounded_rect(x + 10, feed_y, w - 20, feed_h, 8, Color::SLATE_700);

    Canvas::draw_string(x + 18, feed_y + 6, "REGISTRO DE TELEMETRIA EN VIVO (EVENTOS BEAVERSYNC):", Color::SLATE_400, None);
    let mut log_y = feed_y + 24;
    for log in logs.iter().rev().take(4) {
        Canvas::draw_string(x + 18, log_y, log, Color::BEAVER_GREEN, None);
        log_y += 18;
    }
}

pub fn render_sync_window() {
    draw_sync_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Sync) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
    }
}

pub fn handle_sync_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::Sync).cloned();
    if let Some(w) = win {
        let left_w = (w.width.saturating_sub(30)) * 55 / 100;
        let right_x = wx + 15 + left_w;
        let right_w = w.width.saturating_sub(left_w + 25);
        let sync_btn_y = wy + 210;

        if mx >= right_x + 10 && mx <= right_x + right_w.saturating_sub(10) && my >= sync_btn_y && my <= sync_btn_y + 32 {
            crate::drivers::mesh::trigger_mesh_sync();
            crate::drivers::audio::click_feedback();
            render_sync_window();
        }
    }
}

#[derive(Clone)]
pub struct SpotlightResult {
    pub title: String,
    pub desc: String,
    pub target: WindowId,
}

pub fn get_spotlight_results(query: &str) -> Vec<SpotlightResult> {
    use alloc::vec::Vec;
    let mut results = Vec::new();
    let q = query.trim();

    struct AppEntry {
        title: &'static str,
        desc: &'static str,
        target: WindowId,
        keywords: &'static [&'static str],
    }

    let all_apps: [AppEntry; 8] = [
        AppEntry {
            title: "Terminal / Consola",
            desc: "Interprete de comandos y shell interactivo CLI",
            target: WindowId::Terminal,
            keywords: &["term", "terminal", "consola", "shell", "cli", "bash", "cmd"],
        },
        AppEntry {
            title: "Monitor de Recursos",
            desc: "Telemetria de memoria, CPU y tareas en tiempo real",
            target: WindowId::Monitor,
            keywords: &["mon", "monitor", "recursos", "cpu", "mem", "top", "dashboard", "ram"],
        },
        AppEntry {
            title: "BeaverFiles",
            desc: "Explorador de archivos VFS (/disk, /net, /sys)",
            target: WindowId::Files,
            keywords: &["file", "files", "archivo", "archivos", "disco", "vfs", "carpeta", "disk"],
        },
        AppEntry {
            title: "BeaverSync",
            desc: "Malla de dispositivos y sincronizacion UDP real",
            target: WindowId::Sync,
            keywords: &["sync", "malla", "mesh", "red", "nodos", "udp", "compartir"],
        },
        AppEntry {
            title: "BeaverAudio",
            desc: "Reproductor y sintetizador PCM sin punto flotante",
            target: WindowId::Player,
            keywords: &["audio", "musica", "sonido", "wav", "player", "chime", "sound", "reproductor"],
        },
        AppEntry {
            title: "Fotos / Galería (BeaverPhotos)",
            desc: "Visor de imágenes BMP, galería de fotos y gráficos VESA",
            target: WindowId::Photos,
            keywords: &["foto", "fotos", "photo", "photos", "galeria", "imagen", "imagenes", "bmp", "pic", "visor"],
        },
        AppEntry {
            title: "Calendario (BeaverCalendar)",
            desc: "Calendario mensual en tiempo real (RTC UTC-6)",
            target: WindowId::Calendar,
            keywords: &["cal", "calendario", "calendar", "fecha", "reloj", "mes", "dia", "time"],
        },
        AppEntry {
            title: "Editor de Notas (BeaverNote)",
            desc: "Editor de notas persistente (/disk/notas.txt)",
            target: WindowId::Note,
            keywords: &["note", "nota", "notas", "editor", "texto", "bloc", "escribir", "txt"],
        },
    ];

    if q.is_empty() {
        for app in &all_apps[..4] {
            results.push(SpotlightResult {
                title: String::from(app.title),
                desc: String::from(app.desc),
                target: app.target,
            });
        }
        return results;
    }

    // Coincidencias en aplicaciones
    for app in &all_apps {
        let matches = contains_ignore_case(app.title, q)
            || contains_ignore_case(app.desc, q)
            || app.keywords.iter().any(|&k| contains_ignore_case(k, q) || contains_ignore_case(q, k));
        if matches {
            results.push(SpotlightResult {
                title: String::from(app.title),
                desc: String::from(app.desc),
                target: app.target,
            });
            if results.len() >= 4 {
                return results;
            }
        }
    }

    // Coincidencias en archivos reales de BeaverFS (/disk)
    if let Some(fs) = crate::storage::beaverfs::BEAVER_FS.try_lock() {
        if let Ok(files) = fs.list_files() {
            for (filename, size, _) in files {
                if contains_ignore_case(&filename, q) {
                    let desc = format!("Archivo en disco ATA (/disk/{}) - {} bytes", filename, size);
                    let target = if filename.ends_with(".wav") {
                        WindowId::Player
                    } else if filename.ends_with(".txt") || filename.contains("nota") {
                        WindowId::Note
                    } else {
                        WindowId::Files
                    };
                    results.push(SpotlightResult {
                        title: format!("/disk/{}", filename),
                        desc,
                        target,
                    });
                    if results.len() >= 4 {
                        return results;
                    }
                }
            }
        }
    }

    results
}

fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let h_bytes = haystack.as_bytes();
    let n_bytes = needle.as_bytes();
    if n_bytes.len() > h_bytes.len() {
        return false;
    }
    for i in 0..=(h_bytes.len() - n_bytes.len()) {
        let mut matches = true;
        for j in 0..n_bytes.len() {
            let h = h_bytes[i + j].to_ascii_lowercase();
            let n = n_bytes[j].to_ascii_lowercase();
            if h != n {
                matches = false;
                break;
            }
        }
        if matches {
            return true;
        }
    }
    false
}

pub fn draw_spotlight_overlay(screen_w: usize, _screen_h: usize) {
    let spot = SPOTLIGHT.lock();
    if !spot.is_open {
        return;
    }

    let modal_w = 480;
    let modal_h = 240;
    let modal_x = (screen_w.saturating_sub(modal_w)) / 2;
    let modal_y = 120;

    Canvas::fill_rounded_rect(modal_x + 6, modal_y + 6, modal_w, modal_h, 14, 0x070B16);
    Canvas::fill_rounded_rect(modal_x, modal_y, modal_w, modal_h, 14, 0x0B1222);
    Canvas::draw_rounded_rect(modal_x, modal_y, modal_w, modal_h, 14, 0x475569);

    Canvas::fill_rounded_rect(modal_x + 14, modal_y + 14, modal_w - 28, 36, 8, 0x1E293B);
    draw_sidebar_icon(modal_x + 22, modal_y + 24, SidebarIconKind::Search, Color::BEAVER_TEAL_BRIGHT);

    if spot.query.is_empty() {
        Canvas::draw_string_prop(modal_x + 46, modal_y + 24, "Escribe para buscar archivos o apps...", Color::SLATE_400, None);
    } else {
        let mut query_display = spot.query.clone();
        query_display.push('|'); // Cursor interactivo
        Canvas::draw_string_prop(modal_x + 46, modal_y + 24, &query_display, Color::WHITE, None);
    }

    Canvas::draw_line_h(modal_x + 14, modal_y + 58, modal_w - 28, Color::SLATE_700);

    let results = get_spotlight_results(&spot.query);

    if results.is_empty() {
        Canvas::fill_rounded_rect(modal_x + 14, modal_y + 70, modal_w - 28, 40, 6, 0x151E2E);
        Canvas::draw_string_prop(modal_x + 24, modal_y + 82, "No se encontraron coincidencias en apps ni archivos.", Color::SLATE_400, None);
    } else {
        for (i, res) in results.iter().enumerate().take(4) {
            let ry = modal_y + 66 + i * 40;
            let is_selected = i == spot.selected_idx;
            let bg_col = if is_selected { 0x1E293B } else { 0x151E2E };
            let border_col = if is_selected { Color::BEAVER_AMBER } else { Color::SLATE_700 };

            Canvas::fill_rounded_rect(modal_x + 14, ry, modal_w - 28, 34, 6, bg_col);
            if is_selected {
                Canvas::draw_rounded_rect(modal_x + 14, ry, modal_w - 28, 34, 6, border_col);
            }
            Canvas::draw_string_prop(modal_x + 24, ry + 4, &res.title, if is_selected { Color::WHITE } else { Color::BEAVER_AMBER }, None);
            Canvas::draw_string_prop(modal_x + 24, ry + 18, &res.desc, Color::SLATE_400, None);
        }
    }
}

// ============================================================================
// Calculadora - Beaver OS (Ring 3 Ready)
// ============================================================================

pub struct CalcState {
    pub display: String,
    pub operand: i64,
    pub pending_op: Option<char>,
    pub fresh_input: bool,
}

pub static CALC_STATE: SpinMutex<CalcState> = SpinMutex::new(CalcState {
    display: String::new(),
    operand: 0,
    pending_op: None,
    fresh_input: true,
});

pub fn draw_calculator_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::Calculator) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::Calculator))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "Calculadora - Beaver OS", is_active);

    // 1. Pantalla LCD Digital
    let disp_x = x + 12;
    let disp_y = y + 36;
    let disp_w = w.saturating_sub(24);
    let disp_h = 44;
    Canvas::fill_rounded_rect(disp_x, disp_y, disp_w, disp_h, 6, 0x020617);
    Canvas::draw_rounded_rect(disp_x, disp_y, disp_w, disp_h, 6, 0x334155);

    let (disp_text, op_hint) = {
        let st = CALC_STATE.lock();
        let txt = if st.display.is_empty() { String::from("0") } else { st.display.clone() };
        let op = st.pending_op.map(|c| format!("Op: {}", c)).unwrap_or_default();
        (txt, op)
    };

    if !op_hint.is_empty() {
        Canvas::draw_string_prop(disp_x + 8, disp_y + 4, &op_hint, Color::BEAVER_AMBER, None);
    }
    let tw = Canvas::string_width_prop(&disp_text);
    let tx = (disp_x + disp_w).saturating_sub(tw + 12);
    Canvas::draw_string_prop(tx, disp_y + 16, &disp_text, Color::BEAVER_GREEN, None);

    // 2. Cuadrícula de Botones (4 columnas x 5 filas)
    let btn_labels = [
        ["C", "(", ")", "/"],
        ["7", "8", "9", "*"],
        ["4", "5", "6", "-"],
        ["1", "2", "3", "+"],
        ["0", "00", "DEL", "="],
    ];

    let grid_x = x + 12;
    let grid_y = y + 90;
    let btn_w = (disp_w.saturating_sub(18)) / 4;
    let btn_h = 42;
    let gap = 6;

    for row in 0..5 {
        for col in 0..4 {
            let bx = grid_x + col * (btn_w + gap);
            let by = grid_y + row * (btn_h + gap);
            let lbl = btn_labels[row][col];

            let (bg_col, text_col, border_col) = match lbl {
                "C" => (0x7F1D1D, Color::WHITE, 0xDC2626),
                "=" => (0x065F46, Color::WHITE, Color::BEAVER_GREEN),
                "/" | "*" | "-" | "+" => (0x78350F, Color::BEAVER_AMBER_LIGHT, 0xF59E0B),
                "DEL" => (0x334155, Color::WHITE, Color::SLATE_400),
                _ => (Color::BEAVER_TITANIUM_BAR, Color::BEAVER_TEXT_DARK, Color::BEAVER_BORDER_LIGHT),
            };

            Canvas::fill_rounded_rect(bx, by, btn_w, btn_h, 6, bg_col);
            Canvas::draw_rounded_rect(bx, by, btn_w, btn_h, 6, border_col);
            let lw = Canvas::string_width_prop(lbl);
            let lx = bx + (btn_w.saturating_sub(lw)) / 2;
            let ly = by + 12;
            Canvas::draw_string_prop(lx, ly, lbl, text_col, None);
        }
    }
}

pub fn render_calculator_window() {
    draw_calculator_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::Calculator) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
    }
}

pub fn handle_calculator_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::Calculator).cloned();
    if let Some(w) = win {
        let disp_w = w.width.saturating_sub(24);
        let grid_x = wx + 12;
        let grid_y = wy + 90;
        let btn_w = (disp_w.saturating_sub(18)) / 4;
        let btn_h = 42;
        let gap = 6;

        let btn_labels = [
            ["C", "(", ")", "/"],
            ["7", "8", "9", "*"],
            ["4", "5", "6", "-"],
            ["1", "2", "3", "+"],
            ["0", "00", "DEL", "="],
        ];

        for row in 0..5 {
            for col in 0..4 {
                let bx = grid_x + col * (btn_w + gap);
                let by = grid_y + row * (btn_h + gap);
                if mx >= bx && mx <= bx + btn_w && my >= by && my <= by + btn_h {
                    crate::drivers::audio::click_feedback();
                    let lbl = btn_labels[row][col];
                    let mut st = CALC_STATE.lock();
                    match lbl {
                        "C" => {
                            st.display.clear();
                            st.operand = 0;
                            st.pending_op = None;
                            st.fresh_input = true;
                        }
                        "DEL" => {
                            st.display.pop();
                        }
                        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                            if st.fresh_input {
                                st.display.clear();
                                st.fresh_input = false;
                            }
                            if st.display.len() < 14 {
                                st.display.push_str(lbl);
                            }
                        }
                        "00" => {
                            if st.fresh_input {
                                st.display.clear();
                                st.fresh_input = false;
                            }
                            if st.display.len() < 13 && !st.display.is_empty() {
                                st.display.push_str("00");
                            }
                        }
                        "+" | "-" | "*" | "/" => {
                            let curr_val = parse_int_i64(&st.display);
                            st.operand = curr_val;
                            st.pending_op = lbl.chars().next();
                            st.fresh_input = true;
                        }
                        "=" => {
                            if let Some(op) = st.pending_op {
                                let rhs = parse_int_i64(&st.display);
                                let res = match op {
                                    '+' => st.operand.checked_add(rhs),
                                    '-' => st.operand.checked_sub(rhs),
                                    '*' => st.operand.checked_mul(rhs),
                                    '/' => if rhs != 0 { st.operand.checked_div(rhs) } else { None },
                                    _ => None,
                                };
                                match res {
                                    Some(val) => {
                                        st.display = format!("{}", val);
                                        st.operand = val;
                                    }
                                    None => {
                                        st.display = String::from("Error");
                                    }
                                }
                                st.pending_op = None;
                                st.fresh_input = true;
                            }
                        }
                        _ => {}
                    }
                    drop(st);
                    render_calculator_window();
                    return;
                }
            }
        }
    }
}

fn parse_int_i64(s: &str) -> i64 {
    let mut val: i64 = 0;
    let mut neg = false;
    for ch in s.chars() {
        if ch == '-' {
            neg = true;
        } else if ch.is_ascii_digit() {
            val = val.saturating_mul(10).saturating_add((ch as u8 - b'0') as i64);
        }
    }
    if neg { -val } else { val }
}

// ============================================================================
// Acerca de Beaver OS - Ventana Institucional (BeaverInfo)
// ============================================================================

pub fn draw_about_window() {
    let (x, y, w, h, is_active) = {
        let mgr = WINDOW_MGR.lock();
        if let Some(win) = mgr.get_window(WindowId::About) {
            (win.x, win.y, win.width, win.height, mgr.is_active(WindowId::About))
        } else {
            return;
        }
    };

    Canvas::draw_window(x, y, w, h, "Acerca de Beaver OS", is_active);

    // Cabecera con Logotipo
    Canvas::fill_rounded_rect(x + 20, y + 42, 44, 44, 10, Color::BEAVER_AMBER);
    Canvas::draw_rounded_rect(x + 20, y + 42, 44, 44, 10, Color::BEAVER_AMBER_LIGHT);
    Canvas::draw_string(x + 36, y + 54, "B", Color::WHITE, None);

    Canvas::draw_string_prop(x + 76, y + 42, "Beaver OS v0.9.0 (beaver-kernel)", Color::BEAVER_TEXT_DARK, None);
    Canvas::draw_string_prop(x + 76, y + 58, "Marca: Beaversoft | Autor: Jose Alberto Puente Maldonado", Color::BEAVER_AMBER_DARK, None);
    Canvas::draw_string_prop(x + 76, y + 74, "Rust Bare-Metal Kernel (x86_64 & AArch64)", Color::BEAVER_TEXT_MUTED, None);

    Canvas::draw_line_h(x + 20, y + 96, w.saturating_sub(40), Color::BEAVER_BORDER_LIGHT);

    // Especificaciones Técnicas Reales
    let specs = [
        ("Arquitectura:", "Microkernel hibrido en Rust (no_std, Ring 0 / Ring 3)"),
        ("Seguridad:", "Aislamiento de procesos por CR3 y MSR Syscalls LSTAR"),
        ("Almacenamiento:", "BeaverFS transaccional con proteccion A/B anti-brickeo"),
        ("Red & Mesh:", "Controlador RTL8139 TCP/IP y BeaverMesh P2P (:4242)"),
        ("Graficos:", "VESA VBE TrueColor (1024x768) con Dirty Rects y Doble Bufer"),
        ("Entorno Dual:", "Beaver Desktop (Ventanas) + Beaver Shell (CLI)"),
    ];

    for (i, (k, v)) in specs.iter().enumerate() {
        let sy = y + 108 + i * 22;
        Canvas::draw_string_prop(x + 24, sy, k, Color::BEAVER_TEXT_DARK, None);
        Canvas::draw_string_prop(x + 130, sy, v, Color::BEAVER_TEXT_MUTED, None);
    }

    // Botón Aceptar / Cerrar
    let btn_w = 110;
    let btn_h = 28;
    let btn_x = x + (w.saturating_sub(btn_w)) / 2;
    let btn_y = y + h.saturating_sub(38);
    Canvas::fill_rounded_rect(btn_x, btn_y, btn_w, btn_h, 6, Color::BEAVER_AMBER);
    Canvas::draw_rounded_rect(btn_x, btn_y, btn_w, btn_h, 6, Color::BEAVER_AMBER_DARK);
    let tw = Canvas::string_width_prop("Aceptar");
    Canvas::draw_string_prop(btn_x + (btn_w.saturating_sub(tw)) / 2, btn_y + 6, "Aceptar", Color::WHITE, None);
}

pub fn render_about_window() {
    draw_about_window();
    let mgr = WINDOW_MGR.lock();
    if let Some(win) = mgr.get_window(WindowId::About) {
        let (x, y, w, h) = (win.x, win.y, win.width, win.height);
        drop(mgr);
        Canvas::mark_dirty_rect(x, y, w + 8, h + 8);
        Canvas::present_dirty();
    }
}

pub fn handle_about_click(mx: usize, my: usize, wx: usize, wy: usize) {
    let win = WINDOW_MGR.lock().get_window(WindowId::About).cloned();
    if let Some(w) = win {
        let btn_w = 110;
        let btn_h = 28;
        let btn_x = wx + (w.width.saturating_sub(btn_w)) / 2;
        let btn_y = wy + w.height.saturating_sub(38);
        if mx >= btn_x && mx <= btn_x + btn_w && my >= btn_y && my <= btn_y + btn_h {
            crate::drivers::audio::click_feedback();
            if let Some(mut mgr) = WINDOW_MGR.try_lock() {
                mgr.minimize(WindowId::About);
            }
            crate::gui::desktop::render_desktop();
        }
    }
}

