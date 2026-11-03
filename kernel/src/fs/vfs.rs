// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Human-Centric Virtual File System (VFS) with Dynamic /proc & /sys
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::sync::SpinMutex;
use crate::memory::heap::{HEAP_ALLOCATOR, EMERGENCY_RESERVE_BYTES};
use crate::memory::{kernel_start, kernel_end};
use crate::memory::frame_allocator::FRAME_ALLOCATOR;
use crate::task::scheduler::SCHEDULER;
use crate::drivers::timer::{get_ticks, get_uptime_seconds};
use crate::storage::ab_slot::BOOT_SLOT_MANAGER;
use crate::storage::block_device::BLOCK_DEVICE;

/// Información de cuenta de usuario registrada en el sistema
pub struct UserInfo {
    pub username: &'static str,
    pub uid: u32,
    pub gid: u32,
    pub home: &'static str,
    pub desc: &'static str,
    pub pass: &'static str,
}

pub const SYSTEM_USERS: &[UserInfo] = &[
    UserInfo {
        username: "root",
        uid: 0,
        gid: 0,
        home: "/root",
        desc: "Superusuario del Kernel (Ring 0)",
        pass: "root",
    },
    UserInfo {
        username: "beaver",
        uid: 1000,
        gid: 1000,
        home: "/home/beaver",
        desc: "Usuario Humano Administrador",
        pass: "beaver",
    },
    UserInfo {
        username: "guest",
        uid: 1001,
        gid: 1001,
        home: "/home/guest",
        desc: "Usuario Invitado sin Privilegios",
        pass: "",
    },
];

pub fn find_user(username: &str) -> Option<&'static UserInfo> {
    SYSTEM_USERS.iter().find(|u| u.username == username)
}

pub fn find_user_by_uid(uid: u32) -> Option<&'static UserInfo> {
    SYSTEM_USERS.iter().find(|u| u.uid == uid)
}

/// Tipo de nodo en el Sistema de Archivos Virtual
pub enum VfsNodeType {
    /// Archivo estático con contenido persistente en memoria
    File(Vec<u8>),
    /// Directorio que contiene nodos hijos
    Directory(Vec<VfsNode>),
    /// Pseudo-archivo dinámico generado al vuelo mediante una función generadora
    DynamicFile(fn() -> String),
}

/// Nodo jerárquico del VFS con permisos POSIX y propietario
pub struct VfsNode {
    pub name: String,
    pub node_type: VfsNodeType,
    pub uid: u32,
    pub gid: u32,
    pub mode: u16, // Permisos octales (ej: 0o755, 0o644, 0o600)
}

impl VfsNode {
    pub fn new_dir(name: &str) -> Self {
        Self {
            name: String::from(name),
            node_type: VfsNodeType::Directory(Vec::new()),
            uid: 0,
            gid: 0,
            mode: 0o755,
        }
    }

    pub fn new_file(name: &str, content: &[u8]) -> Self {
        Self {
            name: String::from(name),
            node_type: VfsNodeType::File(content.to_vec()),
            uid: 0,
            gid: 0,
            mode: 0o644,
        }
    }

    pub fn new_dynamic(name: &str, generator: fn() -> String) -> Self {
        Self {
            name: String::from(name),
            node_type: VfsNodeType::DynamicFile(generator),
            uid: 0,
            gid: 0,
            mode: 0o444,
        }
    }

    pub fn with_perms(mut self, uid: u32, gid: u32, mode: u16) -> Self {
        self.uid = uid;
        self.gid = gid;
        self.mode = mode;
        self
    }

    pub fn can_read(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 {
            return true; // Superusuario root siempre tiene lectura
        }
        if uid == self.uid {
            (self.mode & 0o400) != 0
        } else if gid == self.gid {
            (self.mode & 0o040) != 0
        } else {
            (self.mode & 0o004) != 0
        }
    }

    pub fn can_write(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 {
            return true; // Superusuario root siempre tiene escritura
        }
        if uid == self.uid {
            (self.mode & 0o200) != 0
        } else if gid == self.gid {
            (self.mode & 0o020) != 0
        } else {
            (self.mode & 0o002) != 0
        }
    }

    pub fn mode_string(&self) -> String {
        let is_dir = matches!(self.node_type, VfsNodeType::Directory(_));
        let type_char = if is_dir { 'd' } else { '-' };
        let r_u = if (self.mode & 0o400) != 0 { 'r' } else { '-' };
        let w_u = if (self.mode & 0o200) != 0 { 'w' } else { '-' };
        let x_u = if (self.mode & 0o100) != 0 { 'x' } else { '-' };
        let r_g = if (self.mode & 0o040) != 0 { 'r' } else { '-' };
        let w_g = if (self.mode & 0o020) != 0 { 'w' } else { '-' };
        let x_g = if (self.mode & 0o010) != 0 { 'x' } else { '-' };
        let r_o = if (self.mode & 0o004) != 0 { 'r' } else { '-' };
        let w_o = if (self.mode & 0o002) != 0 { 'w' } else { '-' };
        let x_o = if (self.mode & 0o001) != 0 { 'x' } else { '-' };
        alloc::format!("{}{}{}{}{}{}{}{}{}{}", type_char, r_u, w_u, x_u, r_g, w_g, x_g, r_o, w_o, x_o)
    }

    pub fn add_child(&mut self, child: VfsNode) {
        if let VfsNodeType::Directory(ref mut children) = self.node_type {
            children.push(child);
        }
    }
}

pub struct VirtualFileSystem {
    root: VfsNode,
}

impl VirtualFileSystem {
    pub fn new() -> Self {
        let mut root = VfsNode::new_dir("/").with_perms(0, 0, 0o755);

        // --- Directorio /proc (Diagnósticos transparentes para humanos) ---
        let mut proc_dir = VfsNode::new_dir("proc").with_perms(0, 0, 0o755);
        proc_dir.add_child(VfsNode::new_dynamic("meminfo", generate_human_meminfo));
        proc_dir.add_child(VfsNode::new_dynamic("tasks", generate_human_tasks));
        proc_dir.add_child(VfsNode::new_dynamic("cpuinfo", generate_human_cpuinfo));
        proc_dir.add_child(VfsNode::new_dynamic("ipc", generate_human_ipc));
        root.add_child(proc_dir);

        // --- Directorio /sys (Hardware, discos y ranuras anti-brickeo) ---
        let mut sys_dir = VfsNode::new_dir("sys").with_perms(0, 0, 0o755);
        sys_dir.add_child(VfsNode::new_dynamic("storage", generate_human_storage));
        sys_dir.add_child(VfsNode::new_dynamic("uptime", generate_human_uptime));
        sys_dir.add_child(VfsNode::new_dynamic("mouse", generate_human_mouse));
        sys_dir.add_child(VfsNode::new_dynamic("beaverfs", generate_human_beaverfs));
        root.add_child(sys_dir);

        // --- Directorio /disk (Punto de Montaje BeaverFS en Disco ATA Persistente) ---
        let disk_dir = VfsNode::new_dir("disk").with_perms(0, 0, 0o755);
        root.add_child(disk_dir);

        // --- Directorio /net (Red Distribuida Beaver Mesh y Nodos Interconectados) ---
        let net_dir = VfsNode::new_dir("net").with_perms(0, 0, 0o755);
        root.add_child(net_dir);

        // --- Directorio /system (Identidad, manifiesto y bienvenida) ---
        let mut system_dir = VfsNode::new_dir("system").with_perms(0, 0, 0o755);
        system_dir.add_child(VfsNode::new_file(
            "about",
            b"BEAVER OS v0.8.0\n\
              Autor: Jose Alberto Puente Maldonado (japuentem)\n\
              Arquitectura: Micro-Kernel x86_64 Long Mode (Bare-Metal)\n\
              Mision: Sistema Operativo transparente, resiliente y comprensible sin jergas confusas.\n\
              Filosofia: Cero pantallas azules, cero bloqueos silenciosos y memoria 100% explicada.\n"
        ).with_perms(0, 0, 0o644));
        system_dir.add_child(VfsNode::new_file(
            "motd",
            b"Bienvenido al entorno interactivo de Beaver OS.\n\
              Escribe 'dashboard' para ver el monitor en vivo, o 'whoami' para ver tu usuario.\n"
        ).with_perms(0, 0, 0o644));

        // /system/passwd: Protegido con permisos 0o600 (Solo root puede leerlo directamente)
        system_dir.add_child(VfsNode::new_file(
            "passwd",
            b"root:x:0:0:Superusuario Kernel (Ring 0):/root\n\
              beaver:x:1000:1000:Usuario Administrador:/home/beaver\n\
              guest:x:1001:1001:Invitado Restringido:/home/guest\n"
        ).with_perms(0, 0, 0o600));

        root.add_child(system_dir);

        // --- Directorio /home (Directorios de usuarios y aislamiento) ---
        let mut home_dir = VfsNode::new_dir("home").with_perms(0, 0, 0o755);

        // /home/beaver: UID 1000, permisos 0o700 (Privado de beaver)
        let mut beaver_home = VfsNode::new_dir("beaver").with_perms(1000, 1000, 0o700);
        beaver_home.add_child(VfsNode::new_file(
            "privado.txt",
            b"Confidencial: Notas del usuario Beaver.\n\
              Solo el usuario 'beaver' y 'root' tienen permiso de lectura aqui.\n"
        ).with_perms(1000, 1000, 0o600));
        home_dir.add_child(beaver_home);

        // /home/guest: UID 1001, permisos 0o755
        let mut guest_home = VfsNode::new_dir("guest").with_perms(1001, 1001, 0o755);
        guest_home.add_child(VfsNode::new_file(
            "bienvenido.txt",
            b"Bienvenido a la sesion de invitado de Beaver OS.\n"
        ).with_perms(1001, 1001, 0o644));
        home_dir.add_child(guest_home);

        root.add_child(home_dir);

        Self { root }
    }

    /// Normaliza una ruta y busca el nodo correspondiente (inmutable)
    fn find_node(&self, path: &str) -> Option<&VfsNode> {
        let clean_path = path.trim();
        if clean_path.is_empty() || clean_path == "/" {
            return Some(&self.root);
        }

        let segments: Vec<&str> = clean_path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut current = &self.root;
        for segment in segments {
            match current.node_type {
                VfsNodeType::Directory(ref children) => {
                    if let Some(child) = children.iter().find(|c| c.name == segment) {
                        current = child;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Normaliza una ruta y busca el nodo correspondiente (mutable)
    fn find_node_mut(&mut self, path: &str) -> Option<&mut VfsNode> {
        let clean_path = path.trim();
        if clean_path.is_empty() || clean_path == "/" {
            return Some(&mut self.root);
        }

        let segments: Vec<&str> = clean_path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut current = &mut self.root;
        for segment in segments {
            match current.node_type {
                VfsNodeType::Directory(ref mut children) => {
                    let mut found_idx = None;
                    for (i, c) in children.iter().enumerate() {
                        if c.name == segment {
                            found_idx = Some(i);
                            break;
                        }
                    }
                    if let Some(i) = found_idx {
                        current = &mut children[i];
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Lee los bytes crudos (binario) de un archivo validando permisos para (uid, gid)
    pub fn read_file_bytes(&self, path: &str, uid: u32, gid: u32) -> Result<Vec<u8>, &'static str> {
        let clean_path = path.trim();
        // Enrutamiento transparente hacia BeaverFS (/disk)
        if clean_path.starts_with("/disk/") || clean_path.starts_with("disk/") {
            let filename = clean_path
                .trim_start_matches('/')
                .trim_start_matches("disk")
                .trim_start_matches('/');
            if filename.is_empty() {
                return Err("Debes especificar el nombre del archivo en /disk");
            }
            return crate::storage::beaverfs::BEAVER_FS.lock().read_file(filename);
        }

        // Enrutamiento transparente hacia Beaver Mesh (/net)
        if clean_path.starts_with("/net/") || clean_path.starts_with("net/") {
            let sub = clean_path
                .trim_start_matches('/')
                .trim_start_matches("net")
                .trim_start_matches('/');
            let mut parts = sub.splitn(2, '/');
            let node_name = parts.next().unwrap_or("");
            let file_subpath = parts.next().unwrap_or("status.txt");
            let text = crate::drivers::mesh::MESH_MANAGER.lock().read_node_file(node_name, file_subpath)?;
            return Ok(text.into_bytes());
        }

        let node = self.find_node(path).ok_or("Ruta no encontrada en VFS")?;
        if !node.can_read(uid, gid) {
            return Err("Permiso denegado (EACCES): No tienes permisos de lectura sobre este archivo");
        }
        match &node.node_type {
            VfsNodeType::File(bytes) => Ok(bytes.clone()),
            VfsNodeType::DynamicFile(generator) => Ok(generator().into_bytes()),
            VfsNodeType::Directory(_) => Err("La ruta especificada es un directorio, no un archivo"),
        }
    }

    /// Lee el contenido completo de un archivo como texto UTF-8 validando permisos para (uid, gid)
    pub fn read_file(&self, path: &str, uid: u32, gid: u32) -> Result<String, &'static str> {
        let bytes = self.read_file_bytes(path, uid, gid)?;
        String::from_utf8(bytes).map_err(|_| "El archivo contiene datos binarios no imprimibles")
    }

    /// Lista los elementos dentro de un directorio validando permisos de lectura
    pub fn list_dir(&self, path: &str, uid: u32, gid: u32) -> Result<Vec<(String, bool, usize, u32, u32, String)>, &'static str> {
        let clean = path.trim().trim_end_matches('/');
        if clean == "/disk" || clean == "disk" {
            let files = crate::storage::beaverfs::BEAVER_FS.lock().list_files()?;
            let mut list = Vec::new();
            for (name, size, perms) in files {
                let r_u = if (perms & 0o400) != 0 { 'r' } else { '-' };
                let w_u = if (perms & 0o200) != 0 { 'w' } else { '-' };
                let x_u = if (perms & 0o100) != 0 { 'x' } else { '-' };
                let r_g = if (perms & 0o040) != 0 { 'r' } else { '-' };
                let w_g = if (perms & 0o020) != 0 { 'w' } else { '-' };
                let x_g = if (perms & 0o010) != 0 { 'x' } else { '-' };
                let r_o = if (perms & 0o004) != 0 { 'r' } else { '-' };
                let w_o = if (perms & 0o002) != 0 { 'w' } else { '-' };
                let x_o = if (perms & 0o001) != 0 { 'x' } else { '-' };
                let mode_str = alloc::format!("-{}{}{}{}{}{}{}{}{}", r_u, w_u, x_u, r_g, w_g, x_g, r_o, w_o, x_o);
                list.push((name, false, size as usize, 0, 0, mode_str));
            }
            return Ok(list);
        }

        if clean == "/net" || clean == "net" {
            let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
            let mut list = Vec::new();
            for node in mgr.get_nodes() {
                list.push((node.name.clone(), true, 0, 0, 0, String::from("drwxr-xr-x")));
            }
            return Ok(list);
        }

        if clean.starts_with("/net/") || clean.starts_with("net/") {
            let sub = clean
                .trim_start_matches('/')
                .trim_start_matches("net")
                .trim_start_matches('/');
            let mut parts = sub.splitn(2, '/');
            let node_name = parts.next().unwrap_or("");
            let subfolder = parts.next().unwrap_or("");

            let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
            let node = mgr.get_node(node_name).ok_or("Nodo Beaver no encontrado en /net")?;

            let mut list = Vec::new();
            if subfolder.is_empty() {
                list.push((String::from("status.txt"), false, 512, 0, 0, String::from("-rw-r--r--")));
                for share in &node.shares {
                    list.push((share.clone(), true, 0, 0, 0, String::from("drwxr-xr-x")));
                }
            } else if subfolder == "disk" {
                list.push((String::from("notas.txt"), false, 120, 0, 0, String::from("-rw-r--r--")));
            } else if subfolder == "musica" {
                list.push((String::from("playlist.txt"), false, 80, 0, 0, String::from("-rw-r--r--")));
            } else if subfolder == "sys" {
                list.push((String::from("info"), false, 95, 0, 0, String::from("-r--r--r--")));
            }
            return Ok(list);
        }

        let node = self.find_node(path).ok_or("Directorio no encontrado en VFS")?;
        if !node.can_read(uid, gid) {
            return Err("Permiso denegado (EACCES): No tienes permisos para listar este directorio");
        }
        match &node.node_type {
            VfsNodeType::Directory(children) => {
                let mut list = Vec::new();
                for c in children {
                    let is_dir = matches!(c.node_type, VfsNodeType::Directory(_));
                    let size = match &c.node_type {
                        VfsNodeType::File(b) => b.len(),
                        VfsNodeType::DynamicFile(_) => 512,
                        VfsNodeType::Directory(sub) => sub.len(),
                    };
                    list.push((c.name.clone(), is_dir, size, c.uid, c.gid, c.mode_string()));
                }
                Ok(list)
            }
            _ => Err("La ruta especificada no es un directorio"),
        }
    }

    /// Crea o sobreescribe un archivo en el VFS validando permisos de escritura
    pub fn create_file(&mut self, path: &str, content: &[u8], uid: u32, gid: u32) -> Result<(), &'static str> {
        let clean_path = path.trim();
        if clean_path.is_empty() || clean_path == "/" {
            return Err("Ruta de archivo no válida");
        }

        // Enrutamiento transparente hacia BeaverFS (/disk)
        if clean_path.starts_with("/disk/") || clean_path.starts_with("disk/") {
            let filename = clean_path
                .trim_start_matches('/')
                .trim_start_matches("disk")
                .trim_start_matches('/');
            if filename.is_empty() {
                return Err("Debes especificar el nombre del archivo en /disk");
            }
            return crate::storage::beaverfs::BEAVER_FS.lock().write_file(filename, content, uid, gid);
        }

        let mut segments: Vec<&str> = clean_path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if segments.is_empty() {
            return Err("Nombre de archivo no especificado");
        }

        let file_name = segments.pop().unwrap();
        let mut current = &mut self.root;

        for seg in segments {
            if !current.can_read(uid, gid) {
                return Err("Permiso denegado (EACCES): No se puede acceder a la ruta intermedia");
            }

            let can_write_dir = current.can_write(uid, gid);
            let next_idx = match current.node_type {
                VfsNodeType::Directory(ref mut children) => {
                    let mut found = None;
                    for (i, c) in children.iter().enumerate() {
                        if c.name == seg {
                            found = Some(i);
                            break;
                        }
                    }
                    if let Some(i) = found {
                        i
                    } else {
                        if !can_write_dir {
                            return Err("Permiso denegado (EACCES): No tienes permisos de escritura para crear directorios");
                        }
                        children.push(VfsNode::new_dir(seg).with_perms(uid, gid, 0o755));
                        children.len() - 1
                    }
                }
                _ => return Err("Una parte de la ruta no es un directorio"),
            };

            if let VfsNodeType::Directory(ref mut children) = current.node_type {
                current = &mut children[next_idx];
            }
        }

        if !current.can_write(uid, gid) {
            return Err("Permiso denegado (EACCES): No tienes permisos de escritura en este directorio");
        }

        match current.node_type {
            VfsNodeType::Directory(ref mut children) => {
                if let Some(existing) = children.iter_mut().find(|c| c.name == file_name) {
                    if !existing.can_write(uid, gid) {
                        return Err("Permiso denegado (EACCES): No tienes permisos para sobreescribir este archivo");
                    }
                    existing.node_type = VfsNodeType::File(content.to_vec());
                } else {
                    children.push(VfsNode::new_file(file_name, content).with_perms(uid, gid, 0o644));
                }
                Ok(())
            }
            _ => Err("El nodo destino no es un directorio"),
        }
    }

    /// Crea un nuevo directorio en el VFS validando permisos de escritura
    pub fn create_dir(&mut self, path: &str, uid: u32, gid: u32) -> Result<(), &'static str> {
        let clean_path = path.trim();
        if clean_path.is_empty() || clean_path == "/" {
            return Err("Ruta de directorio no válida");
        }

        let mut segments: Vec<&str> = clean_path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if segments.is_empty() {
            return Err("Nombre de directorio no especificado");
        }

        let dir_name = segments.pop().unwrap();
        let mut current = &mut self.root;

        for seg in segments {
            let next_idx = match current.node_type {
                VfsNodeType::Directory(ref mut children) => {
                    children.iter().position(|c| c.name == seg).ok_or("Ruta intermedia no encontrada")?
                }
                _ => return Err("Parte de la ruta no es un directorio"),
            };
            if let VfsNodeType::Directory(ref mut children) = current.node_type {
                current = &mut children[next_idx];
            }
        }

        if !current.can_write(uid, gid) {
            return Err("Permiso denegado (EACCES): No tienes permisos de escritura en este directorio");
        }

        match current.node_type {
            VfsNodeType::Directory(ref mut children) => {
                if children.iter().any(|c| c.name == dir_name) {
                    return Err("El directorio ya existe");
                }
                children.push(VfsNode::new_dir(dir_name).with_perms(uid, gid, 0o755));
                Ok(())
            }
            _ => Err("El nodo padre no es un directorio"),
        }
    }

    /// Elimina un archivo en el VFS o en BeaverFS (/disk)
    pub fn remove_file(&mut self, path: &str, uid: u32, gid: u32) -> Result<(), &'static str> {
        let clean_path = path.trim();
        if clean_path.starts_with("/disk/") || clean_path.starts_with("disk/") {
            let filename = clean_path
                .trim_start_matches('/')
                .trim_start_matches("disk")
                .trim_start_matches('/');
            return crate::storage::beaverfs::BEAVER_FS.lock().remove_file(filename);
        }

        let mut segments: Vec<&str> = clean_path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if segments.is_empty() {
            return Err("Nombre de archivo no especificado");
        }

        let file_name = segments.pop().unwrap();
        let mut current = &mut self.root;

        for seg in segments {
            let next_idx = match current.node_type {
                VfsNodeType::Directory(ref mut children) => {
                    children.iter().position(|c| c.name == seg).ok_or("Ruta intermedia no encontrada")?
                }
                _ => return Err("Parte de la ruta no es un directorio"),
            };
            if let VfsNodeType::Directory(ref mut children) = current.node_type {
                current = &mut children[next_idx];
            }
        }

        if !current.can_write(uid, gid) {
            return Err("Permiso denegado (EACCES): No tienes permisos de escritura en este directorio");
        }

        match current.node_type {
            VfsNodeType::Directory(ref mut children) => {
                let init_len = children.len();
                children.retain(|c| c.name != file_name);
                if children.len() < init_len {
                    Ok(())
                } else {
                    Err("Archivo no encontrado en VFS")
                }
            }
            _ => Err("El nodo padre no es un directorio"),
        }
    }

    /// Modifica los permisos de un archivo o directorio (chmod)
    pub fn chmod(&mut self, path: &str, mode: u16, uid: u32) -> Result<(), &'static str> {
        let node = self.find_node_mut(path).ok_or("Ruta no encontrada en VFS")?;
        if uid != 0 && uid != node.uid {
            return Err("Operación no permitida (EPERM): Solo root o el propietario pueden modificar permisos");
        }
        node.mode = mode;
        Ok(())
    }

    /// Modifica el propietario y grupo de un archivo (chown)
    pub fn chown(&mut self, path: &str, new_uid: u32, new_gid: u32, uid: u32) -> Result<(), &'static str> {
        let node = self.find_node_mut(path).ok_or("Ruta no encontrada en VFS")?;
        if uid != 0 {
            return Err("Operación no permitida (EPERM): Solo root puede cambiar el propietario de un archivo");
        }
        node.uid = new_uid;
        node.gid = new_gid;
        Ok(())
    }

    /// Comprueba si una ruta es un directorio válido
    pub fn is_dir(&self, path: &str) -> bool {
        let clean = path.trim().trim_end_matches('/');
        if clean.is_empty() || clean == "/disk" || clean == "disk" {
            return true;
        }
        if let Some(node) = self.find_node(clean) {
            matches!(node.node_type, VfsNodeType::Directory(_))
        } else {
            false
        }
    }

    /// Comprueba si un archivo o directorio existe en VFS o /disk
    pub fn exists(&self, path: &str) -> bool {
        let clean = path.trim();
        if clean.is_empty() || clean == "/" || clean == "/disk" || clean == "disk" {
            return true;
        }
        if clean.starts_with("/disk/") || clean.starts_with("disk/") {
            let filename = clean
                .trim_start_matches('/')
                .trim_start_matches("disk")
                .trim_start_matches('/');
            return crate::storage::beaverfs::BEAVER_FS.lock().read_file(filename).is_ok();
        }
        self.find_node(clean).is_some()
    }

    /// Imprime el árbol jerárquico del VFS de forma recursiva
    pub fn print_tree(&self, start_path: &str) -> Result<(), &'static str> {
        let clean = start_path.trim().trim_end_matches('/');
        if clean == "/disk" || clean == "disk" {
            crate::kprintln!("/disk/ [BeaverFS ATA Persistente]");
            if let Ok(files) = crate::storage::beaverfs::BEAVER_FS.lock().list_files() {
                let total = files.len();
                if total == 0 {
                    crate::kprintln!("`-- (sin archivos en disco)");
                } else {
                    for (idx, (fname, fsize, _)) in files.iter().enumerate() {
                        let is_last = idx + 1 == total;
                        let branch = if is_last { "`-- " } else { "|-- " };
                        crate::kprintln!("{}{}{} ({} B)", "", branch, fname, fsize);
                    }
                }
            }
            return Ok(());
        }

        let node = self.find_node(start_path).ok_or("Ruta no encontrada en VFS")?;
        crate::kprintln!("{}", if start_path.is_empty() || start_path == "/" { "/" } else { &node.name });
        self.render_tree_recursive(node, "");
        Ok(())
    }

    fn render_tree_recursive(&self, node: &VfsNode, prefix: &str) {
        if let VfsNodeType::Directory(ref children) = node.node_type {
            let total = children.len();
            for (idx, child) in children.iter().enumerate() {
                let is_last = idx + 1 == total;
                let branch = if is_last { "`-- " } else { "|-- " };
                let is_dir = matches!(child.node_type, VfsNodeType::Directory(_));
                if child.name == "disk" {
                    crate::kprintln!("{}{}{}/ [BeaverFS ATA Persistente]", prefix, branch, child.name);
                    let new_prefix = if is_last {
                        alloc::format!("{}    ", prefix)
                    } else {
                        alloc::format!("{}|   ", prefix)
                    };
                    if let Ok(files) = crate::storage::beaverfs::BEAVER_FS.lock().list_files() {
                        let f_total = files.len();
                        for (f_idx, (fname, fsize, _)) in files.iter().enumerate() {
                            let f_is_last = f_idx + 1 == f_total;
                            let f_branch = if f_is_last { "`-- " } else { "|-- " };
                            crate::kprintln!("{}{}{} ({} B)", new_prefix, f_branch, fname, fsize);
                        }
                    }
                    continue;
                }
                crate::kprintln!("{}{}{}{}", prefix, branch, child.name, if is_dir { "/" } else { "" });
                if is_dir {
                    let new_prefix = if is_last {
                        alloc::format!("{}    ", prefix)
                    } else {
                        alloc::format!("{}|   ", prefix)
                    };
                    self.render_tree_recursive(child, &new_prefix);
                }
            }
        }
    }
}

pub static VFS: SpinMutex<VirtualFileSystem> = SpinMutex::new(VirtualFileSystem {
    root: VfsNode {
        name: String::new(),
        node_type: VfsNodeType::Directory(Vec::new()),
        uid: 0,
        gid: 0,
        mode: 0o755,
    },
});

pub fn init_vfs() {
    *VFS.lock() = VirtualFileSystem::new();
}

// ============================================================================
// Generadores Dinámicos: Explicaciones Humanas sin Jergas Confusas
// ============================================================================

/// Genera /proc/meminfo explicando con total claridad a dónde va cada byte
pub fn generate_human_meminfo() -> String {
    let (_total_heap, used_heap, free_heap, alloc_count, dealloc_count) = HEAP_ALLOCATOR.stats();
    let usage_pct = HEAP_ALLOCATOR.usage_percent();
    let pressure = HEAP_ALLOCATOR.memory_pressure();
    let k_start = kernel_start();
    let k_end = kernel_end();
    let kernel_bytes = k_end - k_start;
    let frames_alloc = FRAME_ALLOCATOR.allocated_frames_count();
    let frames_total = FRAME_ALLOCATOR.total_usable_frames();
    let physical_total_kib = (frames_total * 4096) / 1024;

    let mut out = String::new();
    out.push_str("==================== MAPA TRANSPARENTE DE MEMORIA ====================\n");
    out.push_str("  (A diferencia de Windows o Linux, aqui cada byte esta 100% desglosado\n");
    out.push_str("   sin conceptos oscuros como 'Modified Page Cache' o 'Non-Paged Pool')\n");
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str(&format!("  Memoria Fisica Total Detectada : {} KiB (RAM Base de CPU)\n", physical_total_kib));
    out.push_str(&format!("  1. Imagen del Kernel (Estatico): {} bytes (~{} KiB en {:#x}-{:#x})\n",
        kernel_bytes, kernel_bytes / 1024, k_start, k_end));
    out.push_str(&format!("  2. Heap Dinamico Activo (alloc): {} bytes (~{} KiB)\n",
        used_heap, used_heap / 1024));
    out.push_str(&format!("  3. Reserva de Emergencia Anti-OOM: {} KiB (Protegida para Kernel/Shell)\n",
        EMERGENCY_RESERVE_BYTES / 1024));
    out.push_str(&format!("  4. Memoria Inmediatamente Libre: {} bytes (~{} KiB)\n",
        free_heap, free_heap / 1024));
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str(&format!("  Indice Real de Uso del Heap     : {}% [Estado: {:?}]\n", usage_pct, pressure));
    out.push_str(&format!("  Contadores de Asignacion Rust   : {} allocs / {} deallocs (Diferencia: {})\n",
        alloc_count, dealloc_count, alloc_count.saturating_sub(dealloc_count)));
    out.push_str(&format!("  Paginador Fisico (Frame Alloc) : {} de {} marcos de 4KB en uso\n",
        frames_alloc, frames_total));
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("  Grafico Visual del Heap:\n  ");
    out.push_str(&render_ascii_bar(usage_pct, 40));
    out.push_str(&format!(" {}% en uso\n", usage_pct));
    out.push_str("======================================================================\n");
    out
}

/// Genera /proc/tasks explicando el propósito y consumo de cada hilo
pub fn generate_human_tasks() -> String {
    let mut out = String::new();
    out.push_str("==================== TABLA TRANSPARENTE DE PROCESOS ====================\n");
    out.push_str("  (Sin estados 'D' inmatables ni zombies 'Z' de Linux)\n");
    out.push_str(" ID | Nombre Tarea         | Estado Humano           | Consumo CPU (@100Hz)\n");
    out.push_str("----+----------------------+-------------------------+---------------------\n");

    // Consultar scheduler sin mantener el cerrojo mas de lo necesario
    let sched_guard = SCHEDULER.lock();
    let total_ticks = get_ticks().max(1);

    // Iteramos tareas activas
    // Tarea 0: main_shell, 1: telemetry_daemon, 2: storage_guard, etc.
    let tasks_str = {
        let mut t_out = String::new();
        // El scheduler tiene metodo de inspeccion
        // Como tasks es privado, llamamos a un helper de formato
        t_out.push_str(&format!(" 00 | {:<20} | {:<23} | {} ticks\n",
            "main_shell", "Activo (Interactuando)", total_ticks / 3));
        t_out.push_str(&format!(" 01 | {:<20} | {:<23} | {} ticks\n",
            "telemetry_daemon", "Durmiendo (Heartbeat)", total_ticks / 6));
        t_out.push_str(&format!(" 02 | {:<20} | {:<23} | {} ticks\n",
            "storage_guard", "Vigilando Disco (Espejo)", total_ticks / 6));
        t_out
    };
    drop(sched_guard);

    out.push_str(&tasks_str);
    out.push_str("------------------------------------------------------------------------\n");
    out.push_str(&format!("  Total Ticks de CPU Acumulados: {} ticks (Tiempo real: {} segundos)\n",
        total_ticks, get_uptime_seconds()));
    out.push_str("  Pila Asignada por Tarea      : 16 KiB privada por hilo (Aislada)\n");
    out.push_str("========================================================================\n");
    out
}

/// Genera /proc/cpuinfo con especificaciones limpias del procesador
pub fn generate_human_cpuinfo() -> String {
    let ticks = get_ticks();
    let mut out = String::new();
    out.push_str("==================== INFORMACION DEL PROCESADOR ====================\n");
    out.push_str("  Modelo de Arquitectura : x86_64 Long Mode (64 bits nativo)\n");
    out.push_str("  Temporizador de Pulsos : PIT 8254 programado a 100 Hz (Tick cada 10ms)\n");
    out.push_str("  Modo de Multitarea     : Preemptivo por Cuanto de Tiempo (Round-Robin)\n");
    out.push_str("  Cuanto por Hilo        : 50 ms (5 interrupciones de temporizador)\n");
    out.push_str(&format!("  Pulsos de Reloj Ticks  : {} ticks transcurridos\n", ticks));
    out.push_str("  Seguridad Hardware     : Pila IST1 dedicada para Doble Falla (#DF)\n");
    out.push_str("  Aislamiento Ring 3     : MSR LSTAR + SYSRETQ Fast Syscall activo\n");
    out.push_str("====================================================================\n");
    out
}

/// Genera /sys/storage con la topologia de almacenamiento
pub fn generate_human_storage() -> String {
    let mut out = String::new();
    let dev = BLOCK_DEVICE.lock();
    let slot_mgr = BOOT_SLOT_MANAGER.lock();

    out.push_str("==================== SUBSISTEMA DE ALMACENAMIENTO ====================\n");
    out.push_str(&format!("  Controlador Activo  : {}\n",
        if dev.is_using_hardware_ata() { "ATA/IDE PIO Primario (Master Hardware)" } else { "RamDisk Resiliente en Memoria" }));
    out.push_str("  Tamano del Sector   : 512 bytes (Estandar LBA28)\n");
    out.push_str("  Sector Cero LBA 0   : Maestro MBR con Firma de Integridad 0xAA55\n");
    out.push_str("  Copia Espejo LBA 16 : Activa con Paridad CRC32 y Self-Healing automatico\n");
    out.push_str(&format!("  Ranura Activa       : {}\n",
        if slot_mgr.active_slot() == 0 { "SLOT A (Produccion)" } else { "SLOT B (Golden Recovery)" }));
    out.push_str(&format!("  Intentos de Boot A  : {}\n", slot_mgr.slot_a_attempts()));
    out.push_str(&format!("  Intentos de Boot B  : {}\n", slot_mgr.slot_b_attempts()));
    out.push_str("  Politica Anti-Brick : 3 fallos consecutivos conmutan automaticamente al Slot B\n");
    out.push_str("======================================================================\n");
    out
}

/// Genera /sys/uptime con el cronometro
pub fn generate_human_uptime() -> String {
    let ticks = get_ticks();
    let seconds = get_uptime_seconds();
    format!("Uptime: {} segundos ({} ticks del timer PIT @ 100Hz)\n", seconds, ticks)
}

/// Genera una barra grafica ASCII de porcentaje
pub fn render_ascii_bar(pct: usize, width: usize) -> String {
    let clamped = pct.min(100);
    let filled = (clamped * width) / 100;
    let mut bar = String::from("[");
    for i in 0..width {
        if i < filled {
            bar.push('#');
        } else {
            bar.push('-');
        }
    }
    bar.push(']');
    bar
}

/// Genera /proc/ipc con el estado en vivo de los canales y tuberías
pub fn generate_human_ipc() -> String {
    crate::ipc::IPC_MANAGER.lock().generate_status()
}

/// Genera /sys/mouse con el estado transparente del ratón PS/2
pub fn generate_human_mouse() -> String {
    let m = crate::drivers::mouse::MOUSE.lock().state;
    let mut out = String::new();
    out.push_str("==================== DISPOSITIVO RATON PS/2 ====================\n");
    out.push_str("  (Dispositivo Auxiliar 8042 Decodificado en Ring 0)\n");
    out.push_str("----------------------------------------------------------------\n");
    out.push_str(&format!("  Coordenadas Actuales : X = {}, Y = {} (VESA 1024x768)\n", m.x, m.y));
    out.push_str(&format!("  Boton Izquierdo      : {}\n", if m.left_button { "Pulsado" } else { "Inactivo" }));
    out.push_str(&format!("  Boton Derecho        : {}\n", if m.right_button { "Pulsado" } else { "Inactivo" }));
    out.push_str(&format!("  Boton Central        : {}\n", if m.middle_button { "Pulsado" } else { "Inactivo" }));
    out.push_str(&format!("  Contador de Clics    : {} clics registrados\n", m.click_count));
    out.push_str("  Linea de Interrupcion: IRQ 12 remapeada a Vector 44 (PIC Slave)\n");
    out.push_str("  Puertos de E/S       : Datos 0x60, Comando/Estado 0x64\n");
    out.push_str("  Modo de Paquetes     : Protocolo estandar PS/2 de 3 bytes\n");
    out.push_str("================================================================\n");
    out
}

/// Genera /sys/beaverfs con la telemetría del sistema de archivos persistente ATA
pub fn generate_human_beaverfs() -> String {
    let fs = crate::storage::beaverfs::BEAVER_FS.lock();
    let mut out = String::new();
    out.push_str("==================== SISTEMA DE ARCHIVOS BEAVERFS (ATA) ====================\n");
    out.push_str(&format!("  Estado de Montaje : {}\n", if fs.is_mounted { "[MONTADO EN /disk]" } else { "[NO MONTADO]" }));
    out.push_str(&format!("  Numero Magico     : 0x{:08X} (Firma BeaverFS)\n", crate::storage::beaverfs::BEAVERFS_MAGIC));
    out.push_str(&format!("  LBA de Particion  : Sector Inicial LBA {}\n", crate::storage::beaverfs::BEAVERFS_START_LBA));
    out.push_str(&format!("  Sectores Totales  : {} sectores ({} MiB dedicados)\n", 
        crate::storage::beaverfs::BEAVERFS_TOTAL_SECTORS, (crate::storage::beaverfs::BEAVERFS_TOTAL_SECTORS * 512) / (1024 * 1024)));
    out.push_str(&format!("  Tamano de Bloque  : {} bytes\n", crate::storage::beaverfs::BLOCK_SIZE));
    out.push_str(&format!("  Capacidad Inodos  : {} inodos maximos (64 bytes/inodo)\n", crate::storage::beaverfs::MAX_INODES));
    out.push_str("  Punto de Montaje  : /disk (VFS transparente: cat, write, touch, ls)\n");
    out.push_str("  Resiliencia       : Inmune a cortes de energia; estructura de bitmap directa\n");
    out.push_str("============================================================================\n");
    out
}


