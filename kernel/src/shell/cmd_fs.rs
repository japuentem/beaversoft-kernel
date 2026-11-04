// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Shell Filesystem, Storage (ATA/AHCI/NVMe/GPT/BeaverFS) & Security Commands
// ============================================================================

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use crate::storage::ab_slot::BOOT_SLOT_MANAGER;
use crate::storage::block_device::BLOCK_DEVICE;
use crate::vga::VgaColor;
use crate::{kprint, kprintln, kprint_color};
use super::{Shell, set_current_dir};

pub fn try_execute(shell: &mut Shell, cmd: &str, args: &[&str]) -> bool {
    match cmd {
        "cd" => {
            let target = if args.is_empty() {
                let (uid, _, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
                if uid == 0 { String::from("/") } else { String::from("/home/beaver") }
            } else {
                shell.resolve_path(args[0])
            };

            let is_directory = crate::fs::vfs::VFS.lock().is_dir(&target);
            if is_directory {
                set_current_dir(&target);
                shell.current_dir = target;
            } else {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "cd: {}: No existe el directorio o no es accesible\n", if args.is_empty() { "/" } else { args[0] });
            }
        }

        "pwd" => {
            kprintln!("{}", shell.cwd());
        }

        "ls" | "dir" => {
            let raw_path = if args.is_empty() { shell.cwd() } else { args[0] };
            let path = shell.resolve_path(raw_path);
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
                let path = shell.resolve_path(args[0]);
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
                return true;
            }
            let path = shell.resolve_path(args[0]);
            let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
            match crate::fs::vfs::VFS.lock().create_dir(&path, curr_uid, curr_gid) {
                Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Directorio '{}' creado exitosamente.\n", path),
                Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
            }
        }

        "rm" | "del" => {
            if args.is_empty() {
                kprintln!("Uso: rm <archivo>  (ejemplo: rm /disk/notas.txt)");
                return true;
            }
            let path = shell.resolve_path(args[0]);
            let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
            match crate::fs::vfs::VFS.lock().remove_file(&path, curr_uid, curr_gid) {
                Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Archivo '{}' eliminado.\n", path),
                Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
            }
        }

        "cp" | "copy" => {
            if args.len() < 2 {
                kprintln!("Uso: cp <origen> <destino>  (ejemplo: cp /disk/notas.txt /disk/copia.txt)");
                return true;
            }
            let src = shell.resolve_path(args[0]);
            let dst = shell.resolve_path(args[1]);
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
                return true;
            }
            let src = shell.resolve_path(args[0]);
            let dst = shell.resolve_path(args[1]);
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
                return true;
            }
            let (count, file_arg) = if args[0] == "-n" && args.len() >= 3 {
                (args[1].parse::<usize>().unwrap_or(10), args[2])
            } else {
                (10, args[0])
            };
            let path = shell.resolve_path(file_arg);
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
                return true;
            }
            let (count, file_arg) = if args[0] == "-n" && args.len() >= 3 {
                (args[1].parse::<usize>().unwrap_or(10), args[2])
            } else {
                (10, args[0])
            };
            let path = shell.resolve_path(file_arg);
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
                return true;
            }
            let pattern = args[0];
            let path = shell.resolve_path(args[1]);
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
                return true;
            }
            let path = shell.resolve_path(args[0]);
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

        "stat" => {
            if args.is_empty() {
                kprintln!("Uso: stat <archivo/directorio>");
                return true;
            }
            let path = shell.resolve_path(args[0]);
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

        "tree" => {
            let raw_path = if args.is_empty() { shell.cwd() } else { args[0] };
            let path = shell.resolve_path(raw_path);
            let vfs = crate::fs::vfs::VFS.lock();
            if let Err(e) = vfs.print_tree(&path) {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", e);
            }
        }

        "touch" => {
            if args.is_empty() {
                kprintln!("Uso: touch <ruta_archivo>  (ejemplo: touch notas.txt)");
                return true;
            }
            let path = shell.resolve_path(args[0]);
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
                return true;
            }
            let path = shell.resolve_path(args[0]);
            let content = args[1..].join(" ");
            let (curr_uid, curr_gid, _) = crate::task::scheduler::SCHEDULER.lock().current_credentials();
            let mut vfs = crate::fs::vfs::VFS.lock();
            match vfs.create_file(&path, content.as_bytes(), curr_uid, curr_gid) {
                Ok(()) => kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Escritos {} bytes en '{}'.\n", content.len(), path),
                Err(err) => kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] {}\n", err),
            }
        }

        "df" => {
            print_df();
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

        "disks" | "lsblk" => {
            kprintln!("============================================================");
            kprintln!("  UNIDADES DE ALMACENAMIENTO DETECTADAS (BARE-METAL / QEMU) ");
            kprintln!("============================================================");
            let bdev = crate::storage::block_device::BLOCK_DEVICE.lock();
            kprintln!("  Medio Activo de BeaverFS: {}", bdev.get_storage_medium_name());

            if bdev.is_using_ahci() {
                let ahci = crate::storage::ahci::AHCI_CONTROLLER.lock();
                if let Some(port) = ahci.active_sata_port {
                    let model = ahci.get_port_model(port);
                    let mb = ahci.get_port_capacity_mb(port);
                    kprintln!("  [Disco SATA 0] Controladora AHCI Port {} (SATA III DMA)", port);
                    kprintln!("    * Estado:     CONECTADO Y OPERATIVO POR DMA");
                    kprintln!("    * Modelo:     {}", model);
                    kprintln!("    * Capacidad:  {} MB", mb);
                    kprintln!("    * Particion:  BeaverFS en LBA 256 (Ranura A: LBA 64, Ranura B: LBA 1024)");
                }
            } else {
                let info = crate::storage::installer::BeaverInstaller::probe_target_disk();
                if info.is_present {
                    kprintln!("  [Disco 0] Primary Master ATA/IDE (Puertos 0x1F0..0x1F7)");
                    kprintln!("    * Estado:     CONECTADO Y OPERATIVO (PIO)");
                    kprintln!("    * Modelo:     {}", info.model);
                    kprintln!("    * Capacidad:  {} MB ({} sectores de 512 bytes)", info.size_mb, info.total_sectors);
                    kprintln!("    * Particion:  BeaverFS en LBA 256 (Ranura A: LBA 64, Ranura B: LBA 1024)");
                } else {
                    kprintln!("  [AVISO] No se detecto disco duro ATA primario.");
                }
            }
            kprintln!("============================================================");
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
                return true;
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
                return true;
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

        "install" | "installer" | "deploy" => {
            kprintln!("====================================================================");
            kprintln!("   INSTALADOR UNIVERSAL BEAVER OS (BARE-METAL DISK DEPLOYER)        ");
            kprintln!("   Autor: José Alberto Puente Maldonado (japuentem)                 ");
            kprintln!("====================================================================");
            let info = crate::storage::installer::BeaverInstaller::probe_target_disk();
            if !info.is_present {
                kprint_color!(VgaColor::LightRed, VgaColor::Black, "[ERROR] No se detecto ningun disco duro interno conectado para instalar.\n");
                return true;
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
                    return true;
                }
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
                    shell.current_dir = alloc::string::String::from(user_info.home);
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
                return true;
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
                return true;
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

        _ => return false,
    }
    true
}

pub fn print_df() {
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

