// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Automated Kernel Test Suite & QEMU isa-debug-exit (0xF4) Runner
// ============================================================================

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use crate::vga::VgaColor;
use crate::{kprint, kprintln, kprint_color};

/// Puerto I/O del dispositivo `isa-debug-exit` de QEMU (`-device isa-debug-exit,iobase=0xf4,iosize=0x04`)
pub const QEMU_DEBUG_EXIT_PORT: u16 = 0xF4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    /// QEMU termina con exit code = (0x10 << 1) | 1 = 33
    Success = 0x10,
    /// QEMU termina con exit code = (0x11 << 1) | 1 = 35
    Failed  = 0x11,
}

/// Cierra la máquina virtual QEMU enviando el código al puerto `isa-debug-exit` (0xF4).
pub fn exit_qemu(exit_code: QemuExitCode) {
    // SAFETY: El puerto 0xF4 corresponde exclusivamente al dispositivo virtual isa-debug-exit de QEMU.
    unsafe {
        crate::io::outl(QEMU_DEBUG_EXIT_PORT, exit_code as u32);
    }
}

pub trait Testable {
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        kprint!("  [TEST] {:<52} ... ", core::any::type_name::<T>());
        self();
        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");
    }
}

/// Runner para `cargo test` (`custom_test_frameworks`)
pub fn test_runner(tests: &[&dyn Testable]) {
    kprintln!("\n==================== BEAVER KERNEL AUTOMATED TEST SUITE ====================");
    kprintln!("Ejecutando {} pruebas automatizadas en QEMU (isa-debug-exit 0xF4)...", tests.len());
    for test in tests {
        test.run();
    }
    kprint_color!(
        VgaColor::LightGreen,
        VgaColor::Black,
        "[SUITE OK] Todas las {} pruebas del kernel pasaron exitosamente.\n",
        tests.len()
    );
    kprintln!("============================================================================");
    exit_qemu(QemuExitCode::Success);
}

/// Ejecuta la batería completa de pruebas automatizadas del kernel y sale por `0xF4` si `exit_on_complete` es `true`.
pub fn run_builtin_test_suite(exit_on_complete: bool) {
    let tests: [(&str, fn()); 8] = [
        ("spinmutex_owner_and_reentrancy_guard", test_spinmutex_owner_and_reentrancy_guard),
        ("frame_allocator_alloc_and_free",       test_frame_allocator_alloc_and_free),
        ("heap_dynamic_allocations",             test_heap_dynamic_allocations),
        ("vfs_beaverfs_read_write",              test_vfs_beaverfs_read_write),
        ("mesh_file_sync_and_ack",               test_mesh_file_sync_and_ack),
        ("audio_integer_chime_no_fpu",           test_audio_integer_chime_no_fpu),
        ("gui_dirty_rects_and_resolution",       test_gui_dirty_rects_and_resolution),
        ("crc32_ieee8023_integrity",             test_crc32_ieee8023),
    ];

    kprintln!("\n==================== BEAVER KERNEL AUTOMATED TEST SUITE ====================");
    kprintln!("Ejecutando {} pruebas automatizadas en QEMU (isa-debug-exit 0xF4)...", tests.len());
    for (name, test_fn) in &tests {
        kprint!("  [TEST] {:<52} ... ", name);
        test_fn();
        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");
    }
    kprint_color!(
        VgaColor::LightGreen,
        VgaColor::Black,
        "[SUITE OK] Todas las {} pruebas del kernel pasaron exitosamente.\n",
        tests.len()
    );
    kprintln!("============================================================================");

    if exit_on_complete {
        exit_qemu(QemuExitCode::Success);
    }
}

// ============================================================================
// Casos de Prueba Unitarios y de Integración del Kernel (Fase 6)
// ============================================================================

pub fn test_spinmutex_owner_and_reentrancy_guard() {
    use crate::sync::SpinMutex;

    let mtx = SpinMutex::new(42_u64);
    assert!(!mtx.is_locked_by_current_cpu(), "El candado debe iniciar libre");
    assert!(mtx.owner_location().is_none(), "No debe haber ubicacion de dueno al iniciar");

    {
        let mut guard = mtx.lock();
        assert_eq!(*guard, 42);
        assert!(mtx.is_locked_by_current_cpu(), "Debe detectar que el nucleo actual posee el candado");
        assert!(mtx.owner_location().is_some(), "Debe registrar file:line del dueno actual");
        assert!(mtx.try_lock().is_none(), "try_lock sobre candado ocupado debe devolver None");
        *guard = 99;
    }

    assert!(!mtx.is_locked_by_current_cpu(), "El candado debe liberarse al salir del ambito");
    assert_eq!(*mtx.lock(), 99);
}

pub fn test_frame_allocator_alloc_and_free() {
    use crate::memory::frame_allocator::{FRAME_ALLOCATOR, PAGE_SIZE};

    let free_before = FRAME_ALLOCATOR.free_frames_count();
    let frame = FRAME_ALLOCATOR.allocate_frame().expect("Debe asignar un marco de 4 KiB");
    assert_eq!(frame % PAGE_SIZE, 0, "El marco asignado debe estar alineado a 4 KiB");
    assert_eq!(FRAME_ALLOCATOR.free_frames_count(), free_before - 1);

    FRAME_ALLOCATOR.free_frame(frame);
    assert_eq!(FRAME_ALLOCATOR.free_frames_count(), free_before);

    let contig = FRAME_ALLOCATOR.allocate_contiguous(4).expect("Debe asignar 4 marcos contiguos");
    assert_eq!(contig % PAGE_SIZE, 0);
    assert_eq!(FRAME_ALLOCATOR.free_frames_count(), free_before - 4);
    for i in 0..4 {
        FRAME_ALLOCATOR.free_frame(contig + i * PAGE_SIZE);
    }
    assert_eq!(FRAME_ALLOCATOR.free_frames_count(), free_before);
}

pub fn test_heap_dynamic_allocations() {
    let b = Box::new(0xBEA0_2026_u64);
    assert_eq!(*b, 0xBEA0_2026_u64);

    let mut v = Vec::with_capacity(64);
    for i in 0..64_u32 {
        v.push(i * 3);
    }
    assert_eq!(v.len(), 64);
    assert_eq!(v[63], 189);

    let mut s = String::from("BeaverOS ");
    s.push_str("Fase 6 Tests");
    assert_eq!(s.as_str(), "BeaverOS Fase 6 Tests");
}

pub fn test_vfs_beaverfs_read_write() {
    let payload = b"AUTOTEST_VFS_BEAVERFS_OK";
    {
        let mut vfs = crate::fs::vfs::VFS.lock();
        vfs.create_file("/disk/autotest.txt", payload, 0, 0)
            .expect("Escritura en /disk/autotest.txt debe tener exito");
        let read_back = vfs.read_file("/disk/autotest.txt", 0, 0)
            .expect("Lectura de /disk/autotest.txt debe tener exito");
        assert_eq!(read_back.as_bytes(), payload);
    }
}

pub fn test_mesh_file_sync_and_ack() {
    use crate::drivers::mesh::{MeshManager, MSG_FILE_SYNC, MSG_FILE_ACK};
    use crate::drivers::net::Ipv4Addr;

    let mut sender = MeshManager::new();
    sender.local_name = String::from("beaver-a001");

    let mut receiver = MeshManager::new();
    receiver.local_name = String::from("beaver-a002");

    // 1. Anuncio inicial de A -> B y B -> A
    let ann_a = sender.prepare_announce();
    receiver.handle_packet(Ipv4Addr::new(10, 0, 0, 1), 4242, &ann_a);
    assert_eq!(receiver.online_count(), 1);

    let ann_b = receiver.prepare_announce();
    sender.handle_packet(Ipv4Addr::new(10, 0, 0, 2), 4242, &ann_b);
    assert_eq!(sender.online_count(), 1);

    // 2. A construye paquete MSG_FILE_SYNC con contenido de /disk/notas.txt
    let test_notes = b"Notas sincronizadas por UDP con ACK en Fase 6";
    let sync_pkt = sender.build_file_sync_packet(7, test_notes);
    assert_eq!(sync_pkt[8], MSG_FILE_SYNC);

    // 3. B recibe MSG_FILE_SYNC, encola escritura y prepara MSG_FILE_ACK
    let has_action = receiver.handle_packet(Ipv4Addr::new(10, 0, 0, 1), 4242, &sync_pkt);
    assert!(has_action, "Recepcion de MSG_FILE_SYNC debe activar accion diferida");
    assert_eq!(
        receiver.pending_received_notas.as_deref(),
        Some(&test_notes[..]),
        "El receptor debe almacenar los bytes exactos de /disk/notas.txt"
    );
    assert_eq!(receiver.pending_tx.len(), 1, "El receptor debe encolar paquete MSG_FILE_ACK");

    let (ack_dst, ack_pkt) = receiver.pending_tx.pop().unwrap();
    assert_eq!(ack_dst.0, [10, 0, 0, 1]);
    assert_eq!(ack_pkt[8], MSG_FILE_ACK);

    // 4. A recibe MSG_FILE_ACK y actualiza sync_percent = 100 y last_sync_file = "/disk/notas.txt"
    sender.handle_packet(Ipv4Addr::new(10, 0, 0, 2), 4242, &ack_pkt);
    let node_b = sender.get_node("beaver-a002").expect("beaver-a002 debe existir en sender");
    assert_eq!(node_b.sync_percent, 100);
    assert_eq!(node_b.last_sync_file, "/disk/notas.txt");
}

pub fn test_audio_integer_chime_no_fpu() {
    let wav = crate::drivers::audio::create_beaver_chime_wav();
    assert!(wav.len() > 44, "El archivo WAV debe contener cabecera RIFF + muestras PCM");
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(&wav[36..40], b"data");
    // Verificar que existan muestras no nulas sintetizadas con punto fijo entero
    let has_nonzero_pcm = wav[44..].iter().any(|&b| b != 0);
    assert!(has_nonzero_pcm, "La sintesis entera debe producir muestras PCM no nulas");
}

pub fn test_crc32_ieee8023() {
    assert_eq!(crate::storage::crc32::crc32(b"123456789"), 0xCBF43926);
}

pub fn test_gui_dirty_rects_and_resolution() {
    use crate::gui::canvas::Canvas;

    let (scr_w, scr_h) = Canvas::screen_dimensions();
    assert!(scr_w >= 640 && scr_h >= 480, "Dimensiones de pantalla activas validas");

    // Limpiar cualquier rectángulo sucio previo
    let _ = Canvas::take_dirty_rect();
    assert_eq!(Canvas::take_dirty_rect(), None);

    // Marcar dos regiones disjuntas y verificar la envolvente (bounding box)
    Canvas::mark_dirty_rect(100, 120, 50, 40);
    Canvas::mark_dirty_rect(200, 80, 100, 60);
    let dirty = Canvas::take_dirty_rect().expect("Debe existir region sucia acumulada");
    assert_eq!(dirty, (100, 80, 200, 80));
    assert_eq!(Canvas::take_dirty_rect(), None, "Debe limpiarse tras take_dirty_rect");
}

#[cfg(test)]
#[test_case]
fn ut_gui_dirty_rects() {
    test_gui_dirty_rects_and_resolution();
}

#[cfg(test)]
#[test_case]
fn ut_spinmutex() {
    test_spinmutex_owner_and_reentrancy_guard();
}

#[cfg(test)]
#[test_case]
fn ut_frame_allocator() {
    test_frame_allocator_alloc_and_free();
}

#[cfg(test)]
#[test_case]
fn ut_heap() {
    test_heap_dynamic_allocations();
}

#[cfg(test)]
#[test_case]
fn ut_vfs_beaverfs() {
    test_vfs_beaverfs_read_write();
}

#[cfg(test)]
#[test_case]
fn ut_mesh_sync_ack() {
    test_mesh_file_sync_and_ack();
}

#[cfg(test)]
#[test_case]
fn ut_audio_integer() {
    test_audio_integer_chime_no_fpu();
}
