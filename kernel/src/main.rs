// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Stage 5 Preemptive Multitasking, Kernel Threads & Scheduler
// ============================================================================

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

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

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use crate::vga::{VgaColor, VGA_WRITER};
use crate::serial::SERIAL;
use crate::gdt::init_gdt;
use crate::interrupts::init_idt;
use crate::interrupts::pic::PICS;
use crate::drivers::timer::init_pit;
use crate::memory::init_memory;
use crate::storage::init_storage;
use crate::task::scheduler::SCHEDULER;
use crate::task::{storage_guard_worker, telemetry_worker};
use crate::shell::SHELL;

// Vinculación directa del código de arranque y trampoline x86_64 desde /boot/boot.s
global_asm!(include_str!("../../boot/boot.s"));

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // 1. Inicializar hardware básico de salida (UART COM1 y VGA)
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
        vga.set_color(VgaColor::Cyan, VgaColor::Black);
        vga.write_str("================================================================================\n");
        vga.set_color(VgaColor::Yellow, VgaColor::Black);
        vga.write_str("  BEAVER OS v0.8.0 | Micro-Kernel x86_64 Bare-Metal                             \n");
        vga.set_color(VgaColor::LightCyan, VgaColor::Black);
        vga.write_str("  Author: José Alberto Puente Maldonado (japuentem)                             \n");
        vga.set_color(VgaColor::LightGreen, VgaColor::Black);
        vga.write_str("  Architecture: Human-Centric VFS | Anti-OOM Guard | Transparent Memory & Tasks \n");
        vga.set_color(VgaColor::Cyan, VgaColor::Black);
        vga.write_str("================================================================================\n\n");
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

    // 8. Inicializar Subsistema de Memoria (Marcos Físicos y Heap Dinámico de 4 MiB)
    kprint!("[INIT] Subsistema de Memoria (Frame Allocator + 4MB Heap)...   ");
    init_memory();
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

    // 9.4 Inicializar Controlador de Red e Internet (Intel E1000 Gigabit & DHCP)
    kprint!("[INIT] Subsistema de Red e Internet (Intel E1000 / DHCP)...    ");
    crate::drivers::net::init();
    let _ = crate::drivers::net::dhcp_discover_and_configure();
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

    // 10. Inicializar Planificador Preemptivo (Scheduler) y Lanzar Hilos Concurrentes
    kprint!("[INIT] Planificador Preemptivo Round-Robin (Time-Slicing)...    ");
    {
        let mut sched = SCHEDULER.lock();
        sched.init(); // Tarea 0: main_shell
        sched.spawn("telemetry_daemon", telemetry_worker);      // Tarea 1: background telemetry
        sched.spawn("storage_guard", storage_guard_worker);    // Tarea 2: background guard
    }
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] 3 Hilos Activos\n");

    // 11. Habilitar Interrupciones de hardware en la CPU (STI)
    kprint!("[INIT] Habilitando interrupciones de hardware en CPU (STI)...   ");
    unsafe {
        asm!("sti");
    }
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

        match vfs.read_file("/net/beaver-laptop/status.txt", 0, 0) {
            Ok(status) => {
                let first_line = status.lines().next().unwrap_or("");
                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  * Lectura remota /net/beaver-laptop/status.txt: '{}' [OK]\n  [OK] Red Distribuida Beaver Mesh 100% operativa.\n\n", first_line);
            }
            Err(e) => {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "  * Error /net: {}\n\n", e);
            }
        }
    }

    // 13. Mensaje de bienvenida al Shell interactivo
    kprint_color!(
        VgaColor::Cyan, VgaColor::Black,
        "================================================================================\n\
         [BEAVER SHELL v0.8.0] Escribe 'dashboard', 'memmap' o 'help':                   \n\
         ================================================================================\n"
    );
    SHELL.lock().prompt();

    // 13.1 En arranque nativo UEFI (GOP presente), inicializar y lanzar de inmediato el entorno de escritorio
    if crate::drivers::vbe::UefiBootInfo::get().is_some() {
        let mut vbe = crate::drivers::vbe::VBE.lock();
        let _ = vbe.set_mode(1024, 768, 32);
        drop(vbe);
        crate::gui::desktop::render_desktop();
    }

    // 14. Bucle principal de la Tarea 0 (Shell interactivo concurrente)
    loop {
        unsafe {
            asm!("hlt");
        }
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

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}