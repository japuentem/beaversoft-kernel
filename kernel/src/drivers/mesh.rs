// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver Mesh Distributed Network Protocol (Zero-Config UDP 4242 /net)
//              Fase 5: descubrimiento real de nodos (sin nodos hardcodeados),
//              anuncio periodico, expiracion en 10 s, hostname por MAC.
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::sync::SpinLock;
use crate::drivers::net::{Ipv4Addr, send_udp_datagram};
use crate::drivers::timer::{get_uptime_seconds, get_ticks};

pub const MESH_PORT: u16 = 4242;
pub const MESH_MAGIC: &[u8; 8] = b"BEAVMESH";

pub const MSG_ANNOUNCE:  u8 = 0x01;
pub const MSG_QUERY:     u8 = 0x02;
pub const MSG_RESPONSE:  u8 = 0x03;
pub const MSG_HEARTBEAT: u8 = 0x04;
pub const MSG_FILE_SYNC: u8 = 0x05;
pub const MSG_FILE_ACK:  u8 = 0x06;

/// Ticks equivalentes a 10 segundos (PIT a 100 Hz → 100 ticks/s)
const NODE_TIMEOUT_TICKS: u64 = 1000;

/// Ticks equivalentes a 2 segundos entre anuncios
const ANNOUNCE_INTERVAL_TICKS: u64 = 200;

#[derive(Clone, Debug)]
pub struct MeshNode {
    pub name:           String,
    pub ip:             Ipv4Addr,
    pub cores:          u16,
    pub ram_mb:         u16,
    pub ram_free_mb:    u16,
    pub uptime_secs:    u64,
    pub os_version:     &'static str,
    pub latency_ms:     u16,
    pub shares:         Vec<String>,
    pub status:         &'static str,
    pub device_type:    &'static str,
    pub sync_percent:   u8,
    pub last_sync_file: &'static str,
    /// Marca de tiempo (ticks) del ultimo anuncio recibido
    pub last_seen:      u64,
}

pub struct MeshManager {
    pub local_name:             String,
    pub nodes:                  Vec<MeshNode>,
    pub announce_seq:           u32,
    pub sync_count:             u64,
    pub sync_logs:              Vec<String>,
    /// Ticks en que se envio el ultimo anuncio
    pub last_announce:          u64,
    /// Paquetes pendientes de enviar fuera del lock (dst_ip, payload)
    pub pending_tx:             Vec<(Ipv4Addr, Vec<u8>)>,
    /// Contenido recibido de /disk/notas.txt pendiente de escribir en VFS fuera del lock
    pub pending_received_notas: Option<Vec<u8>>,
}

impl MeshManager {
    pub fn new() -> Self {
        // Fase 5: sin nodos hardcodeados.
        // El nombre local se fija en init_mesh() derivandolo de la MAC.
        Self {
            local_name:             String::from("beaver-local"),
            nodes:                  Vec::new(),
            announce_seq:           0,
            sync_count:             0,
            sync_logs:              Vec::new(),
            last_announce:          0,
            pending_tx:             Vec::new(),
            pending_received_notas: None,
        }
    }

    fn push_log(&mut self, msg: String) {
        if self.sync_logs.len() >= 10 {
            self.sync_logs.remove(0);
        }
        self.sync_logs.push(msg);
    }

    // -----------------------------------------------------------------------
    // Informacion local
    // -----------------------------------------------------------------------

    /// Calcula la potencia computacional agregada de la malla (Cores de CPU totales)
    pub fn total_cluster_cores(&self) -> u32 {
        let local_cores = 1u32; // Beaver OS reporta 1 BSP real (Fase 3)
        let remote_cores: u32 = self.nodes.iter()
            .filter(|n| n.status == "Online")
            .map(|n| n.cores as u32)
            .sum();
        local_cores + remote_cores
    }

    /// Calcula la memoria RAM total distribuida disponible en toda la red
    pub fn total_cluster_ram_mb(&self) -> u32 {
        let local_frames = crate::memory::frame_allocator::FRAME_ALLOCATOR.total_usable_frames();
        let local_ram = ((local_frames * 4096) / (1024 * 1024)) as u32;
        let remote_ram: u32 = self.nodes.iter()
            .filter(|n| n.status == "Online")
            .map(|n| n.ram_mb as u32)
            .sum();
        local_ram + remote_ram
    }

    // -----------------------------------------------------------------------
    // Construccion de paquetes
    // -----------------------------------------------------------------------

    /// Construye el paquete de anuncio UDP Beaver Mesh
    pub fn build_announce_packet(&self) -> Vec<u8> {
        let mut pkt = Vec::with_capacity(64);
        pkt.extend_from_slice(MESH_MAGIC);  // [0..8]  Magic
        pkt.push(MSG_ANNOUNCE);             // [8]     Tipo
        pkt.push(1);                        // [9]     Version de protocolo
        pkt.extend_from_slice(&self.announce_seq.to_be_bytes()); // [10..14] Seq

        // [14] Longitud del nombre local + nombre
        let name_bytes = self.local_name.as_bytes();
        pkt.push(name_bytes.len() as u8);
        pkt.extend_from_slice(name_bytes);

        // Hardware real: BSP 1 core, RAM reportada por frame allocator
        let cores: u16 = 1;
        let ram_mb: u16 = {
            let total = crate::memory::frame_allocator::FRAME_ALLOCATOR.total_usable_frames();
            let mb = (total * 4096) / (1024 * 1024);
            mb as u16
        };
        pkt.extend_from_slice(&cores.to_be_bytes());
        pkt.extend_from_slice(&ram_mb.to_be_bytes());
        let uptime = get_uptime_seconds();
        pkt.extend_from_slice(&uptime.to_be_bytes());
        pkt
    }

    /// Construye el paquete de consulta global (MSG_QUERY)
    pub fn build_query_packet(&self) -> Vec<u8> {
        let mut pkt = Vec::with_capacity(16);
        pkt.extend_from_slice(MESH_MAGIC);
        pkt.push(MSG_QUERY);
        pkt.push(1);
        pkt
    }

    /// Construye el paquete de sincronizacion de `/disk/notas.txt` (MSG_FILE_SYNC)
    pub fn build_file_sync_packet(&self, seq: u32, content: &[u8]) -> Vec<u8> {
        let clamped_len = content.len().min(1200);
        let name_bytes = self.local_name.as_bytes();
        let mut pkt = Vec::with_capacity(20 + name_bytes.len() + clamped_len);
        pkt.extend_from_slice(MESH_MAGIC);              // [0..8]
        pkt.push(MSG_FILE_SYNC);                        // [8]
        pkt.push(1);                                    // [9] version
        pkt.extend_from_slice(&seq.to_be_bytes());      // [10..14] seq
        pkt.push(name_bytes.len() as u8);               // [14] name_len
        pkt.extend_from_slice(name_bytes);              // [15..15+name_len]
        pkt.extend_from_slice(&(clamped_len as u16).to_be_bytes()); // [after_name..+2]
        pkt.extend_from_slice(&content[..clamped_len]);
        pkt
    }

    /// Construye el paquete de confirmacion de archivo sincronizado (MSG_FILE_ACK)
    pub fn build_file_ack_packet(&self, seq: u32, acked_bytes: u16) -> Vec<u8> {
        let name_bytes = self.local_name.as_bytes();
        let mut pkt = Vec::with_capacity(20 + name_bytes.len());
        pkt.extend_from_slice(MESH_MAGIC);              // [0..8]
        pkt.push(MSG_FILE_ACK);                         // [8]
        pkt.push(1);                                    // [9] version
        pkt.extend_from_slice(&seq.to_be_bytes());      // [10..14] seq
        pkt.push(name_bytes.len() as u8);               // [14] name_len
        pkt.extend_from_slice(name_bytes);              // [15..15+name_len]
        pkt.extend_from_slice(&acked_bytes.to_be_bytes());
        pkt
    }

    /// Prepara un anuncio incrementando secuencia y devolviendo el datagrama
    pub fn prepare_announce(&mut self) -> Vec<u8> {
        self.announce_seq = self.announce_seq.wrapping_add(1);
        self.last_announce = get_ticks();
        self.build_announce_packet()
    }

    /// Emite un datagrama UDP Broadcast (255.255.255.255:4242) anunciando este nodo.
    /// NOTA: no llamar mientras NET_INTERFACE este bloqueado.
    pub fn send_announce(&mut self) {
        let pkt = self.prepare_announce();
        send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &pkt);
    }

    /// Emite una consulta global (MSG_QUERY) pidiendo que todos se identifiquen.
    /// NOTA: no llamar mientras NET_INTERFACE este bloqueado.
    pub fn query_network(&mut self) {
        let pkt = self.build_query_packet();
        send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &pkt);
    }

    // -----------------------------------------------------------------------
    // Recepcion de paquetes (sin tocar NET_INTERFACE para evitar deadlock)
    // -----------------------------------------------------------------------

    /// Procesa datagramas entrantes en el puerto 4242.
    /// Devuelve `true` si hay acciones diferidas (anuncio, escritura VFS o envio de ACK)
    /// que deben ejecutarse mediante `flush_mesh_pending_actions()` fuera del lock de red.
    pub fn handle_packet(&mut self, src_ip: Ipv4Addr, _src_port: u16, payload: &[u8]) -> bool {
        if payload.len() < 10 || &payload[0..8] != MESH_MAGIC {
            return false;
        }

        let msg_type = payload[8];
        match msg_type {
            MSG_ANNOUNCE | MSG_RESPONSE => {
                if payload.len() < 15 {
                    return false;
                }
                let name_len = payload[14] as usize;
                if payload.len() < 15 + name_len {
                    return false;
                }
                let name_bytes = &payload[15..15 + name_len];
                let name = match core::str::from_utf8(name_bytes) {
                    Ok(s) => String::from(s),
                    Err(_) => return false,
                };

                // Ignorar anuncios con nuestro propio nombre local
                if name == self.local_name {
                    return false;
                }

                // Parsear cores, ram_mb, uptime si el paquete es suficientemente largo
                let after_name = 15 + name_len;
                let cores = if payload.len() >= after_name + 2 {
                    u16::from_be_bytes([payload[after_name], payload[after_name + 1]])
                } else {
                    1
                };
                let ram_mb = if payload.len() >= after_name + 4 {
                    u16::from_be_bytes([payload[after_name + 2], payload[after_name + 3]])
                } else {
                    0
                };
                let uptime_secs = if payload.len() >= after_name + 12 {
                    u64::from_be_bytes([
                        payload[after_name + 4], payload[after_name + 5],
                        payload[after_name + 6], payload[after_name + 7],
                        payload[after_name + 8], payload[after_name + 9],
                        payload[after_name + 10], payload[after_name + 11],
                    ])
                } else {
                    0
                };

                let now = get_ticks();

                // Actualizar nodo existente o insertar nuevo
                if let Some(existing) = self.nodes.iter_mut().find(|n| n.ip.0 == src_ip.0) {
                    let was_offline = existing.status != "Online";
                    existing.name         = name.clone();
                    existing.cores        = cores;
                    existing.ram_mb       = ram_mb;
                    existing.ram_free_mb  = ram_mb;
                    existing.uptime_secs  = uptime_secs;
                    existing.status       = "Online";
                    existing.last_seen    = now;
                    if was_offline {
                        self.push_log(format!(
                            "[MESH] Nodo {} ({}) reconectado ({} MB RAM).",
                            name, src_ip.to_string(), ram_mb
                        ));
                    }
                } else {
                    self.push_log(format!(
                        "[MESH] Nuevo nodo descubierto: {} en {} ({} core, {} MB).",
                        name, src_ip.to_string(), cores, ram_mb
                    ));
                    self.nodes.push(MeshNode {
                        name,
                        ip: src_ip,
                        cores,
                        ram_mb,
                        ram_free_mb: ram_mb,
                        uptime_secs,
                        os_version: "Beaver OS v0.8.0",
                        latency_ms: 1,
                        shares: vec![String::from("disk"), String::from("sys")],
                        status: "Online",
                        device_type: "Nodo Beaver",
                        sync_percent: 0,
                        last_sync_file: "-",
                        last_seen: now,
                    });
                }
                false
            }
            MSG_QUERY => {
                // Encolar anuncio de respuesta para enviar fuera del lock de red
                let pkt = self.prepare_announce();
                self.pending_tx.push((Ipv4Addr::new(255, 255, 255, 255), pkt));
                true
            }
            MSG_FILE_SYNC => {
                if payload.len() < 15 {
                    return false;
                }
                let seq = u32::from_be_bytes([payload[10], payload[11], payload[12], payload[13]]);
                let name_len = payload[14] as usize;
                let after_name = 15 + name_len;
                if payload.len() < after_name + 2 {
                    return false;
                }
                let sender_name = match core::str::from_utf8(&payload[15..after_name]) {
                    Ok(s) => String::from(s),
                    Err(_) => return false,
                };
                if sender_name == self.local_name {
                    return false;
                }
                let content_len = u16::from_be_bytes([payload[after_name], payload[after_name + 1]]) as usize;
                if payload.len() < after_name + 2 + content_len {
                    return false;
                }
                let content_bytes = payload[after_name + 2..after_name + 2 + content_len].to_vec();
                let now = get_ticks();

                if let Some(existing) = self.nodes.iter_mut().find(|n| n.ip.0 == src_ip.0) {
                    existing.status = "Online";
                    existing.last_seen = now;
                    existing.sync_percent = 100;
                    existing.last_sync_file = "/disk/notas.txt";
                }
                self.pending_received_notas = Some(content_bytes);
                let ack_pkt = self.build_file_ack_packet(seq, content_len as u16);
                self.pending_tx.push((src_ip, ack_pkt));
                self.push_log(format!(
                    "[SYNC] /disk/notas.txt recibido de {} ({} B, seq #{}) -> ACK enviado.",
                    sender_name, content_len, seq
                ));
                true
            }
            MSG_FILE_ACK => {
                if payload.len() < 15 {
                    return false;
                }
                let seq = u32::from_be_bytes([payload[10], payload[11], payload[12], payload[13]]);
                let name_len = payload[14] as usize;
                let after_name = 15 + name_len;
                if payload.len() < after_name + 2 {
                    return false;
                }
                let responder_name = match core::str::from_utf8(&payload[15..after_name]) {
                    Ok(s) => String::from(s),
                    Err(_) => return false,
                };
                if responder_name == self.local_name {
                    return false;
                }
                let acked_bytes = u16::from_be_bytes([payload[after_name], payload[after_name + 1]]);
                let now = get_ticks();

                if let Some(existing) = self.nodes.iter_mut().find(|n| n.ip.0 == src_ip.0) {
                    existing.status = "Online";
                    existing.last_seen = now;
                    existing.sync_percent = 100;
                    existing.last_sync_file = "/disk/notas.txt";
                }
                self.push_log(format!(
                    "[SYNC] ACK de {} ({}): /disk/notas.txt (100% confirmado, {} B, seq #{}).",
                    responder_name, src_ip.to_string(), acked_bytes, seq
                ));
                false
            }
            _ => false,
        }
    }

    // -----------------------------------------------------------------------
    // Mantenimiento periodico (sin enviar paquetes dentro del lock)
    // -----------------------------------------------------------------------

    /// Actualiza expiracion de nodos (10 s → Offline, 30 s → eliminar) y
    /// devuelve `Some(pkt)` si corresponde emitir el anuncio periodico (cada 2 s).
    pub fn tick_state(&mut self) -> Option<Vec<u8>> {
        let now = get_ticks();

        // Expirar nodos sin anuncio en 10 s
        let mut expired_names: Vec<(String, Ipv4Addr)> = Vec::new();
        for node in self.nodes.iter_mut() {
            if node.status == "Online" && now.saturating_sub(node.last_seen) > NODE_TIMEOUT_TICKS {
                node.status = "Offline";
                expired_names.push((node.name.clone(), node.ip));
            }
        }
        for (ename, eip) in expired_names {
            self.push_log(format!(
                "[MESH] Nodo {} ({}) sin respuesta (>10s) -> Offline.",
                ename, eip.to_string()
            ));
        }

        // Eliminar nodos Offline que llevan mas de 30 s
        self.nodes.retain(|n| {
            n.status == "Online" || now.saturating_sub(n.last_seen) < NODE_TIMEOUT_TICKS * 3
        });

        // Verificar si toca enviar anuncio cada 2 s
        if self.last_announce == 0 || now.saturating_sub(self.last_announce) >= ANNOUNCE_INTERVAL_TICKS {
            Some(self.prepare_announce())
        } else {
            None
        }
    }

    // -----------------------------------------------------------------------
    // Sincronizacion manual (boton GUI y comando sync)
    // -----------------------------------------------------------------------

    /// Registra la sincronizacion manual y prepara los paquetes (query, announce, file_sync)
    pub fn prepare_sync_packets_with_file(&mut self, notas_content: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        self.sync_count += 1;
        let seq = self.sync_count as u32;
        let query_pkt = self.build_query_packet();
        let announce_pkt = self.prepare_announce();
        let file_pkt = self.build_file_sync_packet(seq, notas_content);
        let n = self.nodes.iter().filter(|x| x.status == "Online").count();
        self.push_log(format!(
            "[SYNC #{}] Enviando /disk/notas.txt ({} B) + QUERY a {} nodo(s) online.",
            self.sync_count, notas_content.len(), n
        ));
        (query_pkt, announce_pkt, file_pkt)
    }

    /// Dispara una query + anuncio inmediato (para llamadas desde shell cuando NET_INTERFACE esta libre)
    pub fn trigger_sync(&mut self) {
        self.sync_count += 1;
        let q = self.build_query_packet();
        let a = self.prepare_announce();
        send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &q);
        send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &a);
    }

    // -----------------------------------------------------------------------
    // Consultas
    // -----------------------------------------------------------------------

    pub fn get_nodes(&self) -> &[MeshNode] {
        &self.nodes
    }

    pub fn get_node(&self, name: &str) -> Option<&MeshNode> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// Cuenta nodos con estado Online
    pub fn online_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.status == "Online").count()
    }

    /// Lectura de archivos remotos sincronizados en VFS /net
    pub fn read_node_file(&self, node_name: &str, subpath: &str) -> Result<String, &'static str> {
        let node = self.get_node(node_name).ok_or("Nodo no encontrado en la red mesh")?;
        let clean = subpath.trim_start_matches('/');
        if clean == "status.txt" || clean == "status" || clean.is_empty() {
            return Ok(format!(
                "=== NODO BEAVER MESH: {} ===\nIP: {}\nEstado: {}\nOS: {}\nCores: {}\nRAM: {} MB\nUptime: {} s\nSincronizacion: {}% ({})\n",
                node.name, node.ip.to_string(), node.status,
                node.os_version, node.cores, node.ram_mb, node.uptime_secs,
                node.sync_percent, node.last_sync_file
            ));
        }
        if clean == "disk/notas.txt" || clean == "notas.txt" {
            let local_copy = crate::fs::vfs::VFS.lock()
                .read_file("/disk/notas.txt", 0, 0)
                .unwrap_or_else(|_| String::from("(sin contenido)"));
            return Ok(format!(
                "=== NOTAS SINCRONIZADAS CON {} ({}%) ===\n{}",
                node.name, node.sync_percent, local_copy
            ));
        }
        Err("Archivo remoto no encontrado")
    }
}

// ---------------------------------------------------------------------------
// Instancia global y helpers de modulo (desacoplados del lock para evitar deadlock)
// ---------------------------------------------------------------------------

pub static MESH_MANAGER: SpinLock<MeshManager> = SpinLock::new(MeshManager {
    local_name:             String::new(),
    nodes:                  Vec::new(),
    announce_seq:           0,
    sync_count:             0,
    sync_logs:              Vec::new(),
    last_announce:          0,
    pending_tx:             Vec::new(),
    pending_received_notas: None,
});

/// Inicializa la malla derivando el hostname de la MAC real
pub fn init_mesh() {
    let mac = crate::drivers::net::NET_INTERFACE.lock().mac_address;
    // Hostname: beaver-XXYY donde XX e YY son los bytes 4 y 5 de la MAC en hex
    let hostname = format!("beaver-{:02x}{:02x}", mac[4], mac[5]);
    let mut mgr = MESH_MANAGER.lock();
    *mgr = MeshManager::new();
    mgr.local_name = hostname.clone();
    mgr.push_log(format!("[MESH] Nodo local inicializado como '{}' (UDP :4242).", hostname));
}

/// Envia un anuncio inmediato desacoplando MESH_MANAGER de NET_INTERFACE
pub fn send_mesh_announce_now() {
    let pkt = if let Some(mut mgr) = MESH_MANAGER.try_lock() {
        Some(mgr.prepare_announce())
    } else {
        None
    };
    if let Some(p) = pkt {
        send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &p);
    }
}

/// Procesa acciones diferidas encoladas durante `handle_packet` (fuera de NET_INTERFACE y MESH_MANAGER locks)
pub fn flush_mesh_pending_actions() {
    let (tx_list, rx_notas) = if let Some(mut mgr) = MESH_MANAGER.try_lock() {
        let tx = core::mem::take(&mut mgr.pending_tx);
        let notas = mgr.pending_received_notas.take();
        (tx, notas)
    } else {
        return;
    };

    if let Some(notas_bytes) = rx_notas {
        let _ = crate::fs::vfs::VFS.lock().create_file("/disk/notas.txt", &notas_bytes, 0, 0);
        if let Some(mut note) = crate::gui::note::BEAVER_NOTE.try_lock() {
            let _ = note.load_from_disk();
        }
    }

    for (dst_ip, pkt) in tx_list {
        send_udp_datagram(dst_ip, MESH_PORT, MESH_PORT, &pkt);
    }
}

/// Dispara la sincronizacion completa de la malla (query + announce + `/disk/notas.txt` con ACK)
pub fn trigger_mesh_sync() {
    let notas_bytes = crate::fs::vfs::VFS.lock()
        .read_file_bytes("/disk/notas.txt", 0, 0)
        .unwrap_or_default();

    let (query_pkt, announce_pkt, file_pkt) = {
        let mut mgr = MESH_MANAGER.lock();
        mgr.prepare_sync_packets_with_file(&notas_bytes)
    };
    send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &query_pkt);
    send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &announce_pkt);
    send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &file_pkt);

    // Sondear RX brevemente para recoger MSG_FILE_ACK de nodos activos en la subred
    for _ in 0..8_000 {
        for _ in 0..500 {
            core::hint::spin_loop();
        }
        crate::drivers::net::net_poll_rx();
    }
}

/// Llamar desde el bucle principal del kernel para mantener la malla viva
pub fn mesh_tick() {
    let pkt = if let Some(mut mgr) = MESH_MANAGER.try_lock() {
        mgr.tick_state()
    } else {
        None
    };
    if let Some(p) = pkt {
        send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &p);
    }
}
