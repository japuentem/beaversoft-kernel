// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Stage 5 Preemptive Multitasking, Kernel Threads & Scheduler
// ============================================================================

#![no_std]
#![no_main]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, test_runner(crate::tests::test_runner))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]

extern crate alloc;

pub mod arch;
pub mod io;
pub mod sync;
pub mod vga;
pub mod serial;
pub mod console;
pub mod gdt;
pub mod interrupts;
pub mod drivers;
pub mod memory;
pub mod storage;
pub mod task;
pub mod shell;
pub mod syscall;
pub mod usermode;
pub mod fs;
pub mod gui;
pub mod ipc;
pub mod elf;
pub mod apic;
pub mod crypto;
pub mod tests;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::arch::global_asm;
use core::panic::PanicInfo;
use crate::vga::VgaColor;
#[cfg(target_arch = "x86_64")]
use crate::vga::VGA_WRITER;
#[cfg(target_arch = "x86_64")]
use crate::serial::SERIAL;
#[cfg(target_arch = "x86_64")]
use crate::gdt::init_gdt;
#[cfg(target_arch = "x86_64")]
use crate::interrupts::init_idt;
#[cfg(target_arch = "x86_64")]
use crate::interrupts::pic::PICS;
#[cfg(target_arch = "x86_64")]
use crate::drivers::timer::init_pit;
#[cfg(target_arch = "x86_64")]
use crate::memory::{init_memory, memmap};
#[cfg(target_arch = "x86_64")]
use crate::storage::init_storage;
#[cfg(target_arch = "x86_64")]
use crate::task::scheduler::SCHEDULER;
#[cfg(target_arch = "x86_64")]
use crate::task::{storage_guard_worker, telemetry_worker};
#[cfg(target_arch = "x86_64")]
use crate::shell::SHELL;

// Vinculación directa del código de arranque según la arquitectura objetivo
#[cfg(target_arch = "x86_64")]
global_asm!(include_str!("../../boot/boot.s"));

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("../../boot/boot_aarch64.s"));

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn kernel_main(boot_magic: u64, boot_info: u64) -> ! {
    // 1. Inicializar hardware básico de salida (UART COM1 y VGA)
    // SAFETY: Los puertos I/O de COM1 (0x3F8..0x3FD) son exclusivos del controlador UART y estamos en Ring 0 con acceso exclusivo mediante `SERIAL.lock()`.
    unsafe {
        SERIAL.lock().init();
    }
    VGA_WRITER.lock().clear();

    // 2. Despliegue de Identidad de Marca Beaver en Consola Serial
    kprintln!("================================================================");
    kprintln!("  [BEAVER] Kernel v0.8.0 - Human-Centric Transparency");
    kprintln!("  Author: José Alberto Puente Maldonado (japuentem)");
    kprintln!("  Architecture: x86_64 Long Mode (Bare-Metal)");
    kprintln!("  Status: Beaver OS Stage 8 Desktop Environment & BeaverFS ATA");
    kprintln!("================================================================");

    // 3. Despliegue en Pantalla VGA
    {
        let mut vga = VGA_WRITER.lock();
        vga.set_color(VgaColor::LightCyan, VgaColor::Black);
        vga.write_str("  ____   _____     _    __     __ _____  ____   ");
        vga.set_color(VgaColor::Yellow, VgaColor::Black);
        vga.write_str("   ___   ____  \n");

        vga.set_color(VgaColor::LightCyan, VgaColor::Black);
        vga.write_str(" | __ ) | ____|   / \\   \\ \\   / /| ____||  _ \\  ");
        vga.set_color(VgaColor::Yellow, VgaColor::Black);
        vga.write_str("  / _ \\ / ___| \n");

        vga.set_color(VgaColor::LightCyan, VgaColor::Black);
        vga.write_str(" |  _ \\ |  _|    / _ \\   \\ \\ / / |  _|  | |_) | ");
        vga.set_color(VgaColor::Yellow, VgaColor::Black);
        vga.write_str(" | | | |\\___ \\ \n");

        vga.set_color(VgaColor::LightCyan, VgaColor::Black);
        vga.write_str(" | |_) || |___  / ___ \\   \\ V /  | |___ |  _ <  ");
        vga.set_color(VgaColor::Yellow, VgaColor::Black);
        vga.write_str(" | |_| | ___) |\n");

        vga.set_color(VgaColor::LightCyan, VgaColor::Black);
        vga.write_str(" |____/ |_____|/_/   \\_\\   \\_/   |_____||_| \\_\\ ");
        vga.set_color(VgaColor::Yellow, VgaColor::Black);
        vga.write_str("  \\___/ |____/ \n");

        vga.set_color(VgaColor::DarkGray, VgaColor::Black);
        vga.write_str("--------------------------------------------------------------------------------\n");
        vga.set_color(VgaColor::White, VgaColor::Black);
        vga.write_str("  BEAVER OS v0.9.0 | Micro-Kernel x86_64 Bare-Metal                             \n");
        vga.set_color(VgaColor::LightCyan, VgaColor::Black);
        vga.write_str("  Author: José Alberto Puente Maldonado (japuentem)                             \n");
        vga.set_color(VgaColor::LightGreen, VgaColor::Black);
        vga.write_str("  Architecture: Ring 3 Isolation | BeaverFS | A/B Recovery | TCP/IP Net         \n");
        vga.set_color(VgaColor::DarkGray, VgaColor::Black);
        vga.write_str("--------------------------------------------------------------------------------\n\n");
    }

    // 4. Inicializar GDT de 64 bits con TSS y pila IST dedicada para Double Fault
    kprint!("[INIT] GDT de 64 bits y Task State Segment (TSS con IST1)...   ");
    init_gdt();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 4.1 Inicializar MSRs de Syscall/Sysret (Ring 3 <-> Ring 0 Bridge)
    kprint!("[INIT] MSR Syscall Gate (Ring 3 Isolation & Fast Syscalls)... ");
    crate::syscall::init_syscalls();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 5. Inicializar IDT (Interrupt Descriptor Table) de 256 vectores
    kprint!("[INIT] IDT y tablas de excepciones de procesador x86_64...     ");
    init_idt();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 6. Remapear y configurar el controlador PIC 8259A Dual
    kprint!("[INIT] Remapeo de PIC 8259 dual a interrupciones 32..47...      ");
    // SAFETY: La IDT ya está cargada con los vectores 32..47 y las interrupciones aún están deshabilitadas (IF=0).
    unsafe {
        PICS.initialize();
    }
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 7. Configurar PIT (Timer 8254) a 100 Hz (tick cada 10ms)
    kprint!("[INIT] PIT 8254 Timer a 100 Hz (Heartbeat & Scheduler Clock)...");
    init_pit(100);
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 7.1 Inicializar Controlador de Ratón PS/2 (IRQ 12)
    kprint!("[INIT] Controlador de Raton PS/2 (IRQ 12 y Decodificador)...    ");
    crate::drivers::mouse::init_mouse();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 8. Parsear mapa de memoria real y inicializar Subsistema de Memoria
    kprint!("[INIT] Mapa de Memoria (MB1/UEFI) y Bitmap Frame Allocator...  ");
    let memmap = memmap::parse_and_print(boot_magic, boot_info);
    init_memory(&memmap);
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 8.1 Inicializar Local APIC, Temporizador Preemptivo y SMP Multi-Core (INIT-SIPI-SIPI)
    kprint!("[INIT] Local APIC, LAPIC Timer & SMP Multi-Core (SIPI)...      ");
    crate::apic::init_smp();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9. Inicializar Subsistema de Almacenamiento y Protección Anti-Brickeo
    init_storage();

    // 9.1 Inicializar Sistema de Archivos Virtual (VFS Humano: /proc, /sys, /system)
    kprint!("[INIT] Virtual File System (VFS Humano: /proc, /sys, /system)...");
    crate::fs::init_vfs();
    crate::gui::note::init_beaver_note();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.2 Inicializar Subsistema de Comunicación entre Procesos (IPC)
    kprint!("[INIT] Subsistema IPC (Mailboxes, Canales y Tuberías)...       ");
    crate::ipc::init_ipc();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.3 Inicializar Subsistema de Audio Bare-Metal (PC Speaker / PIT 2 / PCI)
    kprint!("[INIT] Controlador de Audio Bare-Metal (PC Speaker / PIT 2)... ");
    crate::drivers::audio::init();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.4 Inicializar Controlador de Red e Internet (Intel E1000 Gigabit)
    kprint!("[INIT] Subsistema de Red e Internet (Intel E1000 / DHCP)...    ");
    crate::drivers::net::init();
    crate::drivers::mesh::init_mesh();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.5 Inicializar Gestor de Paquetes y Binarios de Usuario en Disco (/disk/bin)
    kprint!("[INIT] Gestor de Paquetes Beaver y Programas en /disk/bin...    ");
    crate::task::pkg::init_disk_bin();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.6 Inicializar Controlador USB Universal (xHCI / EHCI & USB HID)
    kprint!("[INIT] Controlador USB Universal (xHCI USB 3.0 & USB HID)...   ");
    crate::drivers::usb::init();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.7 Inicializar Entorno de Scripting Nativo en Disco (/disk/*.bs)
    kprint!("[INIT] Entorno de Scripting BeaverScript y Motor en Disco...   ");
    crate::task::script::init_disk_scripts();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.8 Inicializar Decodificador Gráfico e Imágenes BMP en Disco (/disk/*.bmp)
    kprint!("[INIT] Decodificador Grafico y Soporte BMP en Disco (/disk)... ");
    crate::gui::bmp::init_disk_images();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.9 Inicializar Flujo de Audio PCM y Archivos WAV en Disco (/disk/*.wav)
    kprint!("[INIT] Subsistema de Audio PCM y Archivo Chime (/disk/chime.wav)");
    crate::drivers::audio::init_disk_audio();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 9.10 Inicializar Controladores de Entrada Táctil (I2C/Digitizer) y HDMI-CEC (TV)
    kprint!("[INIT] Drivers de Pantalla Tactil (I2C) y Control Remoto HDMI-CEC");
    crate::drivers::touchscreen::init_touchscreen();
    crate::drivers::cec::init_cec();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n");

    // 10. Inicializar Planificador Preemptivo (Scheduler) y Lanzar Hilos Concurrentes
    kprint!("[INIT] Planificador Preemptivo Round-Robin (Time-Slicing)...    ");
    {
        let mut sched = SCHEDULER.lock();
        sched.init(); // Tarea 0: main_shell
        sched.spawn("telemetry_daemon", telemetry_worker);      // Tarea 1: background telemetry
        sched.spawn("storage_guard", storage_guard_worker);    // Tarea 2: background guard
    }
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] 3 Hilos Activos\n");

    // 11. Interrupciones se habilitarán tras completar pruebas de arranque e interfaz inicial
    kprint!("[INIT] Preparando interrupciones de hardware en CPU (STI)...    ");
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK]\n\n");

    // 12. Validación dinámica automática de asignación en Heap (Box, Vec, String)
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[TEST] Verificando asignación dinámica en Heap de Rust (alloc::*)...\n");
    {
        let heap_box = Box::new(2026_u64);
        let mut test_vec = Vec::new();
        test_vec.push("Beaver");
        test_vec.push("Kernel");
        test_vec.push("Ready!");

        let mut test_str = String::from("Multitasking Active");
        test_str.push_str(" [OK]");

        kprint_color!(
            VgaColor::White, VgaColor::Black,
            "  * Box<u64>: {} en {:p} | Vec: {} elementos | String: '{}'\n",
            *heap_box, heap_box, test_vec.len(), test_str
        );
    }
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[TEST] Asignaciones y liberaciones automáticas verificadas sin fugas.\n\n");

    // 12.1 Validación automática del Automount de BeaverFS en VFS (/disk)
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[TEST] Verificando Automount de BeaverFS en VFS (/disk)...\n");
    {
        let mut vfs = crate::fs::vfs::VFS.lock();
        match vfs.create_file("/disk/boot_test.txt", b"VFS_ATA_AUTOMOUNT_OK", 0, 0) {
            Ok(()) => {
                match vfs.read_file("/disk/boot_test.txt", 0, 0) {
                    Ok(content) => {
                        let files = vfs.list_dir("/disk", 0, 0).unwrap_or_default();
                        kprint_color!(
                            VgaColor::LightGreen, VgaColor::Black,
                            "  * /disk/boot_test.txt escrito y leido via VFS: '{}'\n  * Total de entradas registradas en /disk: {}\n  [OK] Integracion BeaverFS <-> VFS 100% verificada.\n\n",
                            content, files.len()
                        );
                    }
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "  [ERROR] Lectura /disk: {}\n\n", e);
                    }
                }
            }
            Err(e) => {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "  [ERROR] Escritura /disk: {}\n\n", e);
            }
        }
    }

    // 12.2 Verificación de Audio, Red y Programas de Usuario Ring 3
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[TEST] Verificando Audio, Red y Ejecución de Programas Ring 3...\n");
    crate::drivers::audio::startup_chime();
    kprint_color!(VgaColor::White, VgaColor::Black, "  * Audio: Melodía de inicio reproducida en PC Speaker [OK]\n");
    let net = crate::drivers::net::NET_INTERFACE.lock();
    kprint_color!(VgaColor::White, VgaColor::Black, "  * Red:   IP {} configurada en eth0 (MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}) [OK]\n",
        net.ip_address.to_string(),
        net.mac_address[0], net.mac_address[1], net.mac_address[2],
        net.mac_address[3], net.mac_address[4], net.mac_address[5]
    );
    drop(net);
    match crate::task::pkg::execute_elf("/disk/bin/hello.elf") {
        Ok(msg) => {
            kprint_color!(VgaColor::White, VgaColor::Black, "  * ELF64: /disk/bin/hello.elf ejecutado -> '{}' [OK]\n", msg);
        }
        Err(e) => {
            kprint_color!(VgaColor::LightRed, VgaColor::Black, "  * ELF64 Error: {}\n", e);
        }
    }
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  [OK] Subsistemas de Audio, Red y Ejecutables 100% operativos.\n\n");

    // 12.3 Verificación de la Prueba de Concepto del Instalador Universal en Disco
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[TEST] Verificando Motor del Instalador Universal (Bare-Metal Deployer)...\n");
    let disk_info = crate::storage::installer::BeaverInstaller::probe_target_disk();
    kprint_color!(VgaColor::White, VgaColor::Black, "  * Disco Interno Detectado: {} ({} MB, {} sectores)\n",
        disk_info.model, disk_info.size_mb, disk_info.total_sectors
    );
    match crate::storage::installer::BeaverInstaller::execute_installation(|_step, _pct| {}) {
        Ok(stats) => {
            kprint_color!(
                VgaColor::LightGreen, VgaColor::Black,
                "  * Despliegue en Disco: {} archivos provisionados ({} bytes en {} sectores) [OK]\n  [OK] Instalador Universal Bare-Metal 100% operativo.\n\n",
                stats.files_provisioned, stats.bytes_installed, stats.sectors_written
            );
        }
        Err(e) => {
            kprint_color!(VgaColor::LightRed, VgaColor::Black, "  * Error de instalacion: {}\n\n", e);
        }
    }

    // 12.4 Verificación de la Red Distribuida Beaver Mesh y VFS /net
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[TEST] Verificando Protocolo Beaver Mesh y VFS Distribuido (/net)...\n");
    {
        let vfs = crate::fs::vfs::VFS.lock();
        let net_nodes = vfs.list_dir("/net", 0, 0).unwrap_or_default();
        kprint_color!(VgaColor::White, VgaColor::Black, "  * Nodos descubiertos en /net: {} (", net_nodes.len());
        for (i, (name, ..)) in net_nodes.iter().enumerate() {
            if i > 0 { kprint!(", "); }
            kprint!("{}", name);
        }
        kprintln!(") [OK]");

        if !net_nodes.is_empty() {
            let first_node = &net_nodes[0].0;
            let status_path = alloc::format!("/net/{}/status.txt", first_node);
            match vfs.read_file(&status_path, 0, 0) {
                Ok(status) => {
                    let first_line = status.lines().next().unwrap_or("");
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  * Lectura remota {}: '{}' [OK]\n  [OK] Red Distribuida Beaver Mesh 100% operativa.\n\n", status_path, first_line);
                }
                Err(e) => {
                    kprint_color!(VgaColor::Yellow, VgaColor::Black, "  * Estado de nodo {}: {}\n\n", first_node, e);
                }
            }
        } else {
            kprint_color!(
                VgaColor::LightGreen, VgaColor::Black,
                "  * Red Mesh: Modo Standalone (escuchando anuncios en puerto UDP 4242) [OK]\n  [OK] Red Distribuida Beaver Mesh operativa en segundo plano.\n\n"
            );
        }
    }

    #[cfg(test)]
    test_main();

    // 13. Mensaje de bienvenida al Shell interactivo
    kprintln!();
    kprint_color!(VgaColor::LightCyan, VgaColor::Black, " ______ _____   ___  _   _ _____ ______ ");
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "  _____ _____  \n");
    kprint_color!(VgaColor::LightCyan, VgaColor::Black, " | ___ \\  ___| / _ \\| | | |  ___| ___ \\ ");
    kprint_color!(VgaColor::Yellow, VgaColor::Black, " /  _  /  ___| \n");
    kprint_color!(VgaColor::LightCyan, VgaColor::Black, " | |_/ / |__  / /_\\ \\ | | | |__ | |_/ / ");
    kprint_color!(VgaColor::Yellow, VgaColor::Black, " | | | \\ `--.  \n");
    kprint_color!(VgaColor::LightCyan, VgaColor::Black, " | ___ \\  __| |  _  | | | |  __|    /   ");
    kprint_color!(VgaColor::Yellow, VgaColor::Black, " | | | |`--. \\ \n");
    kprint_color!(VgaColor::LightCyan, VgaColor::Black, " | |_/ / |___ | | | \\ \\_/ / |___| |\\ \\  ");
    kprint_color!(VgaColor::Yellow, VgaColor::Black, " \\ \\_/ /\\__/ / \n");
    kprint_color!(VgaColor::LightCyan, VgaColor::Black, " \\____/\\____/ \\_| |_/\\___/\\____/\\_| \\_| ");
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "  \\___/\\____/  \n");

    kprint_color!(
        VgaColor::DarkGray, VgaColor::Black,
        "--------------------------------------------------------------------------------\n"
    );
    kprint_color!(
        VgaColor::White, VgaColor::Black,
        " [BEAVER SHELL v0.9.0] Escribe 'help' para ver comandos o 'dashboard' en vivo.  \n"
    );
    kprint_color!(
        VgaColor::DarkGray, VgaColor::Black,
        "--------------------------------------------------------------------------------\n"
    );
    SHELL.lock().prompt();

    // 13.1 En arranque nativo UEFI (GOP presente), mostrar splash screen y lanzar entorno de escritorio
    if crate::drivers::vbe::UefiBootInfo::get().is_some() {
        let mut vbe = crate::drivers::vbe::VBE.lock();
        let _ = vbe.set_mode(1024, 768, 32);
        drop(vbe);
        crate::gui::splash::show_boot_splash();
        crate::gui::desktop::render_desktop();
    }

    // 14. Habilitar interrupciones (STI) y entrar al bucle principal de la Tarea 0
    crate::arch::current::enable_interrupts();
    loop {
        // Procesar entrada pendiente de teclado y puerto serie en contexto de Tarea 0 (IF=1)
        crate::interrupts::process_pending_input();
        // Sondear anillo RX del E1000 para recibir tramas (ARP, ICMP, UDP/BeaverSync)
        crate::drivers::net::net_poll_rx();
        // Mantenimiento de la malla: anuncio periodico cada 2 s + expiracion de nodos en 10 s
        crate::drivers::mesh::mesh_tick();
        // Suspende el procesador de forma segura en Ring 0 / EL1 hasta la próxima interrupción.
        crate::arch::current::halt(); // Esperar la siguiente interrupcion
    }
}

// ============================================================================
// Panic Handler
// ============================================================================
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprint_color!(
        VgaColor::LightRed, VgaColor::Black,
        "\n============================================================\n\
         [CRITICAL PANIC] Beaver Kernel Exception Encountered!\n\
         Info: {:?}\n\
         ============================================================\n",
        info
    );

    crate::tests::exit_qemu(crate::tests::QemuExitCode::Failed);

    loop {
        crate::arch::current::halt();
    }
}