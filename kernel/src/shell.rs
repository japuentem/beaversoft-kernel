// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Dynamic Interactive Kernel Shell (Modular Dispatcher & Line Editor)
// ============================================================================

extern crate alloc;

pub mod cmd_fs;
pub mod cmd_net;
pub mod cmd_sys;

pub use cmd_sys::{hardware_reboot, hardware_shutdown};

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use crate::drivers::timer::{get_ticks, get_uptime_seconds};
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

                for _ in 0..self.command_buffer.len() {
                    kprint!("\x08 \x08");
                }

                if self.history_index + 1 < self.history.len() {
                    self.history_index += 1;
                    self.command_buffer = self.history[self.history_index].clone();
                    kprint!("{}", self.command_buffer);
                } else {
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

        // 1. Delegar a submódulos especializados por categoría
        if cmd_fs::try_execute(self, cmd, args) {
            return;
        }
        if cmd_net::try_execute(self, cmd, args) {
            return;
        }
        if cmd_sys::try_execute(self, cmd, args) {
            return;
        }

        // 2. Comandos generales del intérprete Shell
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
                kprintln!("    mesh / sync    - Descubrimiento P2P y sincronizacion UDP (:4242)");
                kprintln!("    dns <dominio>  - Resuelve nombre de dominio a direccion IPv4");
                kprintln!("    curl <url>     - Consulta o descarga recurso web HTTP/1.1");
                kprintln!("    wget <url> [-O]- Descarga archivo de Internet y guarda en disco /disk");
                kprintln!("    netstat / arp  - Estado de interfaz de red y cache ARP");
                kprintln!("  [HARDWARE, AUDIO, PAQUETES Y SISTEMA]");
                kprintln!("    lsusb / usb    - Lista controladores y dispositivos USB conectados");
                kprintln!("    beaver <f.bs>  - Interpreta y ejecuta scripts en Ring 3");
                kprintln!("    beep / play    - Sintetizador y reproductor PCM/WAV");
                kprintln!("    pkg <cmd>      - Gestor de paquetes ELF64 en /disk/bin");
                kprintln!("    whoami / su    - Sesion de usuario y permisos");
                kprintln!("    dashboard/top  - Monitor en vivo de CPU, tareas y memoria");
                kprintln!("    logo / fetch   - Muestra el emblema oficial y telemetria de Beaver OS");
                kprintln!("    free / memmap  - Mapa transparente de Heap y marcos de 4KB");
                kprintln!("    autotest       - Suite de pruebas automaticas del kernel");
                kprintln!("    gui / desktop  - Inicia escritorio VESA TrueColor");
                kprintln!("    slots          - Estado de ranuras A/B anti-brickeo y Sector Cero");
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
                kprintln!("Subsistemas: A/B Slot Resilient, Zero-Buffer VFS, BeaverFS ATA, VESA TrueColor");
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

            _ => {
                if cmd.starts_with("./") || cmd.ends_with(".elf") || cmd.ends_with(".bs") {
                    let clean_cmd = cmd.trim_start_matches("./");
                    let path = self.resolve_path(clean_cmd);
                    if path.ends_with(".bs") {
                        let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                        if let Ok(source) = crate::fs::vfs::VFS.lock().read_file(&path, curr_uid, curr_gid) {
                            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[RUN-BS RING3] Ejecutando script '{}' en Ring 3...\n", path);
                            if let Ok(output) = crate::task::script::run_script_in_ring3(&source) {
                                if !output.is_empty() {
                                    kprintln!("{}", output);
                                }
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
