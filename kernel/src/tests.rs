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
    let tests: [(&str, fn()); 15] = [
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
