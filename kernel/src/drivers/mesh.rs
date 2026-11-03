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

pub const MSG_ANNOUNCE: u8 = 0x01;
pub const MSG_QUERY:    u8 = 0x02;
pub const MSG_RESPONSE: u8 = 0x03;
pub const MSG_HEARTBEAT:u8 = 0x04;

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
    pub local_name:     String,
    pub nodes:          Vec<MeshNode>,
    pub announce_seq:   u32,
    pub sync_count:     u64,
    pub sync_logs:      Vec<String>,
    /// Ticks en que se envio el ultimo anuncio
    pub last_announce:  u64,
}

impl MeshManager {
    pub fn new() -> Self {
        // Fase 5: sin nodos hardcodeados.
        // El nombre local se fija en init_mesh() derivandolo de la MAC.
        Self {
            local_name:    String::from("beaver-local"),
            nodes:         Vec::new(),
            announce_seq:  0,
            sync_count:    0,
            sync_logs:     Vec::new(),
            last_announce: 0,
        }
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
        // La RAM local real proviene del frame allocator; usamos 0 para no falsear
        let remote_ram: u32 = self.nodes.iter()
            .filter(|n| n.status == "Online")
            .map(|n| n.ram_mb as u32)
            .sum();
        remote_ram
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

        // Hardware real: BSP 1 core, RAM reportada por frame allocator (usamos 0 si no disponible)
        let cores: u16 = 1;
        let ram_mb: u16 = {
            // Leer RAM usable del frame allocator (acceso directo, no lock externo)
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

    /// Emite un datagrama UDP Broadcast (255.255.255.255:4242) anunciando este nodo
    pub fn send_announce(&mut self) {
        self.announce_seq = self.announce_seq.wrapping_add(1);
        self.last_announce = get_ticks();
        let pkt = self.build_announce_packet();
        send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &pkt);
    }

    /// Emite una consulta global (MSG_QUERY) pidiendo que todos se identifiquen
    pub fn query_network(&mut self) {
        let mut pkt = Vec::with_capacity(16);
        pkt.extend_from_slice(MESH_MAGIC);
        pkt.push(MSG_QUERY);
        pkt.push(1);
        send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &pkt);
    }

    // -----------------------------------------------------------------------
    // Recepcion de paquetes
    // -----------------------------------------------------------------------

    /// Procesa datagramas entrantes en el puerto 4242
    pub fn handle_packet(&mut self, src_ip: Ipv4Addr, _src_port: u16, payload: &[u8]) {
        if payload.len() < 10 || &payload[0..8] != MESH_MAGIC {
            return;
        }

        let msg_type = payload[8];
        match msg_type {
            MSG_ANNOUNCE | MSG_RESPONSE => {
                // Formato de anuncio:
                // [0..8]  magic
                // [8]     tipo
                // [9]     version
                // [10..14] seq (u32 BE)
                // [14]    len_name
                // [15..15+len_name] name
                // [15+N..17+N] cores (u16 BE)
                // [17+N..19+N] ram_mb (u16 BE)
                // [19+N..27+N] uptime (u64 BE)
                if payload.len() < 15 {
                    return;
                }
                let name_len = payload[14] as usize;
                if payload.len() < 15 + name_len {
                    return;
                }
                let name_bytes = &payload[15..15 + name_len];
                let name = match core::str::from_utf8(name_bytes) {
                    Ok(s) => String::from(s),
                    Err(_) => return,
                };

                // Ignorar anuncios propios (misma IP que nosotros)
                let my_ip = crate::drivers::net::NET_INTERFACE.lock().ip_address;
                if src_ip.0 == my_ip.0 {
                    return;
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
                    existing.name         = name;
                    existing.cores        = cores;
                    existing.ram_mb       = ram_mb;
                    existing.uptime_secs  = uptime_secs;
                    existing.status       = "Online";
                    existing.last_seen    = now;
                } else {
                    self.nodes.push(MeshNode {
                        name,
                        ip: src_ip,
                        cores,
                        ram_mb,
                        ram_free_mb: 0,
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
            }
            MSG_QUERY => {
                // Alguien pregunta — responder con nuestro anuncio
                self.send_announce();
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Mantenimiento periodico
    // -----------------------------------------------------------------------

    /// Llama desde el bucle principal del kernel (cada tick).
    /// 1. Expira nodos sin anuncio en 10 s.
    /// 2. Envia anuncio propio cada 2 s.
    pub fn tick(&mut self) {
        let now = get_ticks();

        // Expirar nodos viejos
        for node in self.nodes.iter_mut() {
            if node.status == "Online" && now.saturating_sub(node.last_seen) > NODE_TIMEOUT_TICKS {
                node.status = "Offline";
            }
        }
        // Eliminar nodos Offline que llevan mas de 30 s (sin anuncio para ser conservadores)
        self.nodes.retain(|n| {
            n.status == "Online" || now.saturating_sub(n.last_seen) < NODE_TIMEOUT_TICKS * 3
        });

        // Enviar anuncio cada 2 s
        if now.saturating_sub(self.last_announce) >= ANNOUNCE_INTERVAL_TICKS {
            self.send_announce();
        }
    }

    // -----------------------------------------------------------------------
    // Sincronizacion manual (boton GUI)
    // -----------------------------------------------------------------------

    /// Dispara una query + anuncio inmediato (boton "Sincronizar Malla Ahora")
    pub fn trigger_sync(&mut self) {
        self.sync_count += 1;
        self.query_network();
        self.send_announce();
        if self.sync_logs.len() > 10 {
            self.sync_logs.remove(0);
        }
        let n = self.nodes.iter().filter(|x| x.status == "Online").count();
        self.sync_logs.push(format!(
            "[SYNC #{}] Query enviada. Nodos online: {}.",
            self.sync_count, n
        ));
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

    /// Simulacion de lectura de archivos remotos (mantiene compatibilidad con VFS /net)
    pub fn read_node_file(&self, node_name: &str, subpath: &str) -> Result<String, &'static str> {
        let node = self.get_node(node_name).ok_or("Nodo no encontrado en la red mesh")?;
        let clean = subpath.trim_start_matches('/');
        if clean == "status.txt" || clean == "status" || clean.is_empty() {
            return Ok(format!(
                "=== NODO BEAVER MESH: {} ===\nIP: {}\nEstado: {}\nOS: {}\nCores: {}\nRAM: {} MB\nUptime: {} s\n",
                node.name, node.ip.to_string(), node.status,
                node.os_version, node.cores, node.ram_mb, node.uptime_secs
            ));
        }
        if clean == "disk/notas.txt" {
            return Ok(format!(
                "=== NOTAS DE {} ===\n(archivo compartido via BeaverSync)\n",
                node.name
            ));
        }
        Err("Archivo remoto no encontrado")
    }
}

// ---------------------------------------------------------------------------
// Instancia global y helpers de modulo
// ---------------------------------------------------------------------------

pub static MESH_MANAGER: SpinLock<MeshManager> = SpinLock::new(MeshManager {
    local_name:    String::new(),
    nodes:         Vec::new(),
    announce_seq:  0,
    sync_count:    0,
    sync_logs:     Vec::new(),
    last_announce: 0,
});

/// Inicializa la malla derivando el hostname de la MAC real
pub fn init_mesh() {
    let mac = crate::drivers::net::NET_INTERFACE.lock().mac_address;
    // Hostname: beaver-XXYY donde XX e YY son los bytes 4 y 5 de la MAC en hex
    let hostname = format!("beaver-{:02x}{:02x}", mac[4], mac[5]);
    let mut mgr = MESH_MANAGER.lock();
    *mgr = MeshManager::new();
    mgr.local_name = hostname;
}

/// Dispara la sincronizacion de la malla desacoplando el lock de la transmision de red
pub fn trigger_mesh_sync() {
    let pkt = {
        let mut mgr = MESH_MANAGER.lock();
        mgr.trigger_sync();
        mgr.build_announce_packet()
    };
    send_udp_datagram(Ipv4Addr::new(255, 255, 255, 255), MESH_PORT, MESH_PORT, &pkt);
}

/// Llamar desde el bucle principal del kernel para mantener la malla viva
pub fn mesh_tick() {
    if let Some(mut mgr) = MESH_MANAGER.try_lock() {
        mgr.tick();
    }
}
