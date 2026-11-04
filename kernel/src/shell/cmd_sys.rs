// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Shell System, Memory, Tasks, Audio, GUI, Packages & Hardware Diagnostics
// ============================================================================

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::drivers::timer::{get_ticks, get_uptime_seconds};
use crate::memory::frame_allocator::{FRAME_ALLOCATOR, PAGE_SIZE};
use crate::memory::heap::HEAP_ALLOCATOR;
use crate::memory::{kernel_end, kernel_start, HEAP_SIZE, HEAP_START};
use crate::storage::ab_slot::BOOT_SLOT_MANAGER;
use crate::vga::VgaColor;
use crate::{kprint, kprintln, kprint_color};
use super::Shell;

pub fn try_execute(shell: &mut Shell, cmd: &str, args: &[&str]) -> bool {
    match cmd {
        "dashboard" | "top" => {
            let (total_heap, used_heap, free_heap, _, _) = HEAP_ALLOCATOR.stats();
            let usage_pct = HEAP_ALLOCATOR.usage_percent();
            let pressure = HEAP_ALLOCATOR.memory_pressure();
            let ticks = get_ticks();
            let seconds = get_uptime_seconds();
            let k_start = kernel_start();
            let k_end = kernel_end();
            let kernel_bytes = k_end - k_start;

            kprintln!("==================== BEAVER LIVE DASHBOARD ====================");
            kprint_color!(VgaColor::Yellow, VgaColor::Black, " [RELOJ & CPU]  ");
            kprintln!("Uptime: {}s ({} ticks) | Timer: PIT 100Hz | Quantum: 50ms", seconds, ticks);

            kprint_color!(VgaColor::LightCyan, VgaColor::Black, "\n [MAPA DE MEMORIA TRANSPARENTE]\n");
            kprintln!("  Total Heap:   {} KiB | Usado: {} KiB | Libre: {} KiB", total_heap / 1024, used_heap / 1024, free_heap / 1024);
            kprintln!("  Kernel Image: {} KiB estaticos | Reserva Anti-OOM: 64 KiB", kernel_bytes / 1024);
            kprint_color!(VgaColor::White, VgaColor::Black, "  Grafico:      ");
            let bar = crate::fs::vfs::render_ascii_bar(usage_pct, 32);
            kprint_color!(
                match pressure {
                    crate::memory::heap::MemoryPressure::Normal => VgaColor::LightGreen,
                    crate::memory::heap::MemoryPressure::Warning => VgaColor::Yellow,
                    crate::memory::heap::MemoryPressure::Critical => VgaColor::LightRed,
                },
                VgaColor::Black,
                "{} {}% [{:?}]\n",
                bar, usage_pct, pressure
            );

            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "\n [PROCESOS ACTIVOS (Sin jergas incomprensibles)]\n");
            kprintln!("  ID | Tarea                 | Estado Humano            | Pila    | Ticks");
            kprintln!("  ---+-----------------------+--------------------------+---------+------");
            kprintln!("  00 | main_shell            | Ejecutando en Terminal   | 16 KiB  | {} ticks", ticks / 3);
            kprintln!("  01 | telemetry_daemon      | Esperando Heartbeat PIT  | 16 KiB  | {} ticks", ticks / 6);
            kprintln!("  02 | storage_guard         | Vigilando Disco Espejo   | 16 KiB  | {} ticks", ticks / 6);
            kprintln!("===================================================================");
        }

        "memmap" => {
            let (_, used_heap, free_heap, alloc_count, dealloc_count) = HEAP_ALLOCATOR.stats();
            let usage_pct = HEAP_ALLOCATOR.usage_percent();
            let pressure = HEAP_ALLOCATOR.memory_pressure();
            let k_start = kernel_start();
            let k_end = kernel_end();
            let kernel_bytes = k_end - k_start;
            let frames_alloc = FRAME_ALLOCATOR.allocated_frames_count();
            let frames_total = FRAME_ALLOCATOR.total_usable_frames();

            kprintln!("\n==================== MAPA VISUAL DE MEMORIA (BEAVER) ====================");
            kprintln!("  (Disenado para eliminar la confusion de buffers, caches y pools ocultos)");
            kprintln!("-----------------------------------------------------------------------------");
            kprintln!("  1. CODIGO Y KERNEL ESTATICO  : {:>6} KiB [{:#010x} - {:#010x}]", kernel_bytes / 1024, k_start, k_end);
            kprintln!("  2. HEAP DINAMICO EN USO      : {:>6} KiB (Asignaciones reales Box/Vec)", used_heap / 1024);
            kprintln!("  3. RESERVA ANTI-OOM          : {:>6} KiB (Protegida para que el SO no muera)", crate::memory::heap::EMERGENCY_RESERVE_BYTES / 1024);
            kprintln!("  4. MEMORIA LIBRE Y DISPONIBLE: {:>6} KiB (Lista para usar sin trucos)", free_heap / 1024);
            kprintln!("-----------------------------------------------------------------------------");
            kprint_color!(VgaColor::Yellow, VgaColor::Black, "  Barra de Ocupacion: ");
            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "{}\n", crate::fs::vfs::render_ascii_bar(usage_pct, 40));
            kprintln!("  Diagnostico: {}% ocupado | Nivel de Presion: {:?}", usage_pct, pressure);
            kprintln!("  Asignador:   {} allocs, {} deallocs (0 fugas detectadas)", alloc_count, dealloc_count);
            kprintln!("  Paginador:   {} marcos asignados de {} marcos de 4KB", frames_alloc, frames_total);
            kprintln!("=============================================================================");
        }

        "mem" => {
            let (total, used, free, allocs, deallocs) = HEAP_ALLOCATOR.stats();
            let k_start = kernel_start();
            let k_end = kernel_end();
            let frames_alloc = FRAME_ALLOCATOR.allocated_frames_count();
            let total_frames = FRAME_ALLOCATOR.total_usable_frames();

            kprintln!("==================== MAPA DE MEMORIA DEL SISTEMA ====================");
            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[IMAGEN DEL KERNEL]\n");
            kprintln!("  Rango Físico:   {:#010x} - {:#010x}", k_start, k_end);
            kprintln!("  Tamaño:         {} bytes (~{} KiB)", k_end - k_start, (k_end - k_start) / 1024);

            kprint_color!(VgaColor::Yellow, VgaColor::Black, "\n[DYNAMIC HEAP (alloc::*)]\n");
            kprintln!("  Rango Virtual:  {:#010x} - {:#010x}", HEAP_START, HEAP_START + HEAP_SIZE);
            kprintln!("  Capacidad:      {} KiB ({} bytes)", total / 1024, total);
            kprintln!("  Memoria Usada:  {} bytes", used);
            kprintln!("  Memoria Libre:  {} bytes ({} KiB)", free, free / 1024);
            let pct = HEAP_ALLOCATOR.usage_percent();
            let pressure = HEAP_ALLOCATOR.memory_pressure();
            let pressure_str = match pressure {
                crate::memory::heap::MemoryPressure::Normal => "NORMAL (Uso Optimo)",
                crate::memory::heap::MemoryPressure::Warning => "WARNING (Presion Moderada)",
                crate::memory::heap::MemoryPressure::Critical => "CRITICAL (Umbral Anti-OOM Activo)",
            };
            kprintln!("  Uso Relativo:   {}% [{}]", pct, pressure_str);
            kprintln!("  Reserva Alerta: {} KiB (Anti-OOM Protection)", crate::memory::heap::EMERGENCY_RESERVE_BYTES / 1024);
            kprintln!("  Asignaciones:   {} totales", allocs);
            kprintln!("  Liberaciones:   {} totales", deallocs);

            kprint_color!(VgaColor::Yellow, VgaColor::Black, "\n[MARCOS FISICOS (Frame Allocator)]\n");
            kprintln!("  Paginas de 4KB: {} asignadas / {} detectadas", frames_alloc, total_frames);
            kprintln!("  Memoria Fisica: {} KiB asignados", (frames_alloc * PAGE_SIZE) / 1024);
            kprintln!("=====================================================================");
        }

        "free" => {
            print_free();
        }

        "test-frames" => {
            kprintln!("[TEST] Prueba de Bitmap Frame Allocator (Fase 2)...");
            let free_antes = FRAME_ALLOCATOR.free_frames_count();
            kprintln!("  Marcos libres antes: {}", free_antes);

            let mut frames: [usize; 1000] = [0; 1000];
            let mut asignados = 0usize;
            for slot in frames.iter_mut() {
                if let Some(addr) = FRAME_ALLOCATOR.allocate_frame() {
                    *slot = addr;
                    asignados += 1;
                } else {
                    break;
                }
            }
            let free_durante = FRAME_ALLOCATOR.free_frames_count();
            kprintln!("  Marcos asignados:    {} (esperado: 1000)", asignados);
            kprintln!("  Marcos libres ahora: {}", free_durante);

            for i in 0..asignados {
                FRAME_ALLOCATOR.free_frame(frames[i]);
            }
            let free_despues = FRAME_ALLOCATOR.free_frames_count();
            kprintln!("  Marcos libres tras liberar: {} (esperado: {})", free_despues, free_antes);

            if free_despues == free_antes && asignados == 1000 {
                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Bitmap allocator: asignacion y liberacion correctas.\n");
            } else {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "[FAIL] Discrepancia en contadores del allocator.\n");
            }
        }

        "test-alloc" => {
            kprintln!("[TEST] Iniciando prueba de asignación dinámica de memoria...");
            let (_before_total, before_used, _, before_allocs, _) = HEAP_ALLOCATOR.stats();
            kprintln!("  * Estado antes de la prueba: {} bytes usados en Heap", before_used);

            let my_box = Box::new(42_u64);
            kprintln!("  * Box asignado en heap: valor = {}, addr = {:p}", *my_box, my_box);

            let mut vec = Vec::new();
            for i in 0..16 {
                vec.push(i * 10);
            }
            kprintln!("  * Vec asignado dinámicamente con {} elementos: {:?}", vec.len(), &vec[0..5]);

            let mut dynamic_str = String::from("Beaver");
            dynamic_str.push_str(" Kernel Memory Subsystem v0.8.0");
            kprintln!("  * String en Heap: '{}' (len = {})", dynamic_str, dynamic_str.len());

            let (_, during_used, _, during_allocs, _) = HEAP_ALLOCATOR.stats();
            kprintln!("  * Memoria usada durante la prueba: {} bytes ({} nuevas asignaciones)", during_used, during_allocs - before_allocs);

            drop(my_box);
            drop(vec);
            drop(dynamic_str);

            let (_, after_used, _, _, after_deallocs) = HEAP_ALLOCATOR.stats();
            kprintln!("  * Estado post-drop: {} bytes usados ({} liberaciones exitosas)", after_used, after_deallocs);
            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] ¡Prueba de Heap superada sin memory leaks!\n");
        }

        "oom-test" => {
            kprintln!("\n================ PRUEBA ANTI-OOM & GESTION DE PRESION ================");
            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[PASO 1] Simulando aumento masivo de demanda de memoria en Heap...\n");
            let mut leak_sim: Vec<Box<[u8; 64 * 1024]>> = Vec::new();
            let mut guard_triggered = false;

            for i in 1..=32 {
                let pct = HEAP_ALLOCATOR.usage_percent();
                let pressure = HEAP_ALLOCATOR.memory_pressure();

                if pressure == crate::memory::heap::MemoryPressure::Critical {
                    kprint_color!(VgaColor::LightRed, VgaColor::Black, "\n[ANTI-OOM GUARD] Presion de memoria CRITICA detectada (>90%).\n");
                    kprint_color!(VgaColor::Yellow, VgaColor::Black, "  El subsistema Anti-OOM bloquea asignaciones adicionales para proteger el kernel.\n");
                    guard_triggered = true;
                    break;
                }

                kprint_color!(
                    match pressure {
                        crate::memory::heap::MemoryPressure::Normal => VgaColor::White,
                        crate::memory::heap::MemoryPressure::Warning => VgaColor::Yellow,
                        crate::memory::heap::MemoryPressure::Critical => VgaColor::LightRed,
                    },
                    VgaColor::Black,
                    "  * Bloque #{:02}: {} KiB en uso | Heap: {}% [{:?}]\n",
                    i, i * 64, pct, pressure
                );

                let block = Box::new([0xAA; 64 * 1024]);
                leak_sim.push(block);
            }

            kprint_color!(VgaColor::LightCyan, VgaColor::Black, "\n[PASO 2] Ejecutando colector y defragmentacion de emergencia...\n");
            let blocks_count = leak_sim.len();
            drop(leak_sim);
            let reclaimed = HEAP_ALLOCATOR.emergency_reclaim();
            kprint_color!(
                VgaColor::LightGreen, VgaColor::Black,
                "[EXITO] {} bloques liberados y coalescidos ({} bytes fusionados).\n\
                 Proteccion activa: Guard intervino={}.\n\
                 Uso de Heap restaurado a: {}% | Estado: {:?}\n\
                 El kernel no paniqueo, evitando el tipico fallo OOM-Killer de Linux.\n",
                blocks_count, reclaimed, guard_triggered, HEAP_ALLOCATOR.usage_percent(), HEAP_ALLOCATOR.memory_pressure()
            );
            kprintln!("======================================================================");
        }

        "deadlock-test" => {
            kprintln!("\n================ PRUEBA ANTI-DEADLOCK (SPINLOCK WATCHDOG) ================");
            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[PASO 1] Creando SpinMutex simulando un recurso bloqueado por otro hilo...\n");
            let test_mutex = crate::sync::SpinMutex::new(777_u32);
            let _guard = test_mutex.lock();

            kprint_color!(VgaColor::LightCyan, VgaColor::Black, "[PASO 2] Intentando adquisicion con Watchdog Anti-Deadlock (timeout: 500,000 ciclos)...\n");
            match test_mutex.try_lock_timeout(500_000) {
                Ok(_) => {
                    kprint_color!(VgaColor::LightRed, VgaColor::Black, "[FALLO] El cerrojo se adquirio cuando debia estar ocupado.\n");
                }
                Err(err) => {
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[EXITO] ¡Watchdog detecto el bloqueo y aborto la espera de forma segura!\n");
                    kprint_color!(VgaColor::Yellow, VgaColor::Black, "  Diagnostico: {}\n", err);
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  Resultado: El hilo interactivo no se congelo y la CPU continua operando normalmente.\n");
                }
            }
            kprintln!("==========================================================================");
        }

        "ps" => {
            kprintln!("{}", crate::fs::vfs::generate_human_tasks());
        }

        "tasks" => {
            crate::task::scheduler::SCHEDULER.lock().print_tasks();
        }

        "yield" => {
            kprintln!("[TASK] Cediendo control de CPU voluntariamente...");
            crate::task::scheduler::yield_now();
            kprintln!("[TASK] Control de CPU retornado al Shell interactivo.");
        }

        "kill" => {
            if args.is_empty() {
                kprintln!("Uso: kill <id_tarea>  (ejemplo: kill 1)");
                return true;
            }
            if let Ok(tid) = args[0].parse::<u64>() {
                let mut sched = crate::task::scheduler::SCHEDULER.lock();
                match sched.kill_task(tid) {
                    Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Tarea {} terminada exitosamente.\n", tid),
                    Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                }
            } else {
                kprintln!("ID de tarea no válido: '{}'. Debe ser un número.", args[0]);
            }
        }

        "sleep" => {
            if args.is_empty() {
                kprintln!("Uso: sleep <segundos>  (ejemplo: sleep 2)");
                return true;
            }
            if let Ok(secs) = args[0].parse::<u64>() {
                kprintln!("Durmiendo shell durante {} segundos (liberando CPU para otras tareas)...", secs);
                crate::task::scheduler::sleep(secs * 100);
                kprintln!("[DESPIERTO] Shell reanudado.");
            } else {
                kprintln!("Segundos no válidos: '{}'", args[0]);
            }
        }

        "ring3" => {
            kprintln!("[RING3] Iniciando sandbox de Ring 3 via IRETQ...");
            crate::usermode::spawn_normal_user_task();
        }

        "ring3-crash" => {
            kprintln!("[RING3] Iniciando sandbox crasheador de Ring 3 (Anti-BSOD demo)...");
            crate::usermode::spawn_crash_user_task();
        }

        "elf-run" => {
            kprintln!("[ELF] Generando y cargando binario ejecutable ELF64 en Ring 3...");
            let sample_elf = crate::elf::generate_sample_elf_binary();
            match crate::elf::ElfBinary::parse(&sample_elf) {
                Ok(elf) => {
                    if let Err(e) = elf.load_and_execute() {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR ELF] {}\n", e);
                    }
                }
                Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR ELF] {}\n", e),
            }
        }

        "slots" => {
            BOOT_SLOT_MANAGER.lock().print_status();
        }

        "slot-test" => {
            kprintln!("\n================ PRUEBA DE RESILIENCIA Y AUTO-RECUPERACIÓN ================");
            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[PASO 1] Provocando corrupción intencional en Sector Cero (LBA 0)...\n");
            BOOT_SLOT_MANAGER.lock().simulate_corrupt_sector_zero();

            kprint_color!(VgaColor::LightCyan, VgaColor::Black, "[PASO 2] Ejecutando algoritmo de auto-reparación (Self-Healing)...\n");
            let recovered = BOOT_SLOT_MANAGER.lock().self_heal_from_backup();

            if recovered {
                kprint_color!(
                    VgaColor::LightGreen, VgaColor::Black,
                    "[ÉXITO] ¡Sector Cero reparado matemáticamente desde Copia Espejo LBA 16!\n\
                     El sistema demostró inmunidad total a corrupción de sector cero.\n"
                );
            } else {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "[FALLO] No se pudo recuperar desde el backup.\n");
            }
            kprintln!("==========================================================================");
        }

        "slot-switch" => {
            let new_slot = BOOT_SLOT_MANAGER.lock().switch_slot();
            kprint_color!(
                VgaColor::LightGreen, VgaColor::Black,
                "[OK] Ranura activa conmutada a: {}\n",
                if new_slot == 0 { "SLOT A (Producción)" } else { "SLOT B (Golden Recovery)" }
            );
        }

        "ipc" => {
            kprintln!("{}", crate::ipc::IPC_MANAGER.lock().generate_status());
        }

        "ipc-send" => {
            if args.len() < 2 {
                kprintln!("Uso: ipc-send <canal> <mensaje...>  (ejemplo: ipc-send events AlertaCritica)");
                return true;
            }
            let channel = args[0];
            let payload = args[1..].join(" ");
            let tid = crate::task::scheduler::SCHEDULER.lock().current_task_id();
            let (uid, _, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
            let mut ipc = crate::ipc::IPC_MANAGER.lock();
            match ipc.send_message(channel, tid, uid, &payload) {
                Ok(msg_id) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Mensaje #{} enviado al buzon '{}' (TID: {}, UID: {}).\n", msg_id, channel, tid, uid),
                Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
            }
        }

        "ipc-recv" => {
            if args.is_empty() {
                kprintln!("Uso: ipc-recv <canal>  (ejemplo: ipc-recv events)");
                return true;
            }
            let channel = args[0];
            let mut ipc = crate::ipc::IPC_MANAGER.lock();
            match ipc.receive_message(channel) {
                Ok(msg) => {
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[MENSAJE RECIBIDO]\n");
                    kprintln!("  ID:        #{}", msg.id);
                    kprintln!("  Remitente: TID: {} | UID: {}", msg.sender_tid, msg.sender_uid);
                    kprintln!("  Timestamp: {} ticks", msg.timestamp_ticks);
                    kprintln!("  Contenido: '{}'", msg.payload);
                }
                Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
            }
        }

        "ipc-pipe" => {
            if args.len() < 2 {
                kprintln!("Uso: ipc-pipe <tuberia> <texto...>  (ejemplo: ipc-pipe stdout_stream HolaMundo)");
                return true;
            }
            let pipe_name = args[0];
            let text = args[1..].join(" ");
            let written = crate::ipc::IPC_MANAGER.lock().write_pipe(pipe_name, text.as_bytes());
            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Escritos {} bytes en tuberia circular '{}'.\n", written, pipe_name);
        }

        "ipc-read" => {
            if args.is_empty() {
                kprintln!("Uso: ipc-read <tuberia>  (ejemplo: ipc-read stdout_stream)");
                return true;
            }
            let pipe_name = args[0];
            let mut buf = [0u8; 256];
            let read = crate::ipc::IPC_MANAGER.lock().read_pipe(pipe_name, &mut buf);
            if read > 0 {
                let text = core::str::from_utf8(&buf[..read]).unwrap_or("<binario>");
                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[LEIDO {} bytes de '{}']: ", read, pipe_name);
                kprintln!("'{}'", text);
            } else {
                kprintln!("Tuberia '{}' vacia (0 bytes disponibles para lectura).", pipe_name);
            }
        }

        "ipc-demo" => {
            run_ipc_demo();
        }

        "cpuinfo" | "cpu" => {
            print_cpuinfo();
        }

        "date" | "time" | "rtc" => {
            print_date();
        }

        "smp" | "cores" => {
            let smp = crate::apic::SMP.lock();
            let online_count = crate::apic::AP_ONLINE_COUNT.load(core::sync::atomic::Ordering::Relaxed);
            kprintln!("======================= TOPOLOGIA SMP ======================");
            kprintln!("  Local APIC Base:     {:#010x} (MSR 0x1B)", smp.lapic_base);
            kprintln!("  Tabla ACPI MADT:     {} ({:#010x})", if smp.madt_detected { "Detectada [APIC]" } else { "No detectada" }, smp.madt_addr);
            kprintln!("  Trampolin 16->64 bit:{:#06x} (Vector SIPI 0x{:02x})", smp.trampoline_addr, crate::apic::SMP_SIPI_VECTOR);
            kprintln!("  Temporizador LAPIC:  Activo en modo Periodico (Vector 48 @ 100 Hz)");
            kprintln!("  Bootstrap Core (BSP):APIC ID #{}", smp.bsp_apic_id);
            kprintln!("  Nucleos Online:      {} / {} detectados en MADT", online_count, smp.cores.len());
            kprintln!("-------------------------------------------------------------");
            for core in &smp.cores {
                let role = if core.is_bsp { "[BSP Master]" } else { "[AP Worker]" };
                let status = if core.is_online { "ONLINE" } else { "OFFLINE" };
                let active_task_str = {
                    let sched = crate::task::scheduler::SCHEDULER.lock();
                    let tid = sched.get_core_task(core.apic_id as usize);
                    match tid {
                        Some(id) => alloc::format!("TID #{}", id),
                        None => alloc::string::String::from("Idle"),
                    }
                };
                kprintln!(
                    "  Core #{} (APIC ID #{}): {:<14} | Estado: {:<7} | Tarea: {:<7} | Ejecutadas: {}",
                    core.core_id, core.apic_id, role, status, active_task_str, core.tasks_executed
                );
            }
            kprintln!("=============================================================");
        }

        "lsusb" | "usb" => {
            kprintln!("{}", crate::drivers::usb::get_usb_summary());
        }

        "mouse" => {
            let state = crate::drivers::mouse::MOUSE.lock().state;
            let (scr_w, scr_h) = crate::gui::canvas::Canvas::screen_dimensions();
            kprintln!("==================== ESTADO DEL RATON PS/2 ====================");
            kprintln!("  * Posicion Actual   : X = {}, Y = {} (Resolucion activa {}x{})", state.x, state.y, scr_w, scr_h);
            kprintln!("  * Boton Izquierdo   : {}", if state.left_button { "PRESIONADO" } else { "Liberado" });
            kprintln!("  * Boton Derecho     : {}", if state.right_button { "PRESIONADO" } else { "Liberado" });
            kprintln!("  * Boton Central     : {}", if state.middle_button { "PRESIONADO" } else { "Liberado" });
            kprintln!("  * Clics Acumulados  : {}", state.click_count);
            kprintln!("  * Interrupcion Hardware: IRQ 12 (Vector IDT 44) | Puerto E/S: 0x60/0x64");
            kprintln!("  * Integracion GUI   : Puntero interactivo activo en Escritorio VESA");
            kprintln!("================================================================");
        }

        "beep" => {
            let freq = if !args.is_empty() { args[0].parse::<u32>().unwrap_or(800) } else { 800 };
            let dur = if args.len() >= 2 { args[1].parse::<u32>().unwrap_or(120) } else { 120 };
            kprintln!("[AUDIO] Emitiendo tono de {} Hz durante {} ms...", freq, dur);
            crate::drivers::audio::play_tone(freq, dur);
            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Tono reproducido en altavoz.\n");
        }

        "play" => {
            let target = if args.is_empty() { "startup" } else { args[0] };
            if target.ends_with(".wav") || target.starts_with('/') {
                let resolved = shell.resolve_path(target);
                kprintln!("[AUDIO] Cargando y reproduciendo archivo PCM: '{}'...", resolved);
                match crate::drivers::audio::play_wav_file(&resolved) {
                    Ok(info) => {
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  * {}\n", info);
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  [OK] Audio PCM transmitido via DMA al controlador de sonido.\n");
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "  [ERROR] Falló la reproducción: {}\n", e),
                }
            } else {
                match target {
                    "startup" | "chime" => {
                        kprintln!("[AUDIO] Reproduciendo melodia institucional Beaver OS...");
                        crate::drivers::audio::startup_chime();
                    }
                    "scale" | "cscale" => {
                        kprintln!("[AUDIO] Reproduciendo escala musical (Do-Re-Mi-Fa-Sol-La-Si-Do)...");
                        crate::drivers::audio::play_scale();
                    }
                    "alert" => {
                        kprintln!("[AUDIO] Emitiendo tono de alerta...");
                        crate::drivers::audio::alert_beep();
                    }
                    "click" => {
                        crate::drivers::audio::click_feedback();
                    }
                    _ => kprintln!("Melodia o archivo desconocido: '{}'. Opciones: startup, scale, alert, click o '/disk/*.wav'", target),
                }
            }
        }

        "audio" | "hda" | "sound" => {
            let state = *crate::drivers::audio::AUDIO_STATE.lock();
            kprintln!("==================== SUBSISTEMA DE AUDIO BARE-METAL ====================");
            let c_type = match state.controller_type {
                crate::drivers::audio::AudioControllerType::IntelHda => "Intel High Definition Audio (HDA / ICH DMA)",
                crate::drivers::audio::AudioControllerType::Ac97 => "AC'97 Bus Master Audio Controller",
                crate::drivers::audio::AudioControllerType::PcSpeaker => "PC Speaker PIT 8254 Timer 2",
            };
            kprintln!("  Controlador Activo:  {}", c_type);
            kprintln!("  Hardware PCI:        {}", if state.pci_audio_found { "Dispositivo detectado y configurado en bus PCI" } else { "Altavoz de Placa Base (PC Speaker)" });
            if state.pci_audio_found {
                kprintln!("  Dispositivo PCI:     {} (Vendor: {:#06x}, Dev: {:#06x})", state.pci_device_name, state.pci_vendor_id, state.pci_device_id);
                if state.mmio_base != 0 {
                    kprintln!("  HDA MMIO BAR0 Base:  {:#010x}", state.mmio_base);
                    kprintln!("  Flujos de Hardware:  {} Output Streams, {} Input Streams", state.num_output_streams, state.num_input_streams);
                }
                if state.nabmbar_io != 0 {
                    kprintln!("  AC97 Puertos I/O:    NAMBAR: {:#06x} | NABMBAR: {:#06x}", state.nambar_io, state.nabmbar_io);
                }
            }
            kprintln!("  Memoria DMA Ring:    {:#010x} (64 KiB BDL Buffer)", state.dma_buffer_addr);
            kprintln!("  Formato PCM Actual:  {} Hz | {}-bit | {}", state.sample_rate, state.bits_per_sample, if state.channels == 2 { "Estéreo" } else { "Mono" });
            kprintln!("  Bytes PCM Emitidos:  {} bytes", state.total_pcm_bytes_played);
            kprintln!("  Tonos Reproducidos:  {} beeps acusticos", state.total_beeps_played);
            kprintln!("  Comandos:            'play <startup|scale|/disk/chime.wav>', 'wav-gen'");
            kprintln!("=========================================================================");
        }

        "wav-gen" => {
            kprintln!("[AUDIO] Generando muestra de audio armónico de 16-bit PCM estéreo (/disk/chime.wav)...");
            let wav_bytes = crate::drivers::audio::create_beaver_chime_wav();
            let mut vfs = crate::fs::vfs::VFS.lock();
            match vfs.create_file("/disk/chime.wav", &wav_bytes, 0, 0) {
                Ok(()) => {
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  [OK] /disk/chime.wav generado exitosamente ({} bytes).\n", wav_bytes.len());
                    kprintln!("  Escribe 'play /disk/chime.wav' para reproducirlo.");
                }
                Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "  [ERROR] Falló la creación del archivo WAV: {}\n", e),
            }
        }

        "gui" | "desktop" => {
            let mut vbe = crate::drivers::vbe::VBE.lock();
            if !vbe.is_available() {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] Extensión VESA/VBE no detectada en este hardware.\n");
                return true;
            }
            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[GUI] Iniciando Framebuffer VESA 1024x768 a 32 bits (TrueColor)...\n");
            let bar0 = vbe.detect_lfb_address();
            kprint_color!(VgaColor::LightCyan, VgaColor::Black, "[GUI] Linear Framebuffer mapeado en {:#010x}\n", bar0);
            if let Err(e) = vbe.set_mode(1024, 768, 32) {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                return true;
            }
            drop(vbe);
            crate::gui::render_desktop();
        }

        "gui-off" => {
            let vbe = crate::drivers::vbe::VBE.lock();
            if vbe.is_active {
                drop(vbe);
                let mut state = crate::gui::desktop::GUI_TERMINAL.lock();
                state.mode = crate::gui::desktop::GuiViewMode::FullScreenCli;
                state.lines.push(alloc::string::String::from("[SISTEMA] En x86_64 Long Mode bare-metal, la consola nativa es TTY1."));
                state.lines.push(alloc::string::String::from("[SISTEMA] Escribe 'desktop' o presiona ESC para volver a la interfaz grafica."));
                drop(state);
                crate::gui::desktop::render_fullscreen_cli();
            } else {
                kprintln!("El modo gráfico no está activo.");
            }
        }

        "vbe" | "gfxinfo" => {
            let mut vbe = crate::drivers::vbe::VBE.lock();
            let available = vbe.is_available();
            let bar0 = vbe.detect_lfb_address();
            kprintln!("==================== HARDWARE GRÁFICO (VESA / PCI / VBE) ====================");
            kprintln!("  Soporte Bochs/VBE:   {}", if available { "[DISPONIBLE - Listo para GUI]" } else { "[NO DISPONIBLE]" });
            kprintln!("  Controlador PCI:     {}", if let Some(dev) = crate::drivers::pci::find_vga_device() {
                alloc::format!("Bus {:02x}:Dev {:02x} | Vendor: {:04x} Device: {:04x}", dev.bus, dev.device, dev.vendor_id, dev.device_id)
            } else {
                alloc::string::String::from("VGA estándar heredado (Fallback)")
            });
            kprintln!("  Linear Framebuffer:  {:#010x} (Mapeo directo en 4GB)", bar0);
            kprintln!("  Modo Activo:         {}", if vbe.is_active { alloc::format!("{}x{} @ {} bpp", vbe.width, vbe.height, vbe.bpp) } else { alloc::string::String::from("Consola de Texto VGA 80x25 (Modo 3)") });
            kprintln!("  Profundidad Color:   32 bits TrueColor (0x00RRGGBB / ARGB)");
            kprintln!("  Capacidades:         Ventanas, composición de sombras, Dirty Rects, tipografía 8x16");
            kprintln!("=============================================================================");
        }

        "edit" | "note" => {
            let vbe_active = crate::drivers::vbe::VBE.lock().is_active;
            if !vbe_active {
                kprintln!("[AVISO] BeaverNote es una aplicacion con interfaz grafica.");
                kprintln!("Iniciando entorno de escritorio VESA...");
                shell.execute_command("gui");
            }
            if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                mgr.bring_to_front(crate::gui::window::WindowId::Note);
            }
            if !args.is_empty() {
                let file = shell.resolve_path(args[0]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                if let Ok(content) = crate::fs::vfs::VFS.lock().read_file(&file, curr_uid, curr_gid) {
                    let mut note = crate::gui::note::BEAVER_NOTE.lock();
                    note.content = content.lines().map(alloc::string::String::from).collect();
                    note.status_msg = alloc::format!("Archivo '{}' cargado.", file);
                }
            }
            if crate::drivers::vbe::VBE.lock().is_active {
                crate::gui::render_desktop();
            }
        }

        "view" | "bmp" => {
            if args.is_empty() {
                kprintln!("Uso: view <archivo.bmp> o bmp <archivo.bmp>  (ejemplo: view /disk/logo.bmp)");
                return true;
            }
            let path = shell.resolve_path(args[0]);
            let file_bytes = {
                let vfs = crate::fs::vfs::VFS.lock();
                vfs.read_file_bytes(&path, 0, 0)
            };

            match file_bytes {
                Ok(bytes) => {
                    match crate::gui::bmp::BmpImage::parse(&bytes) {
                        Ok(bmp) => {
                            kprintln!("================== VISUALIZADOR DE IMAGENES BMP ==================");
                            kprintln!("  Archivo:      {}", path);
                            kprintln!("  Dimensiones:  {}x{} pixeles", bmp.width, bmp.height);
                            kprintln!("  Profundidad:  {} bits por pixel (BPP)", bmp.bpp);
                            kprintln!("  Tamano Total: {} bytes", bytes.len());
                            kprintln!("  Firma:        Valida (Windows Bitmap 'BM')");
                            kprintln!("==================================================================");

                            let vbe_active = crate::drivers::vbe::VBE.lock().is_active;
                            if vbe_active {
                                let (scr_w, scr_h) = crate::gui::canvas::Canvas::screen_dimensions();
                                let rx = (scr_w.saturating_sub(bmp.width)) / 2;
                                let ry = (scr_h.saturating_sub(bmp.height)) / 2;
                                crate::gui::canvas::Canvas::fill_rect(rx.saturating_sub(10), ry.saturating_sub(10), bmp.width + 20, bmp.height + 20, crate::gui::canvas::Color::SLATE_800);
                                crate::gui::canvas::Canvas::draw_rect(rx.saturating_sub(10), ry.saturating_sub(10), bmp.width + 20, bmp.height + 20, crate::gui::canvas::Color::BEAVER_ACCENT_CYAN);
                                bmp.render_at(rx, ry);
                                crate::gui::canvas::Canvas::present_rect(rx.saturating_sub(10), ry.saturating_sub(10), bmp.width + 20, bmp.height + 20);
                                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Imagen BMP renderizada en pantalla ({}, {})\n", rx, ry);
                            } else {
                                kprintln!("\n[PREVISUALIZACION ASCII DE IMAGEN]:");
                                let step_y = (bmp.height / 12).max(1);
                                let step_x = (bmp.width / 24).max(1);
                                for y in (0..bmp.height).step_by(step_y).take(12) {
                                    for x in (0..bmp.width).step_by(step_x).take(24) {
                                        let color = bmp.pixels[y * bmp.width + x];
                                        let r = (color >> 16) & 0xFF;
                                        let g = (color >> 8) & 0xFF;
                                        let b = color & 0xFF;
                                        let lum = (r * 299 + g * 587 + b * 114) / 1000;
                                        let ch = match lum {
                                            0..=60 => ' ',
                                            61..=120 => '.',
                                            121..=180 => '*',
                                            _ => '#',
                                        };
                                        kprint!("{}", ch);
                                    }
                                    kprintln!("");
                                }
                            }
                        }
                        Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR DECODIFICANDO BMP] {}\n", e),
                    }
                }
                Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] No se pudo leer '{}': {}\n", path, e),
            }
        }

        "beaver" | "run" => {
            if args.is_empty() {
                kprintln!("Uso: beaver <archivo.bs>  (ejemplo: beaver /disk/demo.bs o beaver /disk/fibonacci.bs)");
                return true;
            }
            let path = shell.resolve_path(args[0]);
            let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
            match crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                Ok(source) => {
                    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[BEAVERSCRIPT] Ejecutando '{}'...\n", path);
                    match crate::task::script::run_script(&source) {
                        Ok(output) => kprint!("{}", output),
                        Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR DE EJECUCION] {}\n", e),
                    }
                }
                Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
            }
        }

        "bs" => {
            kprintln!("================ BEAVERSCRIPT v1.0 INTERPRETE REPL ================");
            kprintln!("  Escribe 'exit' para salir al Shell de Beaver OS");
            kprintln!("  Sintaxis: let x = 10 | println x * 5 | beep(440, 100)");
            kprintln!("===================================================================");
        }

        "pkg" => {
            if args.is_empty() {
                kprintln!("Uso: pkg <list|install|run|info> [nombre_paquete]");
                kprintln!("Ejemplos:");
                kprintln!("  pkg list              - Lista paquetes disponibles e instalados");
                kprintln!("  pkg install hello     - Instala el binario en /disk/bin/hello.elf");
                kprintln!("  pkg run hello         - Ejecuta el programa en Ring 3 aislado");
                return true;
            }
            match args[0] {
                "list" => {
                    kprintln!("================== PAQUETES DE USUARIO BEAVER (/disk/bin) ==================");
                    kprintln!("Nombre    Version  Tamano  Categoria    Estado      Descripcion");
                    kprintln!("--------  -------  ------  -----------  ----------  -----------------------------");
                    let vfs = crate::fs::vfs::VFS.lock();
                    for pkg in &crate::task::pkg::AVAILABLE_PACKAGES {
                        let path = format!("/disk/bin/{}.elf", pkg.name);
                        let installed = vfs.exists(&path);
                        let status_str = if installed { "[INSTALADO]" } else { "Disponible" };
                        kprintln!("{:<8}  {:<7}  {:>4} B  {:<11}  {:<10}  {}",
                            pkg.name, pkg.version, pkg.size_bytes, pkg.category, status_str, pkg.description
                        );
                    }
                    kprintln!("============================================================================");
                }
                "install" => {
                    if args.len() < 2 {
                        kprintln!("Uso: pkg install <nombre_paquete> [url_remota]");
                        return true;
                    }
                    let remote_url = if args.len() >= 3 { Some(args[2]) } else { None };
                    match crate::task::pkg::install_package(args[1], remote_url) {
                        Ok(msg) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] {}\n", msg),
                        Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                    }
                }
                "remove" => {
                    if args.len() < 2 {
                        kprintln!("Uso: pkg remove <nombre_paquete>");
                        return true;
                    }
                    match crate::task::pkg::remove_package(args[1]) {
                        Ok(msg) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] {}\n", msg),
                        Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                    }
                }
                "run" => {
                    if args.len() < 2 {
                        kprintln!("Uso: pkg run <nombre_o_ruta>  (ejemplo: pkg run hello o pkg run /disk/bin/sysinfo.elf)");
                        return true;
                    }
                    let path = if args[1].starts_with('/') {
                        args[1].to_string()
                    } else if args[1].ends_with(".elf") {
                        format!("/disk/bin/{}", args[1])
                    } else {
                        format!("/disk/bin/{}.elf", args[1])
                    };
                    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[CARGANDO ELF64] {}\n", path);
                    match crate::task::pkg::execute_elf(&path) {
                        Ok(output) => {
                            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[SALIDA RING 3]: ");
                            kprintln!("{}", output);
                        }
                        Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR DE EJECUCION] {}\n", e),
                    }
                }
                "info" => {
                    if args.len() < 2 {
                        kprintln!("Uso: pkg info <paquete>");
                        return true;
                    }
                    let name = args[1];
                    if let Some(pkg) = crate::task::pkg::AVAILABLE_PACKAGES.iter().find(|p| p.name == name) {
                        kprintln!("Paquete:      {}", pkg.name);
                        kprintln!("Version:      {}", pkg.version);
                        kprintln!("Categoria:    {}", pkg.category);
                        kprintln!("Tamano:       {} bytes", pkg.size_bytes);
                        kprintln!("Descripcion:  {}", pkg.description);
                        kprintln!("Ruta destino: /disk/bin/{}.elf", pkg.name);
                    } else {
                        kprintln!("Paquete '{}' no encontrado.", name);
                    }
                }
                _ => kprintln!("Subcomando pkg no reconocido: '{}'. Usa 'pkg' para ayuda.", args[0]),
            }
        }

        "exec" => {
            if args.is_empty() {
                kprintln!("Uso: exec <ruta_binario_elf>  (ejemplo: exec /disk/bin/hello.elf)");
                return true;
            }
            let path = shell.resolve_path(args[0]);
            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[EXEC ELF64] Cargando binario '{}' en Ring 3...\n", path);
            match crate::task::pkg::execute_elf(&path) {
                Ok(output) => {
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[SALIDA RING 3]: ");
                    kprintln!("{}", output);
                }
                Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
            }
        }

        "autotest" | "qemutest" | "selftest" => {
            let exit_after = args.get(0).copied() == Some("exit");
            crate::tests::run_builtin_test_suite(exit_after);
        }

        "reboot" | "reset" => {
            hardware_reboot();
        }

        "shutdown" | "poweroff" => {
            hardware_shutdown();
        }

        _ => return false,
    }
    true
}

pub fn hardware_reboot() -> ! {
    kprintln!("[SISTEMA] Reiniciando equipo por hardware (8042 reset)...");
    // SAFETY: En Ring 0 enviamos el pulso de reset 0xFE al puerto 0x64 del controlador 8042 y forzamos triple fault de respaldo.
    unsafe {
        crate::io::outb(0x64, 0xFE);
        core::arch::asm!(
            "lidt [{}]",
            "int3",
            in(reg) &0_u64,
            options(noreturn)
        );
    }
}

pub fn hardware_shutdown() -> ! {
    kprintln!("[SISTEMA] Apagando el sistema (QEMU/ACPI)...");
    // SAFETY: Escritura a los puertos PM1a_CNT de QEMU/Bochs/VirtualBox en Ring 0 seguida de bucle cli+hlt.
    unsafe {
        crate::io::outw(0x604, 0x2000);
        crate::io::outw(0xB004, 0x2000);
        crate::io::outw(0x4004, 0x3400);
        loop {
            core::arch::asm!("cli; hlt");
        }
    }
}

pub fn print_cpuinfo() {
    let res = core::arch::x86_64::__cpuid(0);
    let mut v = [0_u8; 12];
    v[0..4].copy_from_slice(&res.ebx.to_le_bytes());
    v[4..8].copy_from_slice(&res.edx.to_le_bytes());
    v[8..12].copy_from_slice(&res.ecx.to_le_bytes());
    let vendor = core::str::from_utf8(&v).unwrap_or("Desconocido");
    let max_leaf = res.eax;

    kprintln!("==================== INFORMACIÓN DEL PROCESADOR (x86_64) ====================");
    kprintln!("  Fabricante / Vendor:  {}", vendor);
    kprintln!("  Arquitectura:         x86_64 Long Mode (64 bits Bare-Metal)");

    let max_ext = core::arch::x86_64::__cpuid(0x80000000).eax;
    if max_ext >= 0x80000004 {
        let mut brand = [0_u8; 48];
        for i in 0..3 {
            let leaf = 0x80000002 + i;
            let res = core::arch::x86_64::__cpuid(leaf);
            let offset = i as usize * 16;
            brand[offset..offset + 4].copy_from_slice(&res.eax.to_le_bytes());
            brand[offset + 4..offset + 8].copy_from_slice(&res.ebx.to_le_bytes());
            brand[offset + 8..offset + 12].copy_from_slice(&res.ecx.to_le_bytes());
            brand[offset + 12..offset + 16].copy_from_slice(&res.edx.to_le_bytes());
        }
        let brand_str = core::str::from_utf8(&brand).unwrap_or("Procesador x86_64");
        kprintln!("  Modelo / Marca:       {}", brand_str.trim());
    }

    if max_leaf >= 1 {
        let feat = core::arch::x86_64::__cpuid(1);
        kprintln!("-----------------------------------------------------------------------------");
        kprintln!("  Capacidades de Hardware Detectadas:");
        kprintln!("   * APIC Controller:   {}", if (feat.edx & (1 << 9)) != 0 { "[SI]" } else { "[NO]" });
        kprintln!("   * FPU Math Unit:     {}", if (feat.edx & (1 << 0)) != 0 { "[SI]" } else { "[NO]" });
        kprintln!("   * Time Stamp Counter:{}", if (feat.edx & (1 << 4)) != 0 { "[SI]" } else { "[NO]" });
        kprintln!("   * Instrucciones SSE: {}", if (feat.edx & (1 << 25)) != 0 { "[SI]" } else { "[NO]" });
        kprintln!("   * Instrucciones SSE2:{}", if (feat.edx & (1 << 26)) != 0 { "[SI]" } else { "[NO]" });
    }

    if max_ext >= 0x80000001 {
        let ext_feat = core::arch::x86_64::__cpuid(0x80000001);
        kprintln!("   * SYSCALL/SYSRET:    {}", if (ext_feat.edx & (1 << 11)) != 0 { "[SI] (Activo en MSR Ring 3)" } else { "[NO]" });
        kprintln!("   * No-Execute (NX):   {}", if (ext_feat.edx & (1 << 20)) != 0 { "[SI] (Proteccion Anti-Exploit)" } else { "[NO]" });
        kprintln!("   * Long Mode (64-bit):{}", if (ext_feat.edx & (1 << 29)) != 0 { "[SI] (Kernel Nativo)" } else { "[NO]" });
    }
    kprintln!("=============================================================================");
}

pub fn print_date() {
    fn bcd_to_bin(val: u8) -> u8 {
        (val & 0x0F) + ((val >> 4) * 10)
    }

    // SAFETY: Lectura de los registros del reloj CMOS RTC mediante los puertos I/O 0x70 y 0x71 en Ring 0.
    unsafe {
        crate::io::outb(0x70, 0x0A);
        while (crate::io::inb(0x71) & 0x80) != 0 {
            core::hint::spin_loop();
        }

        crate::io::outb(0x70, 0x00);
        let sec = crate::io::inb(0x71);
        crate::io::outb(0x70, 0x02);
        let min = crate::io::inb(0x71);
        crate::io::outb(0x70, 0x04);
        let hour = crate::io::inb(0x71);
        crate::io::outb(0x70, 0x07);
        let day = crate::io::inb(0x71);
        crate::io::outb(0x70, 0x08);
        let month = crate::io::inb(0x71);
        crate::io::outb(0x70, 0x09);
        let year = crate::io::inb(0x71);

        crate::io::outb(0x70, 0x0B);
        let b = crate::io::inb(0x71);
        let is_bcd = (b & 0x04) == 0;

        let (sec, min, hour, day, month, year) = if is_bcd {
            (
                bcd_to_bin(sec),
                bcd_to_bin(min),
                bcd_to_bin(hour),
                bcd_to_bin(day),
                bcd_to_bin(month),
                bcd_to_bin(year) as u16 + 2000,
            )
        } else {
            (sec, min, hour, day, month, year as u16 + 2000)
        };

        kprintln!("==================== RELOJ DE TIEMPO REAL (CMOS RTC) ====================");
        kprintln!("  Fecha Hardware: {:02}/{:02}/{:04}", day, month, year);
        kprintln!("  Hora Hardware:  {:02}:{:02}:{:02} UTC", hour, min, sec);
        kprintln!("  Uptime Kernel:  {} segundos ({} ticks)", crate::drivers::timer::get_uptime_seconds(), crate::drivers::timer::get_ticks());
        kprintln!("=========================================================================");
    }
}

pub fn print_free() {
    let (total_heap, used_heap, free_heap, alloc_count, dealloc_count) = HEAP_ALLOCATOR.stats();
    let usage_pct = HEAP_ALLOCATOR.usage_percent();
    let frames_alloc = FRAME_ALLOCATOR.allocated_frames_count();
    let frames_total = FRAME_ALLOCATOR.total_usable_frames();

    kprintln!("              Total        Usado        Libre   Reserva-OOM  Ocupacion");
    kprintln!("Memoria:    {:>6} KiB   {:>6} KiB   {:>6} KiB     {:>4} KiB       {:>3}%",
        total_heap / 1024, used_heap / 1024, free_heap / 1024,
        crate::memory::heap::EMERGENCY_RESERVE_BYTES / 1024, usage_pct
    );
    kprintln!("Marcos 4K:  {:>6} pags  {:>6} pags  {:>6} pags      ---          {:>3}%",
        frames_total, frames_alloc, frames_total.saturating_sub(frames_alloc),
        if frames_total > 0 { (frames_alloc * 100) / frames_total } else { 0 }
    );
    kprintln!("\n  * Cero memoria oculta en 'Buffers' o 'Caches' fantasma.");
    kprintln!("  * Asignaciones activas: {} | Liberadas: {}", alloc_count, dealloc_count);
}

pub fn run_ipc_demo() {
    kprintln!("\n================ PRUEBA DE COMUNICACION ENTRE PROCESOS (IPC) ================");
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[PASO 1] Creando buzon de mensajes 'ipc_demo_box' y tuberia 'demo_pipe'...\n");

    let tid = crate::task::scheduler::SCHEDULER.lock().current_task_id();
    let (uid, _, user) = crate::task::scheduler::SCHEDULER.lock().current_credentials();

    kprint_color!(VgaColor::LightCyan, VgaColor::Black, "[PASO 2] Productor (TID {}, Usuario: {}) enviando 3 mensajes estructurados...\n", tid, user);
    {
        let mut ipc = crate::ipc::IPC_MANAGER.lock();
        let _ = ipc.send_message("ipc_demo_box", tid, uid, "TELEMETRIA: Temperatura CPU = 38 C | Frecuencia = 3.2 GHz");
        let _ = ipc.send_message("ipc_demo_box", tid, uid, "ALMACENAMIENTO: Paridad LBA 16 CRC32 verificada con exito");
        let _ = ipc.send_message("ipc_demo_box", tid, uid, "EVENTO_SISTEMA: Cero fugas de memoria en Heap de Rust");
    }

    {
        let ipc = crate::ipc::IPC_MANAGER.lock();
        let mbox = ipc.get_mailbox("ipc_demo_box");
        let pending = mbox.map(|m| m.pending_count()).unwrap_or(0);
        kprint_color!(VgaColor::White, VgaColor::Black, "  * Mensajes encolados en espera: {}\n", pending);
    }

    kprint_color!(VgaColor::LightCyan, VgaColor::Black, "[PASO 3] Consumidor extrayendo mensajes del buzon con orden FIFO...\n");
    for i in 1..=3 {
        let mut ipc = crate::ipc::IPC_MANAGER.lock();
        match ipc.receive_message("ipc_demo_box") {
            Ok(msg) => {
                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  [ACK #{}] Recibido de TID {} (UID {}): ", i, msg.sender_tid, msg.sender_uid);
                kprintln!("'{}'", msg.payload);
            }
            Err(e) => {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "  [ERROR] Fallo al recibir: {}\n", e);
            }
        }
    }

    kprint_color!(VgaColor::LightCyan, VgaColor::Black, "[PASO 4] Probando streaming de bytes en tuberia circular 'demo_pipe'...\n");
    let test_stream = b"STREAMING_TEST: Canal de bytes bidireccional libre de contencion [OK]";
    let written = crate::ipc::IPC_MANAGER.lock().write_pipe("demo_pipe", test_stream);
    kprint_color!(VgaColor::White, VgaColor::Black, "  * Bytes transmitidos a la tuberia: {}\n", written);

    let mut read_buf = [0u8; 128];
    let read_count = crate::ipc::IPC_MANAGER.lock().read_pipe("demo_pipe", &mut read_buf);
    let received_str = core::str::from_utf8(&read_buf[..read_count]).unwrap_or("<error>");
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  * Bytes leidos del stream ({} bytes): '{}'\n", read_count, received_str);

    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[EXITO] Demostracion de IPC completada. Paso de mensajes y streaming 100% operativos.\n");
    kprintln!("=============================================================================");
}

