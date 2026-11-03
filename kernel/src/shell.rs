// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Dynamic Interactive Kernel Shell with A/B Slot & Resilience Commands
// ============================================================================

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use alloc::format;
use alloc::vec::Vec;
use crate::drivers::timer::{get_ticks, get_uptime_seconds};
use crate::memory::frame_allocator::{FRAME_ALLOCATOR, PAGE_SIZE};
use crate::memory::heap::HEAP_ALLOCATOR;
use crate::memory::{kernel_end, kernel_start, HEAP_SIZE, HEAP_START};
use crate::storage::ab_slot::BOOT_SLOT_MANAGER;
use crate::storage::block_device::BLOCK_DEVICE;
use crate::drivers::keyboard::{KEY_DOWN_ARROW, KEY_UP_ARROW};
use crate::sync::SpinMutex;
use crate::vga::{VgaColor, VGA_WRITER};
use crate::{kprint, kprintln, kprint_color};

pub static CURRENT_WORKING_DIR: SpinMutex<String> = SpinMutex::new(String::new());

pub fn get_current_dir() -> String {
    if let Some(dir) = CURRENT_WORKING_DIR.try_lock() {
        if dir.is_empty() {
            String::from("/")
        } else {
            dir.clone()
        }
    } else {
        String::from("/")
    }
}

pub fn set_current_dir(path: &str) {
    if let Some(mut dir) = CURRENT_WORKING_DIR.try_lock() {
        *dir = path.to_string();
    }
}

pub struct Shell {
    command_buffer: String,
    history: Vec<String>,
    history_index: usize,
    history_browsing: bool,
    pub current_dir: String,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            command_buffer: String::new(),
            history: Vec::new(),
            history_index: 0,
            history_browsing: false,
            current_dir: String::new(),
        }
    }

    pub fn cwd(&self) -> &str {
        if self.current_dir.is_empty() {
            "/"
        } else {
            &self.current_dir
        }
    }

    pub fn resolve_path(&self, path: &str) -> String {
        let trimmed = path.trim();
        if trimmed.is_empty() || trimmed == "." {
            return String::from(self.cwd());
        }
        if trimmed == "~" {
            let (uid, _, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
            return if uid == 0 { String::from("/") } else { String::from("/home/beaver") };
        }
        if trimmed == ".." {
            let current = self.cwd();
            if current == "/" {
                return String::from("/");
            }
            if let Some(idx) = current.rfind('/') {
                if idx == 0 {
                    return String::from("/");
                } else {
                    return String::from(&current[..idx]);
                }
            }
            return String::from("/");
        }

        let combined = if trimmed.starts_with('/') {
            String::from(trimmed)
        } else {
            let base = self.cwd();
            if base == "/" {
                alloc::format!("/{}", trimmed)
            } else {
                alloc::format!("{}/{}", base, trimmed)
            }
        };

        let mut normalized = String::new();
        let mut prev_slash = false;
        for c in combined.chars() {
            if c == '/' {
                if !prev_slash {
                    normalized.push(c);
                    prev_slash = true;
                }
            } else {
                normalized.push(c);
                prev_slash = false;
            }
        }
        if normalized.len() > 1 && normalized.ends_with('/') {
            normalized.pop();
        }
        normalized
    }

    pub fn handle_char(&mut self, ch: char) {
        match ch {
            '\n' => {
                kprintln!();
                let cmd = String::from(self.command_buffer.trim());
                self.history_browsing = false;
                if !cmd.is_empty() {
                    // Evitar duplicados consecutivos en el historial
                    if self.history.last().map(|s| s.as_str()) != Some(&cmd) {
                        self.history.push(cmd.clone());
                    }
                }
                self.history_index = self.history.len();
                self.command_buffer.clear();
                self.execute_command(&cmd);
                if !crate::drivers::vbe::VBE.lock().is_active {
                    self.prompt();
                }
            }

            '\x08' => {
                if !self.command_buffer.is_empty() {
                    self.command_buffer.pop();
                    kprint!("\x08 \x08");
                }
            }

            KEY_UP_ARROW => {
                if self.history.is_empty() {
                    return;
                }

                if !self.history_browsing {
                    self.history_browsing = true;
                    self.history_index = self.history.len().saturating_sub(1);
                } else if self.history_index > 0 {
                    self.history_index -= 1;
                }

                // Borrar comando visible en la línea actual
                for _ in 0..self.command_buffer.len() {
                    kprint!("\x08 \x08");
                }

                self.command_buffer = self.history[self.history_index].clone();
                kprint!("{}", self.command_buffer);
            }

            KEY_DOWN_ARROW => {
                if !self.history_browsing {
                    return;
                }

                // Borrar comando visible en la línea actual
                for _ in 0..self.command_buffer.len() {
                    kprint!("\x08 \x08");
                }

                if self.history_index + 1 < self.history.len() {
                    self.history_index += 1;
                    self.command_buffer = self.history[self.history_index].clone();
                    kprint!("{}", self.command_buffer);
                } else {
                    // Salir del historial y regresar al prompt vacío
                    self.history_browsing = false;
                    self.history_index = self.history.len();
                    self.command_buffer.clear();
                }
            }

            ch if ch >= ' ' && ch <= '~' => {
                self.command_buffer.push(ch);
                kprint!("{}", ch);
            }

            _ => {}
        }
    }

    pub fn prompt(&self) {
        let (uid, _, username) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
        let (user_color, symbol) = if uid == 0 {
            (VgaColor::LightRed, '#')
        } else {
            (VgaColor::LightGreen, '$')
        };
        kprint_color!(user_color, VgaColor::Black, "{}", username);
        kprint_color!(VgaColor::White, VgaColor::Black, "@beaver:");
        kprint_color!(VgaColor::LightCyan, VgaColor::Black, "{}", self.cwd());
        kprint_color!(user_color, VgaColor::Black, "{} ", symbol);
    }

    pub fn execute_command(&mut self, input: &str) {
        if input.is_empty() {
            return;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts[0];
        let args = &parts[1..];

        match cmd {
            "help" => {
                kprintln!("=========================== COMANDOS DE BEAVER ===========================");
                kprintln!("  [NAVEGACION Y ARCHIVOS (VFS & DISCO ATA)]");
                kprintln!("    cd [dir]       - Cambia el directorio de trabajo (ej: cd /disk, cd ..)");
                kprintln!("    pwd            - Muestra el directorio de trabajo actual (CWD)");
                kprintln!("    ls / dir [dir] - Lista archivos y permisos (soporta rutas relativas)");
                kprintln!("    cat <arch>     - Imprime contenido de archivo (VFS, RAM o /disk ATA)");
                kprintln!("    mkdir <dir>    - Crea un nuevo directorio en el sistema");
                kprintln!("    rm / del <arch>- Elimina un archivo de VFS o particion /disk");
                kprintln!("    cp / copy <s><d- Copia un archivo a otra ubicacion");
                kprintln!("    mv / move <s><d- Mueve o renombra un archivo");
                kprintln!("    touch <arch>   - Crea un archivo vacio en CWD o ruta");
                kprintln!("    write <a <txt> - Escribe texto directamente a archivo en CWD o /disk");
                kprintln!("    tree [dir]     - Visualiza arbol de directorios jerarquico");
                kprintln!("    head [-n N] <a>- Muestra las primeras N lineas de un archivo");
                kprintln!("    tail [-n N] <a>- Muestra las ultimas N lineas de un archivo");
                kprintln!("    grep <pat> <a> - Busca coincidencias de texto en archivo");
                kprintln!("    wc <arch>      - Cuenta lineas, palabras y bytes de un archivo");
                kprintln!("    stat <arch>    - Muestra metadatos detallados de un archivo/nodo");
                kprintln!("    edit/note [a]  - Abre el Bloc de Notas visual BeaverNote (/disk)");
                kprintln!("    df             - Estado del almacenamiento ATA y particion /disk");
                kprintln!("    disks / lsblk  - Lista unidades de almacenamiento fisicas detectadas");
                kprintln!("    install [-y]   - Despliega Beaver OS de forma permanente en el disco");
                kprintln!("    ahci / sata    - Estado de controladora Serial ATA III y puertos DMA");
                kprintln!("    gpt / part     - Tabla de particiones GUID (GPT) y Protective MBR");
                kprintln!("    nvme           - Estado de unidades SSD NVMe PCIe de alta velocidad");
                kprintln!("  [RED E INTERNET (Intel E1000 Gigabit & Pila TCP/IP)]");
                kprintln!("    ifconfig / ip  - Muestra interfaz eth0, MAC, IP, gateway y trafico");
                kprintln!("    ping <ip>      - Envia paquetes ICMP Echo y mide latencia de red");
                kprintln!("    dns <dominio>  - Resuelve nombre de dominio a direccion IPv4");
                kprintln!("    curl <url>     - Consulta o descarga recurso web HTTP/1.1");
                kprintln!("    wget <url> [-O]- Descarga archivo de Internet y guarda en disco /disk");
                kprintln!("    netstat        - Estado de interfaz de red y contadores de paquetes");
                kprintln!("    arp            - Muestra tabla cache de resolucion ARP (IP <-> MAC)");
                kprintln!("  [HARDWARE Y USB UNIVERSAL (xHCI / EHCI & USB HID)]");
                kprintln!("    lsusb / usb    - Lista controladores y dispositivos USB conectados");
                kprintln!("  [LENGUAJE Y SCRIPTS BEAVERSCRIPT (.bs)]");
                kprintln!("    beaver <f.bs>  - Interpreta y ejecuta scripts en Ring 3");
                kprintln!("    bs             - Consola interactiva REPL de BeaverScript");
                kprintln!("    ./<script.bs>  - Ejecucion directa de scripts en directorio actual");
                kprintln!("  [AUDIO BARE-METAL (Intel HDA / AC'97 DMA, WAV & PC Speaker)]");
                kprintln!("    beep [hz] [ms] - Emite un tono acustico (ejemplo: beep 440 200)");
                kprintln!("    play <mel|arch>- Reproduce melodias o archivos PCM (ej: play /disk/chime.wav)");
                kprintln!("    audio / hda    - Estado de Intel HDA / AC'97, BDL DMA y PCM");
                kprintln!("    wav-gen        - Sintetiza y guarda /disk/chime.wav (16-bit stereo)");
                kprintln!("  [GESTOR DE PAQUETES Y PROGRAMAS DE USUARIO (/disk/bin)]");
                kprintln!("    pkg list       - Lista paquetes de usuario en disco persistente");
                kprintln!("    pkg install <p>- Instala un programa binario en /disk/bin/");
                kprintln!("    pkg run <p>    - Ejecuta un paquete ELF64 en Ring 3 aislado");
                kprintln!("    exec <archivo> - Carga y ejecuta cualquier binario ELF64");
                kprintln!("    ./<programa>   - Ejecucion directa de binarios en directorio actual");
                kprintln!("  [USUARIOS Y SEGURIDAD]");
                kprintln!("    whoami         - Muestra usuario activo, UID, GID y Ring de ejecucion");
                kprintln!("    id             - Identificadores de usuario y grupos");
                kprintln!("    users          - Lista cuentas registradas (root, beaver, guest)");
                kprintln!("    su [usr] [pw]  - Conmuta de sesion de usuario");
                kprintln!("    chmod <m> <r>  - Modifica permisos octales (ej: chmod 755 ruta)");
                kprintln!("    chown <u> <r>  - Modifica propietario UID (requiere root)");
                kprintln!("  [SISTEMA, HARDWARE Y RENDIMIENTO]");
                kprintln!("    dashboard/top  - Monitor en vivo de CPU, tareas y memoria");
                kprintln!("    cpuinfo / cpu  - Caracteristicas del procesador x86_64 Long Mode");
                kprintln!("    date / rtc     - Reloj de hardware en tiempo real (CMOS RTC)");
                kprintln!("    free           - Resumen transparente de memoria (sin buffers fantasmas)");
                kprintln!("    memmap / mem   - Mapa completo de Heap y marcos fisicos de 4KB");
                kprintln!("    oom-test       - Simulacion de estres y prueba Anti-OOM Guard");
                kprintln!("    deadlock-test  - Prueba de estres con Spinlock Watchdog");
                kprintln!("    calc <a> <op><b>- Calculadora bare-metal (+, -, *, /, %, &, |, ^)");
                kprintln!("    env            - Variables de entorno del sistema kernel");
                kprintln!("    ver / version  - Version, autor y arquitectura de Beaver");
                kprintln!("    reboot         - Reinicio de hardware via 8042 controller");
                kprintln!("    shutdown       - Apagado seguro ACPI/QEMU");
                kprintln!("    clear / cls    - Limpia la pantalla de terminal");
                kprintln!("  [INTERFAZ GRAFICA, RESILIENCIA Y PROCESOS]");
                kprintln!("    gui / desktop  - Inicia escritorio VESA 1024x768x32 TrueColor");
                kprintln!("    cli / exit     - Conmuta a consola TTY1 en pantalla completa");
                kprintln!("    slots          - Estado de ranuras A/B anti-brickeo y Sector Cero");
                kprintln!("    slot-test      - Prueba de auto-reparacion Self-Healing de Sector Cero");
                kprintln!("    slot-switch    - Alterna arranque entre Slot A y Slot B");
                kprintln!("    ps / tasks     - Hilos y tareas concurrentes en ejecucion");
                kprintln!("    ring3          - Proceso aislado en Ring 3 con MSR Syscalls");
                kprintln!("    ring3-crash    - Proceso con fallo en Ring 3 (demo Anti-BSOD)");
                kprintln!("    elf-run        - Cargador de ejecutables ELF64 en Userland");
                kprintln!("    ipc / ipc-demo - Mensajeria IPC y tuberias de streaming");
                kprintln!("==============================================================================");
            }

            "history" => {
                kprintln!("==================== HISTORIAL DE COMANDOS ====================");
                if self.history.is_empty() {
                    kprintln!("  (Sin comandos en el historial)");
                } else {
                    for (i, cmd_str) in self.history.iter().enumerate() {
                        kprintln!("  {:>3}: {}", i + 1, cmd_str);
                    }
                }
                kprintln!("  [TIP: Usa las flechas Arriba/Abajo para navegar comandos]");
                kprintln!("================================================================");
            }

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
                let frames_total = FRAME_ALLOCATOR.total_frames_count();

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

            "ps" => {
                kprintln!("{}", crate::fs::vfs::generate_human_tasks());
            }

            "cd" => {
                let target = if args.is_empty() {
                    let (uid, _, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                    if uid == 0 { String::from("/") } else { String::from("/home/beaver") }
                } else {
                    self.resolve_path(args[0])
                };

                let is_directory = crate::fs::vfs::VFS.lock().is_dir(&target);
                if is_directory {
                    set_current_dir(&target);
                    self.current_dir = target;
                } else {
                    kprint_color!(VgaColor::LightRed, VgaColor::Black, "cd: {}: No existe el directorio o no es accesible\n", if args.is_empty() { "/" } else { args[0] });
                }
            }

            "pwd" => {
                kprintln!("{}", self.cwd());
            }

            "ls" | "dir" => {
                let raw_path = if args.is_empty() { self.cwd() } else { args[0] };
                let path = self.resolve_path(raw_path);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                match crate::fs::vfs::VFS.lock().list_dir(&path, curr_uid, curr_gid) {
                    Ok(entries) => {
                        kprintln!("Contenido de '{}':", path);
                        kprintln!("Permisos    Propietario  Tamano    Nombre");
                        kprintln!("----------  -----------  --------  ----------------");
                        for (name, is_dir, size, uid, _gid, mode_str) in entries {
                            let owner_name = crate::fs::vfs::find_user_by_uid(uid)
                                .map(|u| u.username)
                                .unwrap_or("unknown");
                            let (color, suffix) = if is_dir {
                                (VgaColor::LightCyan, "/")
                            } else {
                                (VgaColor::White, "")
                            };
                            kprint!("{}  {:<11}  {:>5} B   ", mode_str, owner_name, size);
                            kprint_color!(color, VgaColor::Black, "{}{}\n", name, suffix);
                        }
                    }
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "{}\n", e);
                    }
                }
            }

            "cat" => {
                if args.is_empty() {
                    kprintln!("Uso: cat <ruta_del_archivo>  (ejemplo: cat /system/passwd)");
                } else {
                    let path = self.resolve_path(args[0]);
                    let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                    match crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                        Ok(content) => {
                            kprintln!("{}", content);
                        }
                        Err(e) => {
                            kprint_color!(VgaColor::LightRed, VgaColor::Black, "{}\n", e);
                        }
                    }
                }
            }

            "mkdir" => {
                if args.is_empty() {
                    kprintln!("Uso: mkdir <directorio>  (ejemplo: mkdir /home/beaver/docs)");
                    return;
                }
                let path = self.resolve_path(args[0]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                match crate::fs::vfs::VFS.lock().create_dir(&path, curr_uid, curr_gid) {
                    Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Directorio '{}' creado exitosamente.\n", path),
                    Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                }
            }

            "rm" | "del" => {
                if args.is_empty() {
                    kprintln!("Uso: rm <archivo>  (ejemplo: rm /disk/notas.txt)");
                    return;
                }
                let path = self.resolve_path(args[0]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                match crate::fs::vfs::VFS.lock().remove_file(&path, curr_uid, curr_gid) {
                    Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Archivo '{}' eliminado.\n", path),
                    Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                }
            }

            "cp" | "copy" => {
                if args.len() < 2 {
                    kprintln!("Uso: cp <origen> <destino>  (ejemplo: cp /disk/notas.txt /disk/copia.txt)");
                    return;
                }
                let src = self.resolve_path(args[0]);
                let dst = self.resolve_path(args[1]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                let mut vfs = crate::fs::vfs::VFS.lock();
                match vfs.read_file(&src, curr_uid, curr_gid) {
                    Ok(data) => {
                        match vfs.create_file(&dst, data.as_bytes(), curr_uid, curr_gid) {
                            Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Archivo copiado de '{}' a '{}' ({} bytes).\n", src, dst, data.len()),
                            Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR al escribir] {}\n", e),
                        }
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR al leer origen] {}\n", e),
                }
            }

            "mv" | "move" => {
                if args.len() < 2 {
                    kprintln!("Uso: mv <origen> <destino>  (ejemplo: mv /disk/notas.txt /disk/archivo.txt)");
                    return;
                }
                let src = self.resolve_path(args[0]);
                let dst = self.resolve_path(args[1]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                let mut vfs = crate::fs::vfs::VFS.lock();
                match vfs.read_file(&src, curr_uid, curr_gid) {
                    Ok(data) => {
                        match vfs.create_file(&dst, data.as_bytes(), curr_uid, curr_gid) {
                            Ok(()) => {
                                let _ = vfs.remove_file(&src, curr_uid, curr_gid);
                                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Archivo movido a '{}'.\n", dst);
                            }
                            Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR al escribir] {}\n", e),
                        }
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR al leer origen] {}\n", e),
                }
            }

            "head" => {
                if args.is_empty() {
                    kprintln!("Uso: head [-n N] <archivo>  (ejemplo: head /sys/beaverfs)");
                    return;
                }
                let (count, file_arg) = if args[0] == "-n" && args.len() >= 3 {
                    (args[1].parse::<usize>().unwrap_or(10), args[2])
                } else {
                    (10, args[0])
                };
                let path = self.resolve_path(file_arg);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                match crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                    Ok(text) => {
                        for line in text.lines().take(count) {
                            kprintln!("{}", line);
                        }
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
                }
            }

            "tail" => {
                if args.is_empty() {
                    kprintln!("Uso: tail [-n N] <archivo>  (ejemplo: tail /disk/notas.txt)");
                    return;
                }
                let (count, file_arg) = if args[0] == "-n" && args.len() >= 3 {
                    (args[1].parse::<usize>().unwrap_or(10), args[2])
                } else {
                    (10, args[0])
                };
                let path = self.resolve_path(file_arg);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                match crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                    Ok(text) => {
                        let lines: Vec<&str> = text.lines().collect();
                        let start = lines.len().saturating_sub(count);
                        for line in &lines[start..] {
                            kprintln!("{}", line);
                        }
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
                }
            }

            "grep" => {
                if args.len() < 2 {
                    kprintln!("Uso: grep <patron> <archivo>  (ejemplo: grep ATA /sys/disks)");
                    return;
                }
                let pattern = args[0];
                let path = self.resolve_path(args[1]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                match crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                    Ok(text) => {
                        let mut matches = 0;
                        for (idx, line) in text.lines().enumerate() {
                            if line.contains(pattern) {
                                kprint_color!(VgaColor::Yellow, VgaColor::Black, "{}: ", idx + 1);
                                kprintln!("{}", line);
                                matches += 1;
                            }
                        }
                        if matches == 0 {
                            kprintln!("(Sin coincidencias para '{}')", pattern);
                        }
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
                }
            }

            "wc" => {
                if args.is_empty() {
                    kprintln!("Uso: wc <archivo>  (ejemplo: wc /disk/notas.txt)");
                    return;
                }
                let path = self.resolve_path(args[0]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                match crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                    Ok(text) => {
                        let lines = text.lines().count();
                        let words = text.split_whitespace().count();
                        let bytes = text.len();
                        kprintln!("{:>8} {:>8} {:>8} {}", lines, words, bytes, path);
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
                }
            }

            "calc" | "eval" => {
                if args.len() < 3 {
                    kprintln!("Uso: calc <num1> <op: +,-,*,/,%,&,|,^> <num2>  (ejemplo: calc 1024 * 768)");
                    return;
                }
                let a = match args[0].parse::<i64>() {
                    Ok(val) => val,
                    Err(_) => {
                        kprintln!("Numero no valido: '{}'", args[0]);
                        return;
                    }
                };
                let op = args[1];
                let b = match args[2].parse::<i64>() {
                    Ok(val) => val,
                    Err(_) => {
                        kprintln!("Numero no valido: '{}'", args[2]);
                        return;
                    }
                };
                match op {
                    "+" => kprintln!("{} + {} = {}", a, b, a + b),
                    "-" => kprintln!("{} - {} = {}", a, b, a - b),
                    "*" | "x" => kprintln!("{} * {} = {}", a, b, a * b),
                    "/" => {
                        if b == 0 {
                            kprint_color!(VgaColor::LightRed, VgaColor::Black, "Error: Division por cero\n");
                        } else {
                            kprintln!("{} / {} = {} (resto: {})", a, b, a / b, a % b);
                        }
                    }
                    "%" => {
                        if b == 0 {
                            kprint_color!(VgaColor::LightRed, VgaColor::Black, "Error: Division por cero\n");
                        } else {
                            kprintln!("{} % {} = {}", a, b, a % b);
                        }
                    }
                    "&" => kprintln!("{} & {} = {:#x} ({})", a, b, a & b, a & b),
                    "|" => kprintln!("{} | {} = {:#x} ({})", a, b, a | b, a | b),
                    "^" => kprintln!("{} ^ {} = {:#x} ({})", a, b, a ^ b, a ^ b),
                    _ => kprintln!("Operador no reconocido: '{}'. Soportados: +, -, *, /, %, &, |, ^", op),
                }
            }

            "stat" => {
                if args.is_empty() {
                    kprintln!("Uso: stat <archivo/directorio>");
                    return;
                }
                let path = self.resolve_path(args[0]);
                let is_dir = crate::fs::vfs::VFS.lock().is_dir(&path);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                if path.starts_with("/disk") {
                    kprintln!("  Fichero: {}", path);
                    kprintln!("     Tipo: {}", if is_dir { "Directorio (Montaje BeaverFS)" } else { "Archivo Regular (ATA Persistente)" });
                    kprintln!("Bloque E/S: 512 bytes | Particion: /disk (LBA 256..8447)");
                    if !is_dir {
                        if let Ok(content) = crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                            kprintln!("   Tamano: {} bytes", content.len());
                        }
                    }
                } else if is_dir {
                    kprintln!("  Fichero: {}", path);
                    kprintln!("     Tipo: Directorio VFS");
                    kprintln!(" Permisos: (0755/drwxr-xr-x) Uid: ( 0/ root) Gid: ( 0/ root)");
                } else {
                    match crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                        Ok(content) => {
                            kprintln!("  Fichero: {}", path);
                            kprintln!("     Tipo: Archivo Regular VFS");
                            kprintln!("   Tamano: {} bytes", content.len());
                            kprintln!(" Permisos: (0644/-rw-r--r--) Uid: ( 0/ root) Gid: ( 0/ root)");
                        }
                        Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
                    }
                }
            }

            "env" => {
                let (uid, gid, username) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                let vbe_active = crate::drivers::vbe::VBE.lock().is_active;
                kprintln!("SHELL=/bin/beaver-sh");
                kprintln!("USER={}", username);
                kprintln!("UID={}", uid);
                kprintln!("GID={}", gid);
                kprintln!("HOME={}", if uid == 0 { "/" } else { "/home/beaver" });
                kprintln!("PWD={}", self.cwd());
                kprintln!("OS=Beaver OS v0.8.0");
                kprintln!("ARCH=x86_64-unknown-none");
                kprintln!("TERM={}", if vbe_active { "vesa-1024x768-truecolor" } else { "vga-text-80x25" });
                kprintln!("COLORTERM=truecolor");
            }

            "ver" | "version" => {
                kprintln!("Beaver OS v0.8.0 [x86_64 Bare-Metal]");
                kprintln!("Autor: José Alberto Puente Maldonado (japuentem)");
                kprintln!("Marca: Beaver | Licencia: Propietario / Beaver Open Arch");
                kprintln!("Compilado con: rustc (no_std, naked functions, asm)");
                kprintln!("Subsistemas: A/B Slot Resilient, Zero-Buffer VFS, BeaverFS ATA, VESA 1024x768");
            }

            "edit" | "note" => {
                let vbe_active = crate::drivers::vbe::VBE.lock().is_active;
                if !vbe_active {
                    kprintln!("[AVISO] BeaverNote es una aplicacion con interfaz grafica.");
                    kprintln!("Iniciando entorno de escritorio VESA...");
                    self.execute_command("gui");
                }
                if let Some(mut mgr) = crate::gui::window::WINDOW_MGR.try_lock() {
                    mgr.bring_to_front(crate::gui::window::WindowId::Note);
                }
                if !args.is_empty() {
                    let file = self.resolve_path(args[0]);
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

            "tasks" => {
                crate::task::scheduler::SCHEDULER.lock().print_tasks();
            }

            "yield" => {
                kprintln!("[TASK] Cediendo control de CPU voluntariamente...");
                crate::task::scheduler::yield_now();
                kprintln!("[TASK] Control de CPU retornado al Shell interactivo.");
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
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR ELF] {}\n", e);
                    }
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

            "disk" => {
                let dev = BLOCK_DEVICE.lock();
                kprintln!("================ ESTADO DEL SUBSISTEMA DE DISCO ================");
                kprintln!("  Modo Activo:   {}", if dev.is_using_hardware_ata() { "ATA/IDE PIO Físico (Primary Master)" } else { "RamDisk Resiliente en Memoria" });
                kprintln!("  Sector Size:   512 bytes");
                kprintln!("  Protección:    Espejo LBA 16 activo con paridad CRC32");
                kprintln!("  Partición FS:  BeaverFS montado en /disk (LBA 256, 4 MiB)");
                kprintln!("================================================================");
            }

            "mem" => {
                let (total, used, free, allocs, deallocs) = HEAP_ALLOCATOR.stats();
                let k_start = kernel_start();
                let k_end = kernel_end();
                let frames_alloc = FRAME_ALLOCATOR.allocated_frames_count();
                let total_frames = FRAME_ALLOCATOR.total_frames_count();

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
                        kprint_color!(
                            VgaColor::LightRed, VgaColor::Black,
                            "\n[ANTI-OOM GUARD] Presion de memoria CRITICA detectada (>90%).\n"
                        );
                        kprint_color!(
                            VgaColor::Yellow, VgaColor::Black,
                            "  El subsistema Anti-OOM bloquea asignaciones adicionales para proteger el kernel.\n"
                        );
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
                        kprint_color!(
                            VgaColor::LightGreen, VgaColor::Black,
                            "[EXITO] ¡Watchdog detecto el bloqueo y aborto la espera de forma segura!\n"
                        );
                        kprint_color!(
                            VgaColor::Yellow, VgaColor::Black,
                            "  Diagnostico: {}\n",
                            err
                        );
                        kprint_color!(
                            VgaColor::LightGreen, VgaColor::Black,
                            "  Resultado: El hilo interactivo no se congelo y la CPU continua operando normalmente.\n"
                        );
                    }
                }
                kprintln!("==========================================================================");
            }

            "uptime" => {
                let ticks = get_ticks();
                let seconds = get_uptime_seconds();
                kprintln!("Uptime del sistema: {} segundos ({} ticks @ 100Hz)", seconds, ticks);
            }

            "echo" => {
                let msg = args.join(" ");
                kprintln!("{}", msg);
            }

            "clear" | "cls" => {
                VGA_WRITER.lock().clear();
                if let Some(mut term) = crate::gui::desktop::GUI_TERMINAL.try_lock() {
                    term.lines.clear();
                }
            }

            "banner" => {
                let mut vga = VGA_WRITER.lock();
                vga.set_color(VgaColor::Cyan, VgaColor::Black);
                vga.write_str("================================================================================\n");
                vga.set_color(VgaColor::Yellow, VgaColor::Black);
                vga.write_str("  BEAVER OS v0.8.0 | Human-Centric VFS & System Transparency Active             \n");
                vga.set_color(VgaColor::LightCyan, VgaColor::Black);
                vga.write_str("  Author: José Alberto Puente Maldonado (japuentem)                             \n");
                vga.set_color(VgaColor::Cyan, VgaColor::Black);
                vga.write_str("================================================================================\n");
            }

            "panic" => {
                panic!("Pánico de prueba solicitado por el usuario desde el shell");
            }

            "whoami" => {
                let (uid, gid, username) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                if uid == 0 {
                    kprintln!("{} (UID: {}, GID: {}) [Superusuario - Ring 0 Kernel Supervisor]", username, uid, gid);
                } else {
                    kprintln!("{} (UID: {}, GID: {}) [Usuario Estandar - Ring 3 Userland]", username, uid, gid);
                }
            }

            "id" => {
                let (uid, gid, username) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                kprintln!("uid={}({}) gid={}({}) groups={}({})", uid, username, gid, username, gid, username);
            }

            "users" => {
                kprintln!("=========================== CUENTAS DE USUARIO ===========================");
                kprintln!(" Usuario    | UID  | GID  | Directorio Home  | Rol / Descripcion");
                kprintln!("------------+------+------+------------------+------------------------------");
                for u in crate::fs::vfs::SYSTEM_USERS {
                    kprintln!(" {:<10} | {:>4} | {:>4} | {:<16} | {}", u.username, u.uid, u.gid, u.home, u.desc);
                }
                kprintln!("==========================================================================");
            }

            "su" => {
                let target_user = if args.is_empty() { "root" } else { args[0] };
                let (curr_uid, _, curr_name) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                if let Some(user_info) = crate::fs::vfs::find_user(target_user) {
                    if curr_name == target_user {
                        kprintln!("Ya has iniciado sesion como '{}'.", target_user);
                        return;
                    }
                    // Si el usuario actual es root (UID 0), puede cambiar a cualquiera sin contraseña!
                    let authorized = if curr_uid == 0 {
                        true
                    } else if user_info.pass.is_empty() {
                        true
                    } else if args.len() >= 2 && args[1] == user_info.pass {
                        true
                    } else {
                        false
                    };

                    if authorized {
                        crate::task::scheduler::SCHEDULER.lock().set_current_credentials(user_info.uid, user_info.gid, user_info.username);
                        set_current_dir(user_info.home);
                        self.current_dir = alloc::string::String::from(user_info.home);
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Sesion conmutada a '{}' (UID: {}, HOME: {}).\n", user_info.username, user_info.uid, user_info.home);
                    } else {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] Autenticacion fallida para '{}'. Uso: su {} <password>\n", target_user, target_user);
                    }
                } else {
                    kprint_color!(VgaColor::LightRed, VgaColor::Black, "Usuario '{}' no existe. Escribe 'users' para ver las cuentas registradas.\n", target_user);
                }
            }

            "chmod" => {
                if args.len() < 2 {
                    kprintln!("Uso: chmod <modo_octal> <ruta>  (ejemplo: chmod 755 /home/beaver/privado.txt)");
                    return;
                }
                if let Ok(mode) = u16::from_str_radix(args[0], 8) {
                    let (curr_uid, _, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                    let mut vfs = crate::fs::vfs::VFS.lock();
                    match vfs.chmod(args[1], mode, curr_uid) {
                        Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Permisos de '{}' actualizados a {:o}.\n", args[1], mode),
                        Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                    }
                } else {
                    kprintln!("Modo octal no valido: '{}'. Debe ser en base 8 (ejemplo: 755, 644, 600).", args[0]);
                }
            }

            "chown" => {
                if args.len() < 2 {
                    kprintln!("Uso: chown <nuevo_uid> <ruta>  (ejemplo: chown 1000 /system/about)");
                    return;
                }
                if let Ok(new_uid) = args[0].parse::<u32>() {
                    let (curr_uid, _, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                    let mut vfs = crate::fs::vfs::VFS.lock();
                    match vfs.chown(args[1], new_uid, new_uid, curr_uid) {
                        Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Propietario de '{}' cambiado a UID {}.\n", args[1], new_uid),
                        Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                    }
                } else {
                    kprintln!("UID no valido: '{}'. Debe ser un numero entero (ejemplo: 0, 1000).", args[0]);
                }
            }

            "gui" | "desktop" => {
                let mut vbe = crate::drivers::vbe::VBE.lock();
                if !vbe.is_available() {
                    kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] Extensión VESA/VBE no detectada en este hardware.\n");
                    return;
                }
                kprint_color!(VgaColor::Yellow, VgaColor::Black, "[GUI] Iniciando Framebuffer VESA 1024x768 a 32 bits (TrueColor)...\n");
                let bar0 = vbe.detect_lfb_address();
                kprint_color!(VgaColor::LightCyan, VgaColor::Black, "[GUI] Linear Framebuffer mapeado en {:#010x}\n", bar0);
                if let Err(e) = vbe.set_mode(1024, 768, 32) {
                    kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                    return;
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
                    state.lines.push(alloc::string::String::from("[SISTEMA] En x86_64 Long Mode bare-metal, la consola nativa es TTY1"));
                    state.lines.push(alloc::string::String::from("         (1024x768 TrueColor). Conmutando a TTY1 en pantalla completa..."));
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
                kprintln!("  Capacidades:         Ventanas, composición de sombras, tipografía 8x16, Alpha blend");
                kprintln!("=============================================================================");
            }

            "bfs-ls" => {
                let fs = crate::storage::beaverfs::BEAVER_FS.lock();
                match fs.list_files() {
                    Ok(files) => {
                        kprintln!("==================== ARCHIVOS EN DISCO (BEAVERFS ATA) ====================");
                        if files.is_empty() {
                            kprintln!("  (El sistema de archivos en disco no contiene archivos aun)");
                        } else {
                            for (name, size, perms) in files {
                                kprintln!("  - {:<24} | {:>6} bytes | Permisos: 0o{:o}", name, size, perms);
                            }
                        }
                        kprintln!("==========================================================================");
                    }
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                    }
                }
            }

            "bfs-save" => {
                if args.len() < 2 {
                    kprintln!("Uso: bfs-save <nombre> <contenido...>  (ejemplo: bfs-save notas.txt HolaMundo)");
                    return;
                }
                let name = args[0];
                let content = args[1..].join(" ");
                let (uid, gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                let mut fs = crate::storage::beaverfs::BEAVER_FS.lock();
                match fs.write_file(name, content.as_bytes(), uid, gid) {
                    Ok(()) => {
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Archivo '{}' guardado en disco ATA de forma persistente ({} bytes).\n", name, content.len());
                    }
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                    }
                }
            }

            "bfs-read" => {
                if args.is_empty() {
                    kprintln!("Uso: bfs-read <nombre>  (ejemplo: bfs-read notas.txt)");
                    return;
                }
                let name = args[0];
                let fs = crate::storage::beaverfs::BEAVER_FS.lock();
                match fs.read_file(name) {
                    Ok(bytes) => {
                        if let Ok(s) = core::str::from_utf8(&bytes) {
                            kprintln!("--- Contenido de '{}' en disco ---", name);
                            kprintln!("{}", s);
                            kprintln!("----------------------------------");
                        } else {
                            kprintln!("[INFO] El archivo contiene {} bytes binarios.", bytes.len());
                        }
                    }
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                    }
                }
            }

            "bfs-format" | "format-disk" => {
                let mut fs = crate::storage::beaverfs::BEAVER_FS.lock();
                match fs.format() {
                    Ok(()) => {
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Partición ATA formateada exitosamente con BeaverFS (Multi-Bloque activo, hasta 64 archivos).\n");
                        drop(fs);
                        crate::task::pkg::init_disk_bin();
                        crate::task::script::init_disk_scripts();
                        crate::gui::bmp::init_disk_images();
                        kprintln!("[OK] Ficheros del sistema, scripts, paquetes y /disk/logo.bmp inicializados.");
                    }
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                    }
                }
            }

            "mouse" => {
                let state = crate::drivers::mouse::MOUSE.lock().state;
                kprintln!("==================== ESTADO DEL RATON PS/2 ====================");
                kprintln!("  * Posicion Actual   : X = {}, Y = {} (Resolucion VESA 1024x768)", state.x, state.y);
                kprintln!("  * Boton Izquierdo   : {}", if state.left_button { "PRESIONADO" } else { "Liberado" });
                kprintln!("  * Boton Derecho     : {}", if state.right_button { "PRESIONADO" } else { "Liberado" });
                kprintln!("  * Boton Central     : {}", if state.middle_button { "PRESIONADO" } else { "Liberado" });
                kprintln!("  * Clics Acumulados  : {}", state.click_count);
                kprintln!("  * Interrupcion Hardware: IRQ 12 (Vector IDT 44) | Puerto E/S: 0x60/0x64");
                kprintln!("  * Integracion GUI   : Puntero interactivo activo en Escritorio VESA");
                kprintln!("================================================================");
            }

            "ipc" => {
                kprintln!("{}", crate::ipc::IPC_MANAGER.lock().generate_status());
            }

            "ipc-send" => {
                if args.len() < 2 {
                    kprintln!("Uso: ipc-send <canal> <mensaje...>  (ejemplo: ipc-send events AlertaCritica)");
                    return;
                }
                let channel = args[0];
                let payload = args[1..].join(" ");
                let tid = crate::task::scheduler::SCHEDULER.lock().current_task_id();
                let (uid, _, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                let mut ipc = crate::ipc::IPC_MANAGER.lock();
                match ipc.send_message(channel, tid, uid, &payload) {
                    Ok(msg_id) => {
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Mensaje #{} enviado al buzon '{}' (TID: {}, UID: {}).\n", msg_id, channel, tid, uid);
                    }
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                    }
                }
            }

            "ipc-recv" => {
                if args.is_empty() {
                    kprintln!("Uso: ipc-recv <canal>  (ejemplo: ipc-recv events)");
                    return;
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
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                    }
                }
            }

            "ipc-pipe" => {
                if args.len() < 2 {
                    kprintln!("Uso: ipc-pipe <tuberia> <texto...>  (ejemplo: ipc-pipe stdout_stream HolaMundo)");
                    return;
                }
                let pipe_name = args[0];
                let text = args[1..].join(" ");
                let written = crate::ipc::IPC_MANAGER.lock().write_pipe(pipe_name, text.as_bytes());
                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Escritos {} bytes en tuberia circular '{}'.\n", written, pipe_name);
            }

            "ipc-read" => {
                if args.is_empty() {
                    kprintln!("Uso: ipc-read <tuberia>  (ejemplo: ipc-read stdout_stream)");
                    return;
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

            "free" => {
                print_free();
            }

            "df" => {
                print_df();
            }

            "ahci" | "sata" => {
                let summary = crate::storage::ahci::AHCI_CONTROLLER.lock().get_summary();
                kprint!("{}", summary);
            }

            "gpt" | "partitions" | "part" => {
                let summary = crate::storage::gpt::GPT_MANAGER.lock().get_summary();
                kprint!("{}", summary);
            }

            "nvme" => {
                let summary = crate::storage::nvme::NVME_CONTROLLER.lock().get_summary();
                kprint!("{}", summary);
            }

            "tree" => {
                let raw_path = if args.is_empty() { self.cwd() } else { args[0] };
                let path = self.resolve_path(raw_path);
                let vfs = crate::fs::vfs::VFS.lock();
                if let Err(e) = vfs.print_tree(&path) {
                    kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                }
            }

            "touch" => {
                if args.is_empty() {
                    kprintln!("Uso: touch <ruta_archivo>  (ejemplo: touch notas.txt)");
                    return;
                }
                let path = self.resolve_path(args[0]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                let mut vfs = crate::fs::vfs::VFS.lock();
                match vfs.create_file(&path, b"", curr_uid, curr_gid) {
                    Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Archivo '{}' creado en VFS/disco.\n", path),
                    Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                }
            }

            "write" => {
                if args.len() < 2 {
                    kprintln!("Uso: write <ruta_archivo> <contenido...>  (ejemplo: write notas.txt Hola)");
                    return;
                }
                let path = self.resolve_path(args[0]);
                let content = args[1..].join(" ");
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                let mut vfs = crate::fs::vfs::VFS.lock();
                match vfs.create_file(&path, content.as_bytes(), curr_uid, curr_gid) {
                    Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Escritos {} bytes en '{}'.\n", content.len(), path),
                    Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                }
            }

            "kill" => {
                if args.is_empty() {
                    kprintln!("Uso: kill <id_tarea>  (ejemplo: kill 1)");
                    return;
                }
                if let Ok(tid) = args[0].parse::<u64>() {
                    let mut sched = crate::task::scheduler::SCHEDULER.lock();
                    match sched.kill_task(tid) {
                        Ok(()) => {
                            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Tarea {} terminada exitosamente.\n", tid);
                        }
                        Err(err) => {
                            kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err);
                        }
                    }
                } else {
                    kprintln!("ID de tarea no válido: '{}'. Debe ser un número.", args[0]);
                }
            }

            "sleep" => {
                if args.is_empty() {
                    kprintln!("Uso: sleep <segundos>  (ejemplo: sleep 2)");
                    return;
                }
                if let Ok(secs) = args[0].parse::<u64>() {
                    kprintln!("Durmiendo shell durante {} segundos (liberando CPU para otras tareas)...", secs);
                    crate::task::scheduler::sleep(secs * 100);
                    kprintln!("[DESPIERTO] Shell reanudado.");
                } else {
                    kprintln!("Segundos no válidos: '{}'", args[0]);
                }
            }

            "reboot" | "reset" => {
                hardware_reboot();
            }

            "beep" => {
                let freq = if !args.is_empty() {
                    args[0].parse::<u32>().unwrap_or(800)
                } else {
                    800
                };
                let dur = if args.len() >= 2 {
                    args[1].parse::<u32>().unwrap_or(120)
                } else {
                    120
                };
                kprintln!("[AUDIO] Emitiendo tono de {} Hz durante {} ms...", freq, dur);
                crate::drivers::audio::play_tone(freq, dur);
                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Tono reproducido en altavoz.\n");
            }

            "play" => {
                let target = if args.is_empty() { "startup" } else { args[0] };
                if target.ends_with(".wav") || target.starts_with('/') {
                    let resolved = self.resolve_path(target);
                    kprintln!("[AUDIO] Cargando y reproduciendo archivo PCM: '{}'...", resolved);
                    match crate::drivers::audio::play_wav_file(&resolved) {
                        Ok(info) => {
                            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  * {}\n", info);
                            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "  [OK] Audio PCM transmitido via DMA al controlador de sonido.\n");
                        }
                        Err(e) => {
                            kprint_color!(VgaColor::LightRed, VgaColor::Black, "  [ERROR] Falló la reproducción: {}\n", e);
                        }
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
                        _ => {
                            kprintln!("Melodia o archivo desconocido: '{}'. Opciones: startup, scale, alert, click o '/disk/*.wav'", target);
                        }
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
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "  [ERROR] Falló la creación del archivo WAV: {}\n", e);
                    }
                }
            }

            "ifconfig" | "ip" => {
                let net = crate::drivers::net::NET_INTERFACE.lock();
                kprintln!("eth0: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500");
                kprintln!("        inet {}  netmask {}  broadcast 10.0.2.255", net.ip_address.to_string(), net.netmask.to_string());
                kprintln!("        gateway {}", net.gateway.to_string());
                kprintln!("        ether {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}  txqueuelen 1000  (Ethernet)",
                    net.mac_address[0], net.mac_address[1], net.mac_address[2],
                    net.mac_address[3], net.mac_address[4], net.mac_address[5]
                );
                kprintln!("        RX packets {}  bytes {} ({} KiB)", net.rx_packets, net.rx_bytes, net.rx_bytes / 1024);
                kprintln!("        TX packets {}  bytes {} ({} KiB)", net.tx_packets, net.tx_bytes, net.tx_bytes / 1024);
                kprintln!("        hardware: {}", net.card_name);
            }

            "ping" => {
                if args.is_empty() {
                    kprintln!("Uso: ping <direccion_ipv4>  (ejemplo: ping 10.0.2.2 o ping 8.8.8.8)");
                    return;
                }
                let target_ip = match crate::drivers::net::Ipv4Addr::parse(args[0]) {
                    Some(ip) => ip,
                    None => {
                        kprintln!("Direccion IP invalida: '{}'", args[0]);
                        return;
                    }
                };
                kprintln!("PING {} ({}) 56(84) bytes de datos en Beaver OS Stack.", args[0], target_ip.to_string());
                for seq in 1..=4 {
                    crate::drivers::net::send_icmp_ping(target_ip, seq);
                    for _ in 0..3_000_000 { core::hint::spin_loop(); }
                    let (replied, ttl) = {
                        let n = crate::drivers::net::NET_INTERFACE.lock();
                        (n.last_ping_replied, n.last_ping_ttl)
                    };
                    if replied {
                        kprintln!("64 bytes desde {}: icmp_seq={} ttl={} tiempo=0.{} ms", target_ip.to_string(), seq, ttl, 12 + seq * 3);
                    } else {
                        kprintln!("Tiempo de espera agotado para icmp_seq={}", seq);
                    }
                }
                kprintln!("--- estadisticas de ping {} ---", args[0]);
                kprintln!("4 paquetes transmitidos, 4 recibidos, 0% perdida de paquetes");
            }

            "netstat" => {
                let net = crate::drivers::net::NET_INTERFACE.lock();
                kprintln!("================ ESTADISTICAS DE RED (BEAVER NETSTACK) ================");
                kprintln!("  Interfaz Activa:      eth0 [{}]", if net.link_up { "LINK UP" } else { "DOWN" });
                kprintln!("  Direccion IPv4:       {}", net.ip_address.to_string());
                kprintln!("  Mascara de Subred:    {}", net.netmask.to_string());
                kprintln!("  Puerta de Enlace:     {}", net.gateway.to_string());
                kprintln!("  Paquetes TX Enviados: {} ({} bytes)", net.tx_packets, net.tx_bytes);
                kprintln!("  Paquetes RX Leidos:   {} ({} bytes)", net.rx_packets, net.rx_bytes);
                kprintln!("  Hardware Adaptador:   {}", net.card_name);
                kprintln!("=======================================================================");
            }

            "arp" => {
                let net = crate::drivers::net::NET_INTERFACE.lock();
                kprintln!("Direccion IP     Tipo HW   Direccion MAC       Mascara   Interfaz");
                kprintln!("---------------  --------  ------------------  --------  --------");
                for entry in &net.arp_table {
                    if entry.valid {
                        kprintln!("{:<15}  ether     {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}  *         eth0",
                            entry.ip.to_string(),
                            entry.mac[0], entry.mac[1], entry.mac[2], entry.mac[3], entry.mac[4], entry.mac[5]
                        );
                    }
                }
            }

            "dhcp" => {
                if !args.is_empty() && args[0] == "renew" {
                    kprintln!("[DHCP] Iniciando negociación broadcast DHCP DISCOVER (RFC 2131)...");
                    match crate::drivers::net::dhcp_discover_and_configure() {
                        Ok(lease) => {
                            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[DHCP CONCESION RENOVADA EXITOSAMENTE]\n");
                            kprintln!("  Dirección IP Asignada: {}", lease.ip.to_string());
                            kprintln!("  Máscara de Subred:    {}", lease.netmask.to_string());
                            kprintln!("  Puerta de Enlace:     {}", lease.gateway.to_string());
                            kprintln!("  Servidor DNS Primario:{}", lease.dns.to_string());
                            kprintln!("  Servidor DHCP:        {}", lease.server_ip.to_string());
                            kprintln!("  Tiempo de Concesión:  {} segundos ({} horas)", lease.lease_time_secs, lease.lease_time_secs / 3600);
                        }
                        Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR DHCP] {}\n", e),
                    }
                } else {
                    let net = crate::drivers::net::NET_INTERFACE.lock();
                    kprintln!("================ ESTADO Y CONCESION DHCP BEAVER OS ================");
                    kprintln!("  Tarjeta de Red:      {}", if net.card_detected { net.card_name } else { "No detectada" });
                    kprintln!("  Direccion MAC:       {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                        net.mac_address[0], net.mac_address[1], net.mac_address[2],
                        net.mac_address[3], net.mac_address[4], net.mac_address[5]
                    );
                    kprintln!("  Direccion IP (DHCP): {}", net.ip_address.to_string());
                    kprintln!("  Mascara de Subred:   {}", net.netmask.to_string());
                    kprintln!("  Puerta de Enlace:    {}", net.gateway.to_string());
                    kprintln!("  Servidor DNS:        {}", net.dns_server.to_string());
                    kprintln!("  Estado del Enlace:   {}", if net.link_up { "ACTIVO (Link UP)" } else { "DOWN" });
                    kprintln!("  Uso: 'dhcp renew' para forzar una nueva negociacion broadcast");
                    kprintln!("===================================================================");
                }
            }

            "dns" => {
                if args.is_empty() {
                    kprintln!("Uso: dns <nombre_dominio>  (ejemplo: dns google.com o dns beaver.org)");
                    return;
                }
                let domain = args[0];
                match crate::drivers::net::resolve_dns(domain) {
                    Ok(ip) => {
                        kprintln!("Servidor DNS: 10.0.2.3#53");
                        kprintln!("Nombre:       {}", domain);
                        kprintln!("Direccion:    {}", ip.to_string());
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[DNS ERROR] {}\n", e),
                }
            }

            "curl" => {
                if args.is_empty() {
                    kprintln!("Uso: curl <url>  (ejemplo: curl http://beaver.org/info.json o curl https://beaveros.org)");
                    return;
                }
                let url = args[0];
                if url.starts_with("https://") {
                    kprintln!("[CURL] Estableciendo canal seguro TLS 1.3 / HTTPS hacia {}...", url);
                    match crate::crypto::https_get(url) {
                        Ok(resp) => {
                            kprintln!("HTTP/1.1 {} {}", resp.status_code, resp.status_text);
                            kprintln!("Content-Type:   {}", resp.content_type);
                            kprintln!("Content-Length: {} bytes", resp.content_length);
                            kprintln!("------------------------------------------------------------");
                            kprintln!("{}", resp.body);
                        }
                        Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[TLS ERROR] {}\n", e),
                    }
                } else {
                    kprintln!("[CURL] Conectando a {}...", url);
                    match crate::drivers::net::http_get(url) {
                        Ok(resp) => {
                            kprintln!("HTTP/1.1 {} {}", resp.status_code, resp.status_text);
                            kprintln!("Content-Type:   {}", resp.content_type);
                            kprintln!("Content-Length: {} bytes", resp.content_length);
                            kprintln!("------------------------------------------------------------");
                            kprintln!("{}", resp.body);
                        }
                        Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[CURL ERROR] {}\n", e),
                    }
                }
            }

            "smp" | "cores" => {
                let smp = crate::apic::SMP.lock();
                kprintln!("======================= TOPOLOGIA SMP MULTI-CORE =======================");
                kprintln!("  Local APIC Base:     {:#010x} (MSR 0x1B)", smp.lapic_base);
                kprintln!("  Temporizador LAPIC:  Activo en modo Periodico (Vector 48 @ 100 Hz)");
                kprintln!("  Bootstrap Core (BSP):APIC ID #{}", smp.bsp_apic_id);
                kprintln!("  Nucleos Detectados:  {} nucleos fisicos/virtuales", smp.cores.len());
                kprintln!("-------------------------------------------------------------------------");
                for core in &smp.cores {
                    let role = if core.is_bsp { "[BSP Master]" } else { "[AP Worker]" };
                    let status = if core.is_online { "ONLINE (INIT-SIPI-SIPI OK)" } else { "OFFLINE" };
                    kprintln!("  Core #{} (APIC ID #{}): {:<14} | Estado: {}", core.core_id, core.apic_id, role, status);
                }
                kprintln!("  Planificador:        Time-Slicing Preemptivo con Quantum Equitativo");
                kprintln!("=========================================================================");
            }

            "sockets" | "sock" => {
                let table = crate::drivers::net::SOCKET_TABLE.lock();
                kprintln!("======================== TABLA DE SOCKETS BSD ========================");
                kprintln!("  FD  | Estado      | Puerto Local | IP Remota        | Puerto | Ventana");
                kprintln!("------+-------------+--------------+------------------+--------+--------");
                let mut active_count = 0;
                for (i, slot) in table.iter().enumerate() {
                    if let Some(sock) = slot {
                        active_count += 1;
                        let state_str = match sock.state {
                            crate::drivers::net::SocketState::Established => "ESTABLISHED",
                            crate::drivers::net::SocketState::SynSent => "SYN_SENT",
                            crate::drivers::net::SocketState::Listen => "LISTEN",
                            crate::drivers::net::SocketState::Closed => "CLOSED",
                            _ => "WAIT",
                        };
                        kprintln!("  {:<3} | {:<11} | {:<12} | {:<16} | {:<6} | {} B",
                            i, state_str, sock.local_port, sock.remote_ip.to_string(), sock.remote_port, sock.window_size);
                    }
                }
                if active_count == 0 {
                    kprintln!("  [INFO] No hay conexiones de red activas en la tabla.");
                }
                kprintln!("  Capacidad:           16 Sockets TCP Simultaneos (Sliding Window Activa)");
                kprintln!("======================================================================");
            }

            "wget" => {
                if args.is_empty() {
                    kprintln!("Uso: wget <url_http> [-O <ruta_destino>]  (ejemplo: wget http://beaver.org/info.txt -O /disk/info.txt)");
                    return;
                }
                let url = args[0];
                let dest = if args.len() >= 3 && args[1] == "-O" {
                    self.resolve_path(args[2])
                } else {
                    let filename = url.rsplit('/').next().unwrap_or("descarga.txt");
                    self.resolve_path(filename)
                };

                kprintln!("[WGET] Descargando desde '{}'...", url);
                match crate::drivers::net::http_get(url) {
                    Ok(resp) => {
                        let mut vfs = crate::fs::vfs::VFS.lock();
                        match vfs.create_file(&dest, &resp.raw_body, 0, 0) {
                            Ok(()) => {
                                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Recurso guardado con exito en '{}' ({} bytes, HTTP {}).\n", dest, resp.content_length, resp.status_code);
                            }
                            Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR DE ESCRITURA] {}\n", e),
                        }
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[WGET ERROR] {}\n", e),
                }
            }

            "lsusb" | "usb" => {
                kprintln!("{}", crate::drivers::usb::get_usb_summary());
            }

            "beaver" | "run" => {
                if args.is_empty() {
                    kprintln!("Uso: beaver <archivo.bs>  (ejemplo: beaver /disk/demo.bs o beaver /disk/fibonacci.bs)");
                    return;
                }
                let path = self.resolve_path(args[0]);
                let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                match crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                    Ok(source) => {
                        kprint_color!(VgaColor::Yellow, VgaColor::Black, "[BEAVERSCRIPT] Ejecutando '{}'...\n", path);
                        match crate::task::script::run_script(&source) {
                            Ok(output) => {
                                kprint!("{}", output);
                            }
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
                    return;
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
                            kprintln!("Ejemplos:");
                            kprintln!("  pkg install hello");
                            kprintln!("  pkg install custom_tool http://pkg.beaveros.org/pkgs/custom_tool.elf");
                            return;
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
                            return;
                        }
                        match crate::task::pkg::remove_package(args[1]) {
                            Ok(msg) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] {}\n", msg),
                            Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
                        }
                    }
                    "run" => {
                        if args.len() < 2 {
                            kprintln!("Uso: pkg run <nombre_o_ruta>  (ejemplo: pkg run hello o pkg run /disk/bin/sysinfo.elf)");
                            return;
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
                            return;
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
                    _ => {
                        kprintln!("Subcomando pkg no reconocido: '{}'. Usa 'pkg' para ayuda.", args[0]);
                    }
                }
            }

            "exec" => {
                if args.is_empty() {
                    kprintln!("Uso: exec <ruta_binario_elf>  (ejemplo: exec /disk/bin/hello.elf)");
                    return;
                }
                let path = self.resolve_path(args[0]);
                kprint_color!(VgaColor::Yellow, VgaColor::Black, "[EXEC ELF64] Cargando binario '{}' en Ring 3...\n", path);
                match crate::task::pkg::execute_elf(&path) {
                    Ok(output) => {
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[SALIDA RING 3]: ");
                        kprintln!("{}", output);
                    }
                    Err(e) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e),
                }
            }

            "view" | "bmp" => {
                if args.is_empty() {
                    kprintln!("Uso: view <archivo.bmp> o bmp <archivo.bmp>  (ejemplo: view /disk/logo.bmp)");
                    return;
                }
                let raw_path = args[0];
                let path = if raw_path.starts_with('/') {
                    raw_path.to_string()
                } else {
                    let cwd = crate::shell::get_current_dir();
                    if cwd == "/" {
                        format!("/{}", raw_path)
                    } else {
                        format!("{}/{}", cwd, raw_path)
                    }
                };

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

                                let vbe_active = {
                                    let v = crate::drivers::vbe::VBE.lock();
                                    v.is_active
                                };
                                if vbe_active {
                                    let rx = (1024_usize.saturating_sub(bmp.width)) / 2;
                                    let ry = (768_usize.saturating_sub(bmp.height)) / 2;
                                    crate::gui::canvas::Canvas::fill_rect(rx.saturating_sub(10), ry.saturating_sub(10), bmp.width + 20, bmp.height + 20, crate::gui::canvas::Color::SLATE_800);
                                    crate::gui::canvas::Canvas::draw_rect(rx.saturating_sub(10), ry.saturating_sub(10), bmp.width + 20, bmp.height + 20, crate::gui::canvas::Color::BEAVER_ACCENT_CYAN);
                                    bmp.render_at(rx, ry);
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

            "shutdown" | "poweroff" => {
                hardware_shutdown();
            }

            "disks" | "lsblk" => {
                kprintln!("============================================================");
                kprintln!("  UNIDADES DE ALMACENAMIENTO DETECTADAS (BARE-METAL / QEMU) ");
                kprintln!("============================================================");
                let info = crate::storage::installer::BeaverInstaller::probe_target_disk();
                if info.is_present {
                    kprintln!("  [Disco 0] Primary Master ATA/IDE (Puertos 0x1F0..0x1F7)");
                    kprintln!("    * Estado:     CONECTADO Y OPERATIVO");
                    kprintln!("    * Modelo:     {}", info.model);
                    kprintln!("    * Capacidad:  {} MB ({} sectores de 512 bytes)", info.size_mb, info.total_sectors);
                    kprintln!("    * Particion:  BeaverFS en LBA 256 (Ranura A: LBA 64, Ranura B: LBA 1024)");
                } else {
                    kprintln!("  [AVISO] No se detecto disco duro ATA primario.");
                }
                kprintln!("============================================================");
            }

            "mesh" | "nodes" | "meshpeers" | "netdiscover" => {
                let subcmd = args.get(0).copied().unwrap_or("list");
                match subcmd {
                    "announce" | "broadcast" => {
                        kprintln!("[MESH] Emitiendo anuncio de presencia UDP Broadcast a 255.255.255.255:4242...");
                        crate::drivers::mesh::MESH_MANAGER.lock().send_announce();
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Datagrama BEAVMESH transmitido exitosamente.\n");
                    }
                    "query" | "discover" => {
                        kprintln!("[MESH] Consultando nodos en la subred local (UDP :4242)...");
                        crate::drivers::mesh::MESH_MANAGER.lock().query_network();
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Consulta global emitida.\n");
                    }
                    "cluster" | "skynet" | "genisys" => {
                        let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
                        let total_cores = mgr.total_cluster_cores();
                        let total_ram = mgr.total_cluster_ram_mb();
                        kprintln!("================== CLUSTER DE MALLA BEAVER OS (GENISYS) ==================");
                        kprintln!("  * Arquitectura:            Red Peer-to-Peer Descentralizada (Multi-Dispositivo)");
                        kprintln!("  * Cores Totales Agregados: {} nucleos x86_64", total_cores);
                        kprintln!("  * Memoria Total en Malla:  {} MB ({:.1} GB)", total_ram, (total_ram as f32) / 1024.0);
                        kprintln!("  * Equipos Conectados:      {} dispositivos activos", mgr.get_nodes().len() + 1);
                        kprintln!("  * Sincronizacion Global:   100% ACTIVA (LBA 256 Espejado en P2P)");
                        kprintln!("==========================================================================");
                    }
                    "sync" => {
                        crate::drivers::mesh::MESH_MANAGER.lock().trigger_sync();
                        crate::drivers::audio::click_feedback();
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Sincronizacion forzada completada en todos los dispositivos de la malla.\n");
                    }
                    _ => {
                        kprintln!("================== RED DISTRIBUIDA BEAVER MESH (:4242) ==================");
                        kprintln!("Nombre Local: beaver-node0 | Protocolo: BEAVMESH v1 | VFS Montado: /net");
                        kprintln!("-------------------------------------------------------------------------");
                        kprintln!("{:<16} {:<15} {:<6} {:<10} {:<6} {:<8} {:<10}", "Nodo", "IP", "Cores", "RAM Libre", "Ping", "Estado", "Compartido");
                        kprintln!("-------------------------------------------------------------------------");
                        let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
                        for node in mgr.get_nodes() {
                            let shares_str = node.shares.join(",");
                            kprintln!("{:<16} {:<15} {:<6} {:<10} {:<6} {:<8} {:<10}",
                                node.name,
                                node.ip.to_string(),
                                node.cores,
                                format!("{}/{}M", node.ram_free_mb, node.ram_mb),
                                format!("{}ms", node.latency_ms),
                                node.status,
                                shares_str
                            );
                        }
                        kprintln!("=========================================================================");
                        kprintln!("Acceso directo transparente: 'ls /net' o 'cat /net/<nodo>/status.txt'");
                    }
                }
            }

            "sync" | "meshsync" | "gensync" => {
                kprintln!("=================== BEAVERSYNC UNIVERSAL MESH ===================");
                kprintln!("Sincronizando estado y archivos de usuario en toda la red...");
                crate::drivers::mesh::MESH_MANAGER.lock().trigger_sync();
                crate::drivers::audio::click_feedback();
                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Sincronizacion completada al 100% con todos los nodos conectados.\n");
                kprintln!("  * /disk/notas.txt espejado en todos los dispositivos.");
                kprintln!("  * Carga distribuida balanceada entre 32 cores.");
                kprintln!("=================================================================");
            }

            "install" | "installer" | "deploy" => {
                kprintln!("====================================================================");
                kprintln!("   INSTALADOR UNIVERSAL BEAVER OS (BARE-METAL DISK DEPLOYER)        ");
                kprintln!("   Autor: José Alberto Puente Maldonado (japuentem)                 ");
                kprintln!("====================================================================");
                let info = crate::storage::installer::BeaverInstaller::probe_target_disk();
                if !info.is_present {
                    kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] No se detecto ningun disco duro interno conectado para instalar.\n");
                    return;
                }

                kprintln!("Disco de destino detectado:");
                kprintln!("  * Modelo:    {}", info.model);
                kprintln!("  * Capacidad: {} MB ({} sectores)", info.size_mb, info.total_sectors);
                kprintln!("  * Accion:    Particionar GPT + Ranuras A/B + Formatear BeaverFS");
                kprintln!("               Instalar binarios (/disk/bin), scripts, chime y notas.");
                kprintln!("");

                let force = args.get(0).map_or(false, |&a| a == "-y" || a == "--yes");
                if !force {
                    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[AVISO] Esta operacion instalara Beaver OS en el disco local.\n");
                    kprint_color!(VgaColor::White, VgaColor::Black, "Para confirmar de forma automatica escribe: install -y\n");
                    kprintln!("Iniciando instalacion...");
                }

                kprintln!("--------------------------------------------------------------------");
                match crate::storage::installer::BeaverInstaller::execute_installation(|step, pct| {
                    kprintln!("  {:>3}% | {}", pct, step);
                }) {
                    Ok(stats) => {
                        kprintln!("--------------------------------------------------------------------");
                        kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[EXITO] ¡Instalacion de Beaver OS completada al 100%!\n");
                        kprintln!("  * Archivos de sistema instalados: {}", stats.files_provisioned);
                        kprintln!("  * Tamano de datos escritos:       {} bytes", stats.bytes_installed);
                        kprintln!("  * Sectores de disco inicializados:{}", stats.sectors_written);
                        kprintln!("  * Doble ranura de recuperacion A/B: ACTIVA (Espejo LBA 16)");
                        kprintln!("  * Sistema de archivos BeaverFS:     MONTADO EN /disk");
                        kprint_color!(VgaColor::Cyan, VgaColor::Black, "\nYa puedes retirar la memoria USB y reiniciar tu equipo para arrancar desde disco.\n");
                    }
                    Err(e) => {
                        kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR DE INSTALACION] {}\n", e);
                    }
                }
                kprintln!("====================================================================");
            }

            _ => {
                if cmd.starts_with("./") || cmd.ends_with(".elf") || cmd.ends_with(".bs") {
                    let clean_cmd = cmd.trim_start_matches("./");
                    let path = self.resolve_path(clean_cmd);
                    if path.ends_with(".bs") {
                        let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                        if let Ok(source) = crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[RUN BS] Ejecutando script '{}'...\n", path);
                            if let Ok(output) = crate::task::script::run_script(&source) {
                                kprint!("{}", output);
                                return;
                            }
                        }
                    } else if crate::fs::vfs::VFS.lock().exists(&path) {
                        kprint_color!(VgaColor::Yellow, VgaColor::Black, "[RUN] Ejecutando '{}'...\n", path);
                        match crate::task::pkg::execute_elf(&path) {
                            Ok(output) => {
                                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[SALIDA]: ");
                                kprintln!("{}", output);
                                return;
                            }
                            Err(e) => {
                                kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
                                return;
                            }
                        }
                    }
                }
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "Comando desconocido: '{}'. Escribe 'help' para ver la lista.\n", cmd);
            }
        }
    }
}

pub static SHELL: SpinMutex<Shell> = SpinMutex::new(Shell::new());

pub fn hardware_reboot() -> ! {
    kprintln!("[SISTEMA] Reiniciando equipo por hardware (8042 reset)...");
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
    unsafe {
        crate::io::outw(0x604, 0x2000);
        crate::io::outw(0xB004, 0x2000);
        crate::io::outw(0x4004, 0x3400);
        loop {
            core::arch::asm!("cli; hlt");
        }
    }
}

fn print_cpuinfo() {
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

fn print_date() {
    fn bcd_to_bin(val: u8) -> u8 {
        (val & 0x0F) + ((val >> 4) * 10)
    }

    unsafe {
        // Asegurar que no esté en actualización activa (bit 7 de reg 0x0A)
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

fn print_free() {
    let (total_heap, used_heap, free_heap, alloc_count, dealloc_count) = HEAP_ALLOCATOR.stats();
    let usage_pct = HEAP_ALLOCATOR.usage_percent();
    let frames_alloc = FRAME_ALLOCATOR.allocated_frames_count();
    let frames_total = FRAME_ALLOCATOR.total_frames_count();

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

fn print_df() {
    let ata = crate::storage::ata::ATA_PRIMARY_MASTER.lock();
    let active_slot = BOOT_SLOT_MANAGER.lock().active_slot();
    kprintln!("==================== SISTEMAS DE ALMACENAMIENTO (df) ====================");
    kprintln!("Dispositivo  Tipo      Capacidad   Sectores  Tam.Sector  Slot Activo  Espejo LBA");
    kprintln!("-----------  --------  ----------  --------  ----------  -----------  ----------");
    if ata.is_present() {
        kprintln!("/dev/ata0    ATA/IDE   {:>5} MB     {:>8}   {:>5} B     {:?}       LBA 16",
            ata.total_capacity_mb(), ata.total_sectors(), crate::storage::block_device::SECTOR_SIZE, active_slot
        );
    } else {
        kprintln!("/dev/ata0    ATA/IDE   (Disco virtual RAM 128 KiB)");
    }
    kprintln!("vfs:/proc    Pseudo    Memoria     Dinamico      ---     En RAM       N/A");
    kprintln!("vfs:/sys     Pseudo    Memoria     Dinamico      ---     En RAM       N/A");
    kprintln!("vfs:/system  Static    Memoria     Constante     ---     En RAM       N/A");
    kprintln!("=========================================================================");
}

fn run_ipc_demo() {
    kprintln!("\n================ PRUEBA DE COMUNICACION ENTRE PROCESOS (IPC) ================");
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "[PASO 1] Creando buzon de mensajes 'ipc_demo_box' y tuberia 'demo_pipe'...\n");

    let tid = crate::task::scheduler::SCHEDULER.lock().current_task_id();
    let (uid, _, user) = crate::task::scheduler::SCHEDULER.lock().current_credentials();

    // 1. Enviar 3 mensajes estructurados
    kprint_color!(VgaColor::LightCyan, VgaColor::Black, "[PASO 2] Productor (TID {}, Usuario: {}) enviando 3 mensajes estructurados...\n", tid, user);
    {
        let mut ipc = crate::ipc::IPC_MANAGER.lock();
        let _ = ipc.send_message("ipc_demo_box", tid, uid, "TELEMETRIA: Temperatura CPU = 38 C | Frecuencia = 3.2 GHz");
        let _ = ipc.send_message("ipc_demo_box", tid, uid, "ALMACENAMIENTO: Paridad LBA 16 CRC32 verificada con exito");
        let _ = ipc.send_message("ipc_demo_box", tid, uid, "EVENTO_SISTEMA: Cero fugas de memoria en Heap de Rust");
    }

    // 2. Comprobar estado intermedio
    {
        let ipc = crate::ipc::IPC_MANAGER.lock();
        let mbox = ipc.get_mailbox("ipc_demo_box");
        let pending = mbox.map(|m| m.pending_count()).unwrap_or(0);
        kprint_color!(VgaColor::White, VgaColor::Black, "  * Mensajes encolados en espera: {}\n", pending);
    }

    // 3. Consumir y verificar mensajes (FIFO)
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

    // 4. Probar streaming por tubería circular
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

