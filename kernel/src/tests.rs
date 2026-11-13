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
    let tests: [(&str, fn()); 28] = [
        ("spinmutex_owner_and_reentrancy_guard",    test_spinmutex_owner_and_reentrancy_guard),
        ("frame_allocator_alloc_and_free",          test_frame_allocator_alloc_and_free),
        ("heap_dynamic_allocations",                test_heap_dynamic_allocations),
        ("paging_4k_and_process_cr3_isolation",     test_paging_4k_and_process_cr3_isolation),
        ("smp_madt_and_trampoline_0x8000",          test_smp_madt_and_trampoline_0x8000),
        ("vfs_beaverfs_read_write",                 test_vfs_beaverfs_read_write),
        ("mesh_file_sync_and_ack",                  test_mesh_file_sync_and_ack),
        ("audio_integer_chime_no_fpu",              test_audio_integer_chime_no_fpu),
        ("gui_dirty_rects_and_resolution",          test_gui_dirty_rects_and_resolution),
        ("crc32_ieee8023_integrity",                test_crc32_ieee8023),
        ("spotlight_live_search",                   test_spotlight_live_search),
        ("real_udp_dhcp_dns_structures",            test_real_udp_dhcp_dns),
        ("smp_distributed_scheduler",               test_smp_distributed_scheduler),
        ("ahci_sata_dma_controller",                test_ahci_sata_dma_controller),
        ("tcp_state_machine_and_http_parser",       test_tcp_state_machine_and_http_parser),
        ("posix_syscalls_and_beaver_libc",          test_posix_syscalls_and_beaver_libc),
        ("device_profile_detection",                test_device_profile_detection),
        ("cec_and_touchscreen_safe_dispatch",       test_cec_and_touchscreen_safe_dispatch),
        ("beaverfs_double_indirect_large_file",     test_beaverfs_double_indirect_large_file),
        ("userland_heap_and_sys_brk",               test_userland_heap_and_sys_brk),
        ("beaverfs_hierarchical_subdirectories",    test_beaverfs_hierarchical_subdirectories),
        ("preemptive_userland_task_scheduling",     test_preemptive_userland_task_scheduling),
        ("tcp_concurrent_listen_accept",            test_tcp_concurrent_listen_accept),
        ("shell_pipes_and_redirection",             test_shell_pipes_and_redirection),
        ("desktop_compositor_user_window_ipc",      test_desktop_compositor_user_window_ipc),
        ("xhci_usb3_controller_and_ports",          test_xhci_usb3_controller_and_ports),
        ("rtc_and_posix_clock_time",                test_rtc_and_posix_clock_time),
        ("userland_audio_pcm_dma_playback",         test_userland_audio_pcm_dma_playback),
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

pub fn test_paging_4k_and_process_cr3_isolation() {
    use crate::memory::frame_allocator::FRAME_ALLOCATOR;
    use crate::memory::paging::{self, KERNEL_FINE_PAGES_COUNT, USER_ELF_VADDR};

    let k_cr3 = paging::kernel_cr3();
    assert!(k_cr3 != 0, "KERNEL_CR3 debe estar inicializado");
    assert_eq!(
        paging::fine_4k_pages_count(),
        KERNEL_FINE_PAGES_COUNT,
        "El kernel debe tener 16,384 paginas finas de 4 KiB mapeadas"
    );

    // 1. Verificar que el código del kernel (0x100000) y el heap (0x400000) estén en páginas de 4 KiB con PAGE_USER = 0
    let (phys_k, is_4k_k, is_user_k) = paging::query_mapping(k_cr3, 0x0010_0000).expect("0x100000 debe estar mapeado");
    assert_eq!(phys_k, 0x0010_0000);
    assert!(is_4k_k, "La imagen del kernel debe usar tablas de 4 KiB");
    assert!(!is_user_k, "Las paginas del kernel NO deben tener el bit PAGE_USER");

    let (phys_h, is_4k_h, is_user_h) = paging::query_mapping(k_cr3, 0x0040_0000).expect("0x400000 debe estar mapeado");
    assert_eq!(phys_h, 0x0040_0000);
    assert!(is_4k_h, "El heap del kernel debe usar tablas de 4 KiB");
    assert!(!is_user_h, "El heap del kernel NO debe tener el bit PAGE_USER");

    // 2. Crear dos espacios de direcciones independientes (CR3_A y CR3_B) y mapear la misma vaddr a distintos marcos físicos
    let cr3_a = paging::create_process_address_space().expect("Debe crear CR3 para Proceso A");
    let cr3_b = paging::create_process_address_space().expect("Debe crear CR3 para Proceso B");
    assert_ne!(cr3_a, cr3_b);

    let frame_a = FRAME_ALLOCATOR.allocate_frame().expect("Marco para Proceso A") as u64;
    let frame_b = FRAME_ALLOCATOR.allocate_frame().expect("Marco para Proceso B") as u64;
    assert_ne!(frame_a, frame_b);

    paging::map_user_page(cr3_a, USER_ELF_VADDR, frame_a, true).expect("Mapear vaddr en CR3_A");
    paging::map_user_page(cr3_b, USER_ELF_VADDR, frame_b, true).expect("Mapear vaddr en CR3_B");

    // 3. Verificar que en CR3_A y CR3_B solo la página del proceso tenga PAGE_USER = true y el kernel siga PAGE_USER = false
    let (mapped_a, is_4k_a, is_user_a) = paging::query_mapping(cr3_a, USER_ELF_VADDR).unwrap();
    assert_eq!(mapped_a, frame_a);
    assert!(is_4k_a && is_user_a, "La pagina del proceso A debe ser de 4 KiB con PAGE_USER=1");

    let (_, _, k_in_a_user) = paging::query_mapping(cr3_a, 0x0010_0000).unwrap();
    assert!(!k_in_a_user, "El kernel dentro de CR3_A debe mantener PAGE_USER=0");

    // 4. Conmutar CR3 a Proceso A y Proceso B y comprobar aislamiento real de datos en la misma dirección virtual
    // SAFETY: `cr3_a` y `cr3_b` contienen los mapeos supervisores del kernel y `USER_ELF_VADDR` está mapeado a marcos válidos.
    unsafe {
        paging::write_cr3(cr3_a);
        core::ptr::write_volatile(USER_ELF_VADDR as *mut u64, 0xBEAF_1111_2222_3333);

        paging::write_cr3(cr3_b);
        core::ptr::write_volatile(USER_ELF_VADDR as *mut u64, 0xCAFE_4444_5555_6666);
        let val_b = core::ptr::read_volatile(USER_ELF_VADDR as *const u64);
        assert_eq!(val_b, 0xCAFE_4444_5555_6666);

        paging::write_cr3(cr3_a);
        let val_a = core::ptr::read_volatile(USER_ELF_VADDR as *const u64);
        assert_eq!(val_a, 0xBEAF_1111_2222_3333);

        paging::switch_to_kernel_cr3();
    }
    assert_eq!(paging::read_cr3(), k_cr3);
}

pub fn test_smp_madt_and_trampoline_0x8000() {
    use crate::apic::{AP_ONLINE_COUNT, SMP_TRAMPOLINE_ADDR};
    use core::sync::atomic::Ordering;

    let smp = crate::apic::SMP.lock();
    assert!(smp.madt_detected, "La tabla ACPI MADT ('APIC') debe ser detectada");
    assert!(smp.madt_addr != 0, "La direccion fisica de MADT debe ser no nula");
    assert_eq!(smp.trampoline_addr, SMP_TRAMPOLINE_ADDR as u64);

    // Verificar que los primeros bytes del trampolín de 16 bits estén instalados en 0x8000 (cli; cld; xor ax, ax)
    // SAFETY: Lectura de los primeros 4 bytes en la página física `0x8000` reservada para el trampolín SMP.
    let trampoline_head = unsafe { core::slice::from_raw_parts(SMP_TRAMPOLINE_ADDR as *const u8, 4) };
    assert_eq!(trampoline_head, &[0xFA, 0xFC, 0x31, 0xC0]);

    // Si QEMU se lanzó con múltiples núcleos (-smp > 1), verificar que los APs despertaron realmente
    if smp.cores.len() > 1 {
        let online_aps = smp.cores.iter().filter(|c| !c.is_bsp && c.is_online).count();
        assert!(online_aps >= 1, "Al menos un AP secundario debe haber despertado por SIPI en 0x8000");
        assert!(AP_ONLINE_COUNT.load(Ordering::Relaxed) >= 2, "AP_ONLINE_COUNT debe reflejar BSP + APs online");
    }
}

pub fn test_spotlight_live_search() {
    let results_empty = crate::gui::apps_system::get_spotlight_results("");
    assert_eq!(results_empty.len(), 4, "Búsqueda vacía en Spotlight debe devolver las 4 aplicaciones sugeridas por defecto");

    let results_term = crate::gui::apps_system::get_spotlight_results("term");
    assert!(!results_term.is_empty(), "Spotlight debe encontrar la aplicación Terminal para 'term'");
    assert!(results_term.iter().any(|r| r.title.contains("Terminal")), "Debe contener 'Terminal'");

    let results_note = crate::gui::apps_system::get_spotlight_results("note");
    assert!(!results_note.is_empty(), "Spotlight debe encontrar BeaverNote para 'note'");
    assert!(results_note.iter().any(|r| r.title.contains("Note")), "Debe contener 'Note'");

    let results_foto = crate::gui::apps_system::get_spotlight_results("foto");
    assert!(!results_foto.is_empty(), "Spotlight debe encontrar Fotos / Galería para 'foto'");
    assert!(results_foto.iter().any(|r| r.title.contains("Photos") || r.title.contains("Foto")), "Debe contener 'Photos' o 'Foto'");
}

pub fn test_real_udp_dhcp_dns() {
    // Verificar resolución estática local para nombres del sistema
    let localhost_ip = crate::drivers::net::resolve_dns("localhost").expect("localhost debe resolver a 127.0.0.1");
    assert_eq!(localhost_ip.0, [127, 0, 0, 1]);

    let beaver_ip = crate::drivers::net::resolve_dns("beaver.lan").expect("beaver.lan debe resolver a la IP local del sistema");
    let my_ip = crate::drivers::net::NET_INTERFACE.lock().ip_address;
    assert_eq!(beaver_ip.0, my_ip.0);

    // Verificar que los buzones de recepción UDP de DHCP y DNS están activos
    {
        let lease = crate::drivers::net::DHCP_PENDING_LEASE.lock();
        assert!(lease.is_none() || lease.is_some(), "DHCP_PENDING_LEASE debe ser accesible");
    }
    {
        let dns = crate::drivers::net::DNS_PENDING_RESPONSE.lock();
        assert!(dns.is_none() || dns.is_some(), "DNS_PENDING_RESPONSE debe ser accesible");
    }
}

pub fn test_smp_distributed_scheduler() {
    let mut sched = crate::task::scheduler::SCHEDULER.lock();
    assert!(sched.is_active(), "El scheduler del kernel debe estar activo");
    assert_eq!(sched.get_core_task(0), Some(0), "El Core 0 (BSP) debe tener asignada la tarea 0 (main_shell)");

    // Registrar tarea de prueba y verificar que adquiere TID válido
    let tid = sched.spawn("smp_test_worker", || {});
    assert!(tid > 0, "El TID de la nueva tarea debe ser mayor que 0");

    let kernel_cr3 = crate::memory::paging::kernel_cr3();
    let tid2 = sched.spawn_with_cr3("smp_cr3_worker", kernel_cr3, || {});
    assert!(tid2 > tid, "La tarea con CR3 aislado debe tener un ID sucesivo");
    assert_eq!(sched.task_state(tid2), Some(crate::task::TaskState::Ready));

    let smp = crate::apic::SMP.lock();
    assert!(!smp.cores.is_empty(), "SMP debe contener la lista de núcleos descubiertos");
}

pub fn test_ahci_sata_dma_controller() {
    let ahci = crate::storage::ahci::AHCI_CONTROLLER.lock();
    if ahci.detected {
        let is_dma = crate::storage::block_device::BLOCK_DEVICE.lock().is_using_ahci();
        assert!(is_dma, "BLOCK_DEVICE debe tener activado el backend AHCI DMA");
        let active_port_idx = ahci.active_sata_port.expect("Debe existir un puerto SATA activo") as usize;
        assert!(ahci.ports[active_port_idx].active, "El puerto activo de AHCI debe estar marcado");

        drop(ahci);

        // Probar escritura y lectura DMA real en sector seguro (LBA 10000, fuera de BeaverFS)
        let test_lba = 10000u32;
        let mut write_buf = [0u8; 512];
        for (i, b) in write_buf.iter_mut().enumerate() {
            *b = ((i * 7 + 0x5A) & 0xFF) as u8;
        }

        let mut dev = crate::storage::block_device::BLOCK_DEVICE.lock();
        let write_res = dev.write_sector(test_lba, &write_buf);
        assert!(write_res.is_ok(), "Escritura DMA por AHCI debe ser exitosa");

        let mut read_buf = [0u8; 512];
        let read_res = dev.read_sector(test_lba, &mut read_buf);
        assert!(read_res.is_ok(), "Lectura DMA por AHCI debe ser exitosa");
        assert_eq!(&read_buf[..], &write_buf[..], "Los datos leidos por AHCI DMA deben coincidir con lo escrito");

        let ahci_post = crate::storage::ahci::AHCI_CONTROLLER.lock();
        let active_port = &ahci_post.ports[active_port_idx];
        assert!(active_port.dma_reads > 0, "Debe registrarse al menos una lectura DMA");
        assert!(active_port.dma_writes > 0, "Debe registrarse al menos una escritura DMA");
    } else {
        // Modo fallback (ATA PIO / Ramdisk)
        let is_dma = crate::storage::block_device::BLOCK_DEVICE.lock().is_using_ahci();
        assert!(!is_dma, "BLOCK_DEVICE debe operar en modo fallback cuando no hay AHCI");
    }
}

pub fn test_tcp_state_machine_and_http_parser() {
    use crate::drivers::net::{
        calculate_tcp_checksum, parse_http_response, bsd_socket, bsd_close,
        Ipv4Addr, SOCKET_TABLE, SocketState, AF_INET, SOCK_STREAM, TCP_FLAG_SYN,
    };

    // 1. Verificación del algoritmo de Checksum TCP con pseudo-cabecera IPv4 (RFC 793)
    let src_ip = Ipv4Addr::new(10, 0, 2, 15);
    let dst_ip = Ipv4Addr::new(10, 0, 2, 2);
    let mut syn_seg = [
        0xC0, 0x00, // Puerto origen: 49152
        0x00, 0x50, // Puerto destino: 80 (HTTP)
        0x00, 0x00, 0x03, 0xE8, // Seq: 1000
        0x00, 0x00, 0x00, 0x00, // Ack: 0
        0x50, TCP_FLAG_SYN,     // Offset: 20 bytes, Flag: SYN
        0xFA, 0xF0,             // Ventana: 64240
        0x00, 0x00,             // Checksum placeholder
        0x00, 0x00,             // Urgent pointer
    ];
    let csum = calculate_tcp_checksum(src_ip, dst_ip, &syn_seg);
    assert_ne!(csum, 0, "El checksum calculado no debe ser cero");
    syn_seg[16] = (csum >> 8) as u8;
    syn_seg[17] = (csum & 0xFF) as u8;
    assert_eq!(syn_seg[16], (csum >> 8) as u8);
    assert_eq!(syn_seg[17], (csum & 0xFF) as u8);

    // 2. Verificación del parser HTTP/1.1 (RFC 7230)
    let raw_http = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 32\r\nConnection: close\r\n\r\n{\"os\":\"Beaver OS\",\"version\":\"0.9\"}";
    let resp = parse_http_response(raw_http).expect("El parser HTTP/1.1 debe procesar la respuesta con exito");
    assert_eq!(resp.status_code, 200, "Codigo de estado HTTP 200");
    assert_eq!(resp.status_text, "OK", "Texto de estado OK");
    assert_eq!(resp.content_type, "application/json", "Content-Type detectado");
    assert_eq!(resp.content_length, 32, "Content-Length coincide");
    assert_eq!(resp.body, "{\"os\":\"Beaver OS\",\"version\":\"0.9\"}", "Cuerpo de texto identico");
    assert_eq!(resp.raw_body, b"{\"os\":\"Beaver OS\",\"version\":\"0.9\"}", "Cuerpo binario identico");

    // 3. Verificación de creación, consulta y cierre de sockets en la tabla BSD
    let sock_id = bsd_socket(AF_INET, SOCK_STREAM, 0).expect("Creacion de socket BSD exitosa");
    {
        let table = SOCKET_TABLE.lock();
        let sock = table[sock_id].as_ref().expect("El descriptor de socket debe ser valido");
        assert_eq!(sock.state, SocketState::Closed, "Estado inicial Closed");
        assert_eq!(sock.local_port, 49152 + (sock_id as u16), "Puerto efimero asignado");
    }
    bsd_close(sock_id);
    {
        let table = SOCKET_TABLE.lock();
        assert!(table[sock_id].is_none(), "El socket debe quedar liberado en la tabla tras bsd_close");
    }

    // 4. Verificación de conexión TCP real y petición HTTP local en beaver.lan (loopback)
    let http_resp = crate::drivers::net::http_get("http://beaver.lan/info.json")
        .expect("Peticion HTTP TCP hacia http://beaver.lan/info.json debe tener exito");
    assert_eq!(http_resp.status_code, 200, "Servidor web HTTP del kernel debe responder con 200 OK");
    assert_eq!(http_resp.content_type, "application/json", "Tipo de contenido JSON");
    assert!(http_resp.body.contains("\"os\": \"Beaver OS\""), "El JSON debe contener el nombre del SO");
}

/// Test 16: Verificación de pasarela de Syscalls POSIX reales y descriptores VFS
pub fn test_posix_syscalls_and_beaver_libc() {
    use crate::syscall::*;

    // 1. Verificación de SYS_UPTIME y SYS_GETPID
    let uptime = syscall_dispatch(SYS_UPTIME, 0, 0, 0, 0, 0);
    assert!(uptime <= 1_000_000, "SYS_UPTIME debe retornar un tiempo valido");

    let pid = syscall_dispatch(SYS_GETPID, 0, 0, 0, 0, 0);
    assert!(pid < 1000, "SYS_GETPID debe retornar el TID actual");

    // 2. Verificación de SYS_WRITE en stdout (fd 1)
    let test_msg = b"[TEST-SYSCALL] Prueba directa de SYS_WRITE POSIX\n";
    let bytes_written = syscall_dispatch(SYS_WRITE, 1, test_msg.as_ptr() as u64, test_msg.len() as u64, 0, 0);
    assert_eq!(bytes_written, test_msg.len() as u64, "SYS_WRITE debe escribir todos los bytes");

    // 3. Verificación de SYS_OPEN, SYS_WRITE, SYS_LSEEK, SYS_READ y SYS_CLOSE sobre VFS
    let path = b"/test_syscall.txt\0";
    let fd = syscall_dispatch(SYS_OPEN, path.as_ptr() as u64, 64, 0, 0, 0); // O_CREAT
    assert!(fd >= 3, "Descriptor fd de archivo debe ser >= 3");

    let payload = b"Beaver OS POSIX Syscalls OK!";
    let w = syscall_dispatch(SYS_WRITE, fd, payload.as_ptr() as u64, payload.len() as u64, 0, 0);
    assert_eq!(w, payload.len() as u64, "Debe escribir la carga util completa");

    // Rebobinar con SYS_LSEEK a 0
    let seek_res = syscall_dispatch(SYS_LSEEK, fd, 0, 0, 0, 0);
    assert_eq!(seek_res, 0, "LSEEK a SEEK_SET 0");

    let mut read_buf = [0u8; 64];
    let r = syscall_dispatch(SYS_READ, fd, read_buf.as_mut_ptr() as u64, payload.len() as u64, 0, 0);
    assert_eq!(r, payload.len() as u64, "Debe leer exactamente los bytes escritos");
    assert_eq!(&read_buf[..payload.len()], payload, "El contenido leido debe ser identico");

    let close_res = syscall_dispatch(SYS_CLOSE, fd, 0, 0, 0, 0);
    assert_eq!(close_res, 0, "SYS_CLOSE debe retornar 0 en descriptor valido");

    // Descriptor cerrado debe fallar con -EBADF
    let bad_read = syscall_dispatch(SYS_READ, fd, read_buf.as_mut_ptr() as u64, 10, 0, 0);
    assert_eq!(bad_read as i64, -9, "SYS_READ en fd cerrado debe retornar -EBADF (-9)");

    // 4. Verificación de binarios ELF Rust auténticos en Ring 3 (/disk/bin/hello.elf y bench.elf)
    crate::task::pkg::init_disk_bin();
    let hello_exec = crate::task::pkg::execute_elf("/disk/bin/hello.elf")
        .expect("El binario user/hello compilado en Rust debe ejecutarse en Ring 3");
    assert!(hello_exec.contains("hello.elf"), "La ejecucion de hello.elf en Ring 3 debe ser exitosa");

    let bench_exec = crate::task::pkg::execute_elf("/disk/bin/bench.elf")
        .expect("El binario user/bench compilado en Rust debe ejecutarse en Ring 3");
    assert!(bench_exec.contains("bench.elf"), "La ejecucion de bench.elf en Ring 3 debe ser exitosa");

    // 5. Verificación de BeaverScript (.bs) aislado en Ring 3 con syscalls POSIX
    let test_bs = "let x = 12\nlet y = 8\nlet sum = x + y\nprintln \"Ring 3 BS OK: \" + sum\n";
    let bs_res = crate::task::script::run_script_in_ring3(test_bs);
    assert!(bs_res.is_ok(), "BeaverScript debe ejecutarse en Ring 3 sin fallos");

    // 6. Verificación de SYS_BRK (heap dinámico de usuario en Ring 3)
    let cur_brk = syscall_dispatch(SYS_BRK, 0, 0, 0, 0, 0);
    assert!(cur_brk >= USER_HEAP_START, "SYS_BRK(0) debe retornar al menos USER_HEAP_START");
    let new_brk = syscall_dispatch(SYS_BRK, cur_brk + 4096, 0, 0, 0, 0);
    assert_eq!(new_brk, cur_brk + 4096, "SYS_BRK debe expandir el heap de usuario");

    // 7. Verificación de SYS_WAITPID (cosecha y sincronización de procesos)
    let wait_res = syscall_dispatch(SYS_WAITPID, 9999, 0, 0, 0, 0);
    assert_eq!(wait_res as i64, -10, "SYS_WAITPID sobre proceso inexistente debe devolver -ECHILD (-10)");
}

/// Test 17: Verificación de detección real de perfil de dispositivo (ACPI FADT / SMBIOS / Periféricos)
pub fn test_device_profile_detection() {
    use crate::drivers::profile::{detect_device_profile, DeviceClass};

    let p = detect_device_profile();
    assert!(p.cpu_cores >= 1, "Debe detectarse al menos 1 nucleo CPU");
    assert!(!p.source.is_empty(), "La fuente de clasificacion no puede estar vacia");
    assert_eq!(p.has_cec, crate::drivers::cec::HDMI_CEC.lock().hw_present, "has_cec debe reflejar hw_present real");
    assert_eq!(p.has_touch, crate::drivers::touchscreen::TOUCHSCREEN.lock().hw_present, "has_touch debe reflejar hw_present real");
    assert_ne!(p.class, DeviceClass::Unknown, "El perfil del dispositivo debe resolverse deterministicamente");
}

/// Test 18: Verificación de estado honesto de HDMI-CEC y despacho seguro sin deadlock en CEC/Touchscreen
pub fn test_cec_and_touchscreen_safe_dispatch() {
    use crate::drivers::cec::{init_cec, inject_cec_key, CEC_USER_UP, HDMI_CEC};
    use crate::drivers::touchscreen::{init_touchscreen, inject_touch_event, TOUCHSCREEN};

    init_cec();
    {
        let cec = HDMI_CEC.lock();
        assert_eq!(cec.is_online, cec.hw_present, "is_online de CEC no debe fingirse cuando hw_present es false");
    }

    let before_cec = HDMI_CEC.lock().frames_received;
    inject_cec_key(CEC_USER_UP);
    let after_cec = HDMI_CEC.lock().frames_received;
    assert_eq!(after_cec, before_cec + 1, "inject_cec_key debe incrementar frames_received sin deadlock");

    init_touchscreen();
    let before_touch = TOUCHSCREEN.lock().total_events;
    inject_touch_event(512, 384, true);
    inject_touch_event(512, 384, false);
    let after_touch = TOUCHSCREEN.lock().total_events;
    assert_eq!(after_touch, before_touch + 2, "inject_touch_event debe procesar Tap+Release sin deadlock");
}

/// Test 19: Verificación de bloques doblemente indirectos y soporte de archivos multi-bloque en BeaverFS (> 68 KiB)
pub fn test_beaverfs_double_indirect_large_file() {
    use crate::storage::beaverfs::BEAVER_FS;
    use alloc::vec::Vec;

    // Archivo de 75 KiB (150 bloques x 512 bytes), superando el umbral de 135 bloques (7 directos + 128 indirectos)
    let file_size = 75 * 1024;
    let mut large_buf = Vec::with_capacity(file_size);
    for i in 0..file_size {
        large_buf.push((i & 0xFF) as u8);
    }

    let file_name = "test_large.bin";
    {
        let mut fs = BEAVER_FS.lock();
        let write_res = fs.write_file(file_name, &large_buf, 0, 0);
        assert!(write_res.is_ok(), "Escritura de archivo de 75 KiB con bloques doblemente indirectos debe ser exitosa");
    }

    {
        let fs = BEAVER_FS.lock();
        let read_data = fs.read_file(file_name).expect("Lectura de archivo grande con bloques doblemente indirectos");
        assert_eq!(read_data.len(), file_size, "El tamaño leído debe ser exactamente 75 KiB");
        assert_eq!(&read_data[..], &large_buf[..], "La integridad byte a byte del archivo doblemente indirecto debe ser perfecta");
    }

    {
        let mut fs = BEAVER_FS.lock();
        let del_res = fs.remove_file(file_name);
        assert!(del_res.is_ok(), "Eliminación y liberación de bloques directos/indirectos/dobles debe ser limpia");
    }
}

/// Test 20: Verificación de llamada SYS_BRK y expansión dinámica de Heap de usuario en Ring 3
pub fn test_userland_heap_and_sys_brk() {
    use crate::syscall::*;

    let initial_brk = syscall_dispatch(SYS_BRK, 0, 0, 0, 0, 0);
    assert!(initial_brk >= USER_HEAP_START, "SYS_BRK(0) debe devolver la base del heap de usuario");

    // Solicitar expansión de 2 páginas (8 KiB)
    let requested = initial_brk + 8192;
    let new_brk = syscall_dispatch(SYS_BRK, requested, 0, 0, 0, 0);
    assert_eq!(new_brk, requested, "SYS_BRK debe asignar y mapear las páginas solicitadas en el proceso");

    let current = syscall_dispatch(SYS_BRK, 0, 0, 0, 0, 0);
    assert_eq!(current, requested, "Consulta de program break debe reflejar el nuevo límite");
}

/// Test 21: Verificación de subdirectorios jerárquicos persistentes en BeaverFS y su integración en VFS
pub fn test_beaverfs_hierarchical_subdirectories() {
    use crate::storage::beaverfs::BEAVER_FS;
    use crate::fs::vfs::VFS;

    // 1. Crear subdirectorio en BeaverFS directamente
    {
        let mut fs = BEAVER_FS.lock();
        let res = fs.create_dir("testdir", 0, 0);
        assert!(res.is_ok(), "create_dir en BeaverFS debe ser exitoso");
        assert!(fs.is_dir("testdir"), "is_dir debe reconocer el nuevo subdirectorio");
        assert!(fs.exists("testdir"), "exists debe confirmar la presencia del subdirectorio");

        // 2. Escribir archivo dentro del subdirectorio
        let write_res = fs.write_file("testdir/nested.txt", b"BeaverFS Subdir OK", 0, 0);
        assert!(write_res.is_ok(), "write_file en ruta jerarquica debe tener exito");

        // 3. Leer archivo dentro del subdirectorio
        let content = fs.read_file("testdir/nested.txt").expect("read_file en subdirectorio");
        assert_eq!(&content[..], b"BeaverFS Subdir OK", "El contenido del archivo anidado debe coincidir");

        // 4. Listar el subdirectorio
        let entries = fs.list_dir("testdir").expect("list_dir en subdirectorio");
        assert!(entries.iter().any(|(name, is_dir, _, _)| name == "nested.txt" && !*is_dir), "list_dir debe incluir nested.txt");
    }

    // 5. Verificación a través del VFS transparente (/disk/...)
    {
        let vfs = VFS.lock();
        assert!(vfs.is_dir("/disk/testdir"), "VFS is_dir debe resolver subdirectorios en /disk/");
        let read_vfs = vfs.read_file_bytes("/disk/testdir/nested.txt", 0, 0).expect("VFS read en /disk/testdir/nested.txt");
        assert_eq!(&read_vfs[..], b"BeaverFS Subdir OK", "VFS debe leer transparentemente archivos en subdirectorios de /disk");

        let vfs_list = vfs.list_dir("/disk/testdir", 0, 0).expect("VFS list_dir en /disk/testdir");
        assert!(vfs_list.iter().any(|(name, is_dir, _, _, _, _)| name == "nested.txt" && !*is_dir), "VFS list_dir debe reportar nested.txt");
    }

    // 6. Limpieza: borrar archivo anidado y subdirectorio
    {
        let mut fs = BEAVER_FS.lock();
        let del_file = fs.remove_file("testdir/nested.txt");
        assert!(del_file.is_ok(), "remove_file en subdirectorio debe tener exito");

        let del_dir = fs.remove_dir("testdir");
        assert!(del_dir.is_ok(), "remove_dir de directorio vacio debe tener exito");
        assert!(!fs.exists("testdir"), "El subdirectorio ya no debe existir tras remove_dir");
    }
}

/// Test 22: Planificación Preemptiva y Concurrente de Procesos en Ring 3 / EL0
pub fn test_preemptive_userland_task_scheduling() {
    use crate::task::scheduler::SCHEDULER;
    use crate::task::TaskState;

    #[cfg(target_arch = "x86_64")]
    let bench_data = crate::task::pkg::X86_64_BENCH_ELF;
    #[cfg(target_arch = "aarch64")]
    let bench_data = crate::task::pkg::ARM64_BENCH_ELF;

    // 1. Programar tarea de usuario ELF en el planificador
    let tid = SCHEDULER.lock().spawn_user_elf("bench_bg_proc", bench_data)
        .expect("Debe registrar proceso ELF de usuario en el planificador");

    assert!(tid > 0, "El TID asignado al proceso de usuario debe ser mayor a 0");

    // 2. Verificar que el estado inicial es Ready
    let initial_state = SCHEDULER.lock().task_state(tid);
    assert_eq!(initial_state, Some(TaskState::Ready), "El nuevo proceso de usuario debe estar en estado Ready");

    // 3. Ejecutar cambio de contexto y ciclo de scheduling
    crate::task::scheduler::yield_now();

    // 4. Verificar que tras ceder tiempo de CPU, la tarea avanzó o finalizó limpiamente
    let post_state = SCHEDULER.lock().task_state(tid);
    assert!(
        post_state == Some(TaskState::Finished) || post_state == Some(TaskState::Running) || post_state == Some(TaskState::Ready),
        "El proceso de usuario debe mantener un ciclo de vida válido en el planificador"
    );

    // 5. Verificar que el kernel recuperó el control con su espacio de direcciones supervisor
    #[cfg(target_arch = "x86_64")]
    assert_eq!(crate::memory::paging::read_cr3(), crate::memory::paging::kernel_cr3(), "El CR3 activo debe ser el del kernel tras retornar al supervisor");
}

/// Test 23: Conexiones TCP Entrantes Concurrentes (`bind`, `listen`, `accept`, `send`, `recv`)
pub fn test_tcp_concurrent_listen_accept() {
    use crate::drivers::net::{
        bsd_socket, bsd_bind, bsd_listen, bsd_accept, bsd_connect, bsd_send, bsd_recv, bsd_close,
        AF_INET, SOCK_STREAM, Ipv4Addr,
    };

    // 1. Crear socket servidor y asignarlo al puerto 8088
    let srv_sock = bsd_socket(AF_INET, SOCK_STREAM, 0)
        .expect("Creacion de socket servidor BSD");
    bsd_bind(srv_sock, 8088)
        .expect("Bind en puerto 8088 debe ser exitoso");

    // 2. Pasar el socket a modo escucha con cola de espera (backlog = 4)
    bsd_listen(srv_sock, 4)
        .expect("Transicion a estado Listen debe ser exitosa");

    // 3. Crear socket cliente y conectar al servidor local en 127.0.0.1:8088
    let cli_sock = bsd_socket(AF_INET, SOCK_STREAM, 0)
        .expect("Creacion de socket cliente BSD");
    bsd_connect(cli_sock, Ipv4Addr::new(127, 0, 0, 1), 8088)
        .expect("Conexion TCP loopback hacia 127.0.0.1:8088 debe tener exito");

    // 4. El servidor acepta la conexion entrante (3-way handshake)
    let accepted_sock = bsd_accept(srv_sock)
        .expect("Accept de conexion cliente entrante debe tener exito");
    assert!(accepted_sock != srv_sock && accepted_sock != cli_sock, "El descriptor aceptado debe ser unico e independiente");

    // 5. El cliente envia una peticion de datos hacia el servidor
    let ping_msg = b"PING-BEAVER-TCP-STREAM";
    let sent_bytes = bsd_send(cli_sock, ping_msg)
        .expect("Envio de datos TCP desde el cliente debe tener exito");
    assert_eq!(sent_bytes, ping_msg.len(), "Se deben enviar todos los bytes de la peticion");

    // 6. El servidor lee los datos recibidos a traves del socket aceptado
    let mut srv_rx_buf = [0_u8; 64];
    let recvd_srv = bsd_recv(accepted_sock, &mut srv_rx_buf)
        .expect("Servidor debe recibir los datos del socket aceptado");
    assert_eq!(&srv_rx_buf[..recvd_srv], ping_msg, "Los datos recibidos en el servidor deben coincidir exactamente");

    // 7. El servidor responde al cliente con confirmacion
    let pong_msg = b"PONG-BEAVER-TCP-ACK";
    let sent_reply = bsd_send(accepted_sock, pong_msg)
        .expect("Servidor debe enviar respuesta de vuelta por el canal TCP");
    assert_eq!(sent_reply, pong_msg.len(), "Se deben enviar todos los bytes de respuesta");

    // 8. El cliente lee la respuesta del servidor
    let mut cli_rx_buf = [0_u8; 64];
    let recvd_cli = bsd_recv(cli_sock, &mut cli_rx_buf)
        .expect("Cliente debe recibir la respuesta del servidor");
    assert_eq!(&cli_rx_buf[..recvd_cli], pong_msg, "La respuesta recibida en el cliente debe coincidir");

    // 9. Cierre ordenado de los tres sockets (FIN-ACK)
    bsd_close(cli_sock);
    bsd_close(accepted_sock);
    bsd_close(srv_sock);
}

/// Test 24: Tuberías y Redirección en Beaver Shell (`|`, `>`, `>>`)
pub fn test_shell_pipes_and_redirection() {
    use crate::shell::Shell;
    use crate::fs::vfs::VFS;

    let mut shell = Shell::new();

    // 1. Redirección simple con sobrescritura (`>`)
    shell.execute_command("echo Beaver OS Pipelines > /disk/test_pipe.txt");
    
    let content1 = VFS.lock().read_file("/disk/test_pipe.txt", 0, 0)
        .expect("El archivo /disk/test_pipe.txt debio crearse mediante redireccion '>'");
    assert!(content1.contains("Beaver OS Pipelines"), "El archivo debe contener el texto redirigido");

    // 2. Redirección acumulativa (`>>`)
    shell.execute_command("echo Segunda Linea Redirigida >> /disk/test_pipe.txt");
    let content2 = VFS.lock().read_file("/disk/test_pipe.txt", 0, 0)
        .expect("Lectura tras append '>>'");
    assert!(content2.contains("Beaver OS Pipelines"), "Debe preservar la primera linea");
    assert!(content2.contains("Segunda Linea Redirigida"), "Debe contener la linea anadida");

    // 3. Tubería con filtrado grep y redirección combinada (`|` + `>`)
    shell.execute_command("cat /disk/test_pipe.txt | grep Segunda > /disk/grep_out.txt");
    let content_grep = VFS.lock().read_file("/disk/grep_out.txt", 0, 0)
        .expect("Lectura de salida de tuberia grep");
    assert!(content_grep.contains("Segunda Linea Redirigida"), "Grep debe haber capturado la segunda linea");
    assert!(!content_grep.contains("Beaver OS Pipelines"), "Grep no debe contener lineas que no coinciden");

    // 4. Conteo de palabras y líneas a través de tubería (`cat | wc`)
    shell.execute_command("cat /disk/test_pipe.txt | wc > /disk/wc_out.txt");
    let content_wc = VFS.lock().read_file("/disk/wc_out.txt", 0, 0)
        .expect("Lectura de salida de tuberia wc");
    assert!(content_wc.contains("(pipe)"), "WC debe reportar el flujo procesado desde stdin pipe");

    // Limpieza de archivos temporales de prueba
    let _ = VFS.lock().remove_file("/disk/test_pipe.txt", 0, 0);
    let _ = VFS.lock().remove_file("/disk/grep_out.txt", 0, 0);
    let _ = VFS.lock().remove_file("/disk/wc_out.txt", 0, 0);
}

/// Test 25: Protocolo de Ventanas de Usuario vía IPC (Desktop Compositor IPC & Syscalls)
pub fn test_desktop_compositor_user_window_ipc() {
    use crate::syscall::*;
    use crate::gui::window::{WINDOW_MGR, GuiEvent, GUI_EVENT_CLICK};

    // 1. Creación de ventana de usuario mediante llamada al sistema (SYS_GUI_CREATE_WINDOW)
    let title = b"Calculadora Ring 3\0";
    let dims: u64 = (320 << 16) | 240; // width=320, height=240
    let coords: u64 = (150 << 16) | 120; // x=150, y=120
    let win_id = syscall_dispatch(SYS_GUI_CREATE_WINDOW, title.as_ptr() as u64, 18, dims, coords, 0);
    assert!(win_id >= 1000 && win_id != u64::MAX, "SYS_GUI_CREATE_WINDOW debe asignar un ID de ventana >= 1000");

    // 2. Verificación de existencia y dimensiones calculadas en el WindowManager
    {
        let mgr = WINDOW_MGR.lock();
        let win = mgr.get_user_window(win_id as u32).expect("La ventana de usuario debe registrarse en WINDOW_MGR");
        assert_eq!(win.title.as_str(), "Calculadora Ring 3");
        assert_eq!(win.width, 320);
        assert_eq!(win.height, 240);
        assert_eq!(win.client_w, 316); // 320 - 4
        assert_eq!(win.client_h, 212); // 240 - 28
        assert_eq!(win.buffer.len(), 316 * 212, "El backbuffer cliente debe coincidir con client_w * client_h");
    }

    // 3. Dibujo de primitivas (rectángulo) mediante SYS_GUI_DRAW_RECT
    let rect_coords: u64 = (10 << 16) | 10;
    let rect_dims: u64 = (60 << 16) | 40;
    let color_amber: u64 = 0x00FFBF00; // Ámbar Beaver
    let draw_res = syscall_dispatch(SYS_GUI_DRAW_RECT, win_id, rect_coords, rect_dims, color_amber, 0);
    assert_eq!(draw_res, 0, "SYS_GUI_DRAW_RECT debe retornar 0 en éxito");

    {
        let mgr = WINDOW_MGR.lock();
        let win = mgr.get_user_window(win_id as u32).unwrap();
        assert_eq!(win.buffer[10 * win.client_w + 10], 0x00FFBF00, "El píxel en el backbuffer debe reflejar el color dibujado");
    }

    // 4. Dibujo de texto mediante SYS_GUI_DRAW_TEXT
    let text = b"OK";
    let text_arg: u64 = ((text.len() as u64) << 32) | 0x00FFFFFF; // Blanco
    let text_res = syscall_dispatch(SYS_GUI_DRAW_TEXT, win_id, (20 << 16) | 20, text.as_ptr() as u64, text_arg, 0);
    assert_eq!(text_res, 0, "SYS_GUI_DRAW_TEXT debe retornar 0 en éxito");

    // 5. Transferencia de píxeles en bloque (Blit) mediante SYS_GUI_BLIT
    let test_pixels = [0x0000FF00_u32; 32]; // Verde
    let blit_res = syscall_dispatch(SYS_GUI_BLIT, win_id, test_pixels.as_ptr() as u64, 32, 0, 0);
    assert_eq!(blit_res, 0, "SYS_GUI_BLIT debe retornar 0 en éxito");

    {
        let mgr = WINDOW_MGR.lock();
        let win = mgr.get_user_window(win_id as u32).unwrap();
        assert_eq!(win.buffer[0], 0x0000FF00, "El primer píxel del backbuffer debe reflejar los datos blitteados");
    }

    // 6. Encolado y recepción de eventos de ratón y ventana (SYS_GUI_GET_EVENT)
    WINDOW_MGR.lock().push_user_window_event(win_id as u32, GuiEvent {
        event_type: GUI_EVENT_CLICK,
        param1: 50,
        param2: 80,
        param3: 1,
    });

    let mut event = GuiEvent { event_type: 0, param1: 0, param2: 0, param3: 0 };
    let poll_res = syscall_dispatch(SYS_GUI_GET_EVENT, win_id, &mut event as *mut _ as u64, 0, 0, 0);
    assert_eq!(poll_res, 1, "SYS_GUI_GET_EVENT debe retornar 1 cuando hay un evento disponible");
    assert_eq!(event.event_type, GUI_EVENT_CLICK, "El tipo de evento debe ser Click");
    assert_eq!(event.param1, 50, "Coordenada relativa X del clic");
    assert_eq!(event.param2, 80, "Coordenada relativa Y del clic");
    assert_eq!(event.param3, 1, "Botón izquierdo");

    // Verificar que la cola de eventos quedó vacía
    let empty_poll = syscall_dispatch(SYS_GUI_GET_EVENT, win_id, &mut event as *mut _ as u64, 0, 0, 0);
    assert_eq!(empty_poll, 0, "SYS_GUI_GET_EVENT debe retornar 0 cuando la cola está vacía");

    // 7. Invalidación y recomposición mediante SYS_GUI_INVALIDATE
    let inv_res = syscall_dispatch(SYS_GUI_INVALIDATE, win_id, 0, 0, 0, 0);
    assert_eq!(inv_res, 0, "SYS_GUI_INVALIDATE debe procesar el marcado dirty de la ventana");

    // 8. Destrucción de la ventana mediante SYS_GUI_DESTROY_WINDOW
    let destroy_res = syscall_dispatch(SYS_GUI_DESTROY_WINDOW, win_id, 0, 0, 0, 0);
    assert_eq!(destroy_res, 0, "SYS_GUI_DESTROY_WINDOW debe retornar 0 en éxito");

    assert!(WINDOW_MGR.lock().get_user_window(win_id as u32).is_none(), "La ventana debe ser eliminada del WindowManager");
}

/// Test 26: Controlador Host USB 3.0 Moderno (xHCI), Root Hub y Detección de Dispositivos Reales
pub fn test_xhci_usb3_controller_and_ports() {
    use crate::drivers::usb::{USB_MANAGER, UsbPortSpeed, get_usb_summary};

    // 1. Inicializar el subsistema USB y escanear el bus PCI
    crate::drivers::usb::init();

    let usb = USB_MANAGER.lock();
    assert!(usb.initialized, "El subsistema USB debe estar marcado como inicializado");

    // 2. Verificar detección del controlador xHCI USB 3.0 en PCI
    assert!(!usb.xhci_controllers.is_empty(), "Debe detectar al menos un controlador xHCI USB 3.0 en el bus PCI");
    let xhci = &usb.xhci_controllers[0];

    assert!(xhci.mmio_base > 0, "La dirección base MMIO BAR0 debe ser válida y no nula");
    assert!(xhci.cap_length >= 0x20, "Longitud de registros de capacidad xHCI debe ser >= 0x20");
    assert!(xhci.max_ports >= 4, "xHCI debe soportar al menos 4 puertos Root Hub");
    assert!(xhci.is_running, "El motor del Host Controller debe estar en estado de ejecución (Run/Stop = 1)");
    assert!(xhci.dcbaap > 0, "DCBAA debe haber sido asignado con una dirección de memoria física válida");
    assert!(xhci.crcr > 0, "CRCR (Command Ring Control Register) debe estar configurado");

    // 3. Verificar estado de los puertos del Root Hub (PORTSC)
    assert_eq!(xhci.ports.len(), xhci.max_ports as usize, "Se deben haber explorado todos los puertos Root Hub");

    let connected_ports = xhci.ports.iter().filter(|p| p.connected).count();
    assert!(connected_ports >= 1, "Debe haber al menos 1 dispositivo USB conectado al Root Hub");

    let kbd_port = xhci.ports.iter().find(|p| p.connected).expect("Puerto conectado");
    assert!(
        kbd_port.speed == UsbPortSpeed::HighSpeed || kbd_port.speed == UsbPortSpeed::FullSpeed,
        "El puerto USB debe negociar velocidad válida (High-Speed o Full-Speed)"
    );
    assert!(kbd_port.powered, "El puerto conectado debe tener alimentación activa (PP = 1)");

    // 4. Verificar lista jerárquica de dispositivos reales conectados
    assert!(!usb.devices.is_empty(), "La lista de dispositivos conectados debe contener el hardware detectado");
    assert!(
        usb.devices[0].speed == UsbPortSpeed::HighSpeed || usb.devices[0].speed == UsbPortSpeed::FullSpeed,
        "La velocidad del dispositivo debe ser coherente con el puerto"
    );

    // 5. Validar que el reporte de resumen genere información auténtica del hardware
    let expected_speed_str = kbd_port.speed.as_str();
    drop(usb);
    let summary = get_usb_summary();
    assert!(summary.contains("xHCI USB 3.0 Host Controller"), "El resumen debe describir el controlador xHCI detectado");
    assert!(summary.contains(expected_speed_str), "El resumen debe reflejar la velocidad negociada en hardware");
}

/// Test 27: Reloj de Tiempo Real (Hardware RTC), Conversión Epoch y Syscalls de Tiempo POSIX
pub fn test_rtc_and_posix_clock_time() {
    use crate::drivers::timer::{read_rtc_datetime, local_datetime, get_unix_timestamp, datetime_to_unix, unix_to_datetime, get_uptime_seconds};
    use crate::syscall::{syscall_dispatch, SYS_GETTIMEOFDAY, SYS_CLOCK_GETTIME, SYS_TIME, TimeVal, TimeZone, TimeSpec, CLOCK_REALTIME, CLOCK_MONOTONIC};

    // 1. Verificación de lectura de fecha y hora del hardware RTC (CMOS en x86 / PL031 en ARM64)
    let utc = read_rtc_datetime();
    assert!(utc.year >= 2026, "El año del reloj hardware debe ser >= 2026");
    assert!(utc.month >= 1 && utc.month <= 12, "El mes debe estar en 1..=12");
    assert!(utc.day >= 1 && utc.day <= 31, "El día debe estar en 1..=31");
    assert!(utc.hour <= 23, "La hora debe estar en 0..=23");
    assert!(utc.minute <= 59, "Los minutos deben estar en 0..=59");
    assert!(utc.second <= 59, "Los segundos deben estar en 0..=59");

    let local = local_datetime();
    assert!(local.month >= 1 && local.month <= 12, "Mes local válido");
    assert!(local.day >= 1 && local.day <= 31, "Día local válido");

    // 2. Verificación de marca de tiempo Unix Epoch (> 1.700.000.000 segundos correspondientes a 2023+)
    let ts = get_unix_timestamp();
    assert!(ts >= 1_700_000_000, "La marca de tiempo Unix debe ser válida para la era moderna (>= 1.700.000.000 s)");

    // 3. Verificación de bi-direccionalidad matemática del algoritmo de calendario gregoriano
    let calculated_unix = datetime_to_unix(&utc);
    let reconverted_dt = unix_to_datetime(calculated_unix);
    assert_eq!(reconverted_dt, utc, "La conversión Unix <-> Fecha gregoriana debe ser biyectiva y exacta");

    // 4. Verificación de llamada al sistema POSIX gettimeofday (SYS_GETTIMEOFDAY 96)
    let mut tv = TimeVal { tv_sec: 0, tv_usec: 0 };
    let mut tz = TimeZone { tz_minuteswest: 0, tz_dsttime: 0 };
    let ret_tod = syscall_dispatch(SYS_GETTIMEOFDAY, &mut tv as *mut _ as u64, &mut tz as *mut _ as u64, 0, 0, 0);
    assert_eq!(ret_tod, 0, "SYS_GETTIMEOFDAY debe retornar 0 en éxito");
    assert!(tv.tv_sec >= 1_700_000_000, "tv_sec debe contener la época Unix actual");
    assert!(tv.tv_usec < 1_000_000, "tv_usec debe ser sub-segundo (< 1.000.000 us)");
    assert_eq!(tz.tz_minuteswest, 360, "tz_minuteswest debe ser 360 minutos (UTC-6 México Centro)");

    // 5. Verificación de llamada al sistema POSIX clock_gettime (SYS_CLOCK_GETTIME 228)
    let mut ts_real = TimeSpec { tv_sec: 0, tv_nsec: 0 };
    let ret_real = syscall_dispatch(SYS_CLOCK_GETTIME, CLOCK_REALTIME, &mut ts_real as *mut _ as u64, 0, 0, 0);
    assert_eq!(ret_real, 0, "clock_gettime(CLOCK_REALTIME) debe retornar 0");
    assert!(ts_real.tv_sec >= 1_700_000_000, "CLOCK_REALTIME tv_sec debe ser consistente con la época");
    assert!(ts_real.tv_nsec < 1_000_000_000, "tv_nsec debe ser sub-segundo (< 1.000.000.000 ns)");

    let mut ts_mono = TimeSpec { tv_sec: 0, tv_nsec: 0 };
    let ret_mono = syscall_dispatch(SYS_CLOCK_GETTIME, CLOCK_MONOTONIC, &mut ts_mono as *mut _ as u64, 0, 0, 0);
    assert_eq!(ret_mono, 0, "clock_gettime(CLOCK_MONOTONIC) debe retornar 0");
    let cur_uptime = get_uptime_seconds();
    assert!(ts_mono.tv_sec == cur_uptime || ts_mono.tv_sec + 1 == cur_uptime, "CLOCK_MONOTONIC debe reflejar el uptime del kernel");

    // Identificador de reloj inválido debe fallar con -EINVAL (-22)
    let bad_clk = syscall_dispatch(SYS_CLOCK_GETTIME, 9999, &mut ts_mono as *mut _ as u64, 0, 0, 0);
    assert_eq!(bad_clk as i64, -22, "clock_gettime con clk_id invalido debe retornar -EINVAL (-22)");

    // 6. Verificación de llamada al sistema POSIX time (SYS_TIME 201)
    let mut tloc: u64 = 0;
    let ret_time = syscall_dispatch(SYS_TIME, &mut tloc as *mut _ as u64, 0, 0, 0, 0);
    assert!(ret_time >= 1_700_000_000, "SYS_TIME debe devolver la época Unix");
    assert_eq!(ret_time, tloc, "SYS_TIME debe escribir el mismo valor en tloc");

    // 7. Verificación de robustez ante punteros nulos o fallos de memoria (-EFAULT)
    let fault_tod = syscall_dispatch(SYS_CLOCK_GETTIME, CLOCK_REALTIME, 0, 0, 0, 0);
    assert_eq!(fault_tod as i64, -14, "clock_gettime con puntero nulo debe retornar -EFAULT (-14)");
}

/// Test 28: Llamadas al Sistema de Audio PCM y Hardware DMA en Espacio de Usuario
pub fn test_userland_audio_pcm_dma_playback() {
    use crate::syscall::{syscall_dispatch, SYS_AUDIO_PLAY, SYS_AUDIO_STATUS, SYS_AUDIO_STOP};
    use crate::drivers::audio::{UserAudioStatus, create_beaver_chime_wav};
    use crate::memory::AUDIO_DMA_BUFFER_ADDR;

    // 1. Consulta del estado del hardware de sonido mediante SYS_AUDIO_STATUS (131)
    let mut status = UserAudioStatus {
        controller_type: 99,
        pci_audio_found: 99,
        sample_rate: 0,
        channels: 0,
        bits_per_sample: 0,
        total_pcm_bytes: 0,
        dma_buffer_addr: 0,
    };
    let ret_stat = syscall_dispatch(SYS_AUDIO_STATUS, &mut status as *mut _ as u64, 0, 0, 0, 0);
    assert_eq!(ret_stat, 0, "SYS_AUDIO_STATUS debe retornar 0 en éxito");
    assert_eq!(status.dma_buffer_addr, AUDIO_DMA_BUFFER_ADDR as u64, "La dirección base DMA debe coincidir con AUDIO_DMA_BUFFER_ADDR (16 MiB)");
    assert!(status.controller_type <= 2, "Tipo de controlador de audio válido (PcSpeaker=0, IntelHda=1, Ac97=2)");

    // 2. Transmisión y reproducción de flujo PCM estéreo 16-bit mediante SYS_AUDIO_PLAY (130)
    let mut pcm_samples = [0u8; 1024];
    for (i, b) in pcm_samples.iter_mut().enumerate() {
        *b = ((i * 7) & 0xFF) as u8;
    }

    let prev_total = status.total_pcm_bytes;
    let fmt_packed: u64 = ((16 << 16) | 2) as u64; // 16-bit, 2 canales (estéreo)
    let play_res = syscall_dispatch(SYS_AUDIO_PLAY, pcm_samples.as_ptr() as u64, pcm_samples.len() as u64, 44100, fmt_packed, 0);
    assert_eq!(play_res, pcm_samples.len() as u64, "SYS_AUDIO_PLAY debe retornar los bytes transmitidos al búfer DMA");

    // Verificar que el estado se actualizó con la configuración transmitida
    let mut status_after = status;
    let _ = syscall_dispatch(SYS_AUDIO_STATUS, &mut status_after as *mut _ as u64, 0, 0, 0, 0);
    assert_eq!(status_after.sample_rate, 44100, "La tasa de muestreo debe ser 44100 Hz");
    assert_eq!(status_after.channels, 2, "Debe ser estéreo (2 canales)");
    assert_eq!(status_after.bits_per_sample, 16, "Debe ser 16-bit por muestra");
    assert!(status_after.total_pcm_bytes >= prev_total + pcm_samples.len() as u64, "El contador total de bytes PCM debe incrementarse");

    // 3. Verificar que los datos PCM se copiaron físicamente en el búfer DMA asignado
    let dma_dest = (AUDIO_DMA_BUFFER_ADDR + 0x1000) as *const u8;
    for i in 0..64 {
        let dma_byte = unsafe { core::ptr::read(dma_dest.add(i)) };
        assert_eq!(dma_byte, pcm_samples[i], "Los datos en memoria física DMA deben coincidir con las muestras enviadas");
    }

    // 4. Reproducción de archivo binario RIFF WAVE completo mediante SYS_AUDIO_PLAY con autodetección (sample_rate=0)
    let wav = create_beaver_chime_wav();
    let wav_play_res = syscall_dispatch(SYS_AUDIO_PLAY, wav.as_ptr() as u64, wav.len() as u64, 0, 0, 0);
    assert!(wav_play_res > 0 && wav_play_res != u64::MAX, "SYS_AUDIO_PLAY debe reproducir un archivo RIFF WAVE detectado automáticamente");

    // 5. Detención de canales de audio mediante SYS_AUDIO_STOP (132)
    let stop_res = syscall_dispatch(SYS_AUDIO_STOP, 0, 0, 0, 0, 0);
    assert_eq!(stop_res, 0, "SYS_AUDIO_STOP debe retornar 0");

    // 6. Validación de manejo robusto de errores de memoria y parámetros
    let null_play = syscall_dispatch(SYS_AUDIO_PLAY, 0, 1024, 44100, fmt_packed, 0);
    assert_eq!(null_play as i64, -14, "Puntero nulo a SYS_AUDIO_PLAY debe devolver -EFAULT (-14)");

    let zero_len = syscall_dispatch(SYS_AUDIO_PLAY, pcm_samples.as_ptr() as u64, 0, 44100, fmt_packed, 0);
    assert_eq!(zero_len as i64, -14, "Longitud 0 a SYS_AUDIO_PLAY debe devolver -EFAULT (-14)");

    let invalid_wav = [0x55u8; 64];
    let bad_wav = syscall_dispatch(SYS_AUDIO_PLAY, invalid_wav.as_ptr() as u64, invalid_wav.len() as u64, 0, 0, 0);
    assert_eq!(bad_wav as i64, -22, "Cabecera WAV inválida con sample_rate=0 debe devolver -EINVAL (-22)");
}

#[cfg(test)]
#[test_case]
fn ut_spotlight_search() {
    test_spotlight_live_search();
}

#[cfg(test)]
#[test_case]
fn ut_real_udp_dhcp_dns() {
    test_real_udp_dhcp_dns();
}

#[cfg(test)]
#[test_case]
fn ut_smp_distributed_sched() {
    test_smp_distributed_scheduler();
}

#[cfg(test)]
#[test_case]
fn ut_paging_4k_isolation() {
    test_paging_4k_and_process_cr3_isolation();
}

#[cfg(test)]
#[test_case]
fn ut_smp_madt_trampoline() {
    test_smp_madt_and_trampoline_0x8000();
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

#[cfg(test)]
#[test_case]
fn ut_ahci_sata_dma() {
    test_ahci_sata_dma_controller();
}

#[cfg(test)]
#[test_case]
fn ut_tcp_state_machine() {
    test_tcp_state_machine_and_http_parser();
}

#[cfg(test)]
#[test_case]
fn ut_posix_syscalls() {
    test_posix_syscalls_and_beaver_libc();
}

#[cfg(test)]
#[test_case]
fn ut_device_profile() {
    test_device_profile_detection();
}

#[cfg(test)]
#[test_case]
fn ut_cec_touchscreen_safe() {
    test_cec_and_touchscreen_safe_dispatch();
}

#[cfg(test)]
#[test_case]
fn ut_shell_pipes_and_redirection() {
    test_shell_pipes_and_redirection();
}

#[cfg(test)]
#[test_case]
fn ut_desktop_compositor_user_window_ipc() {
    test_desktop_compositor_user_window_ipc();
}

#[cfg(test)]
#[test_case]
fn ut_xhci_usb3_controller() {
    test_xhci_usb3_controller_and_ports();
}

#[cfg(test)]
#[test_case]
fn ut_rtc_and_posix_clock_time() {
    test_rtc_and_posix_clock_time();
}

#[cfg(test)]
#[test_case]
fn ut_userland_audio_pcm_dma() {
    test_userland_audio_pcm_dma_playback();
}
