// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Shell Network (Intel E1000 DMA, ICMP, ARP, DNS, HTTP) & BeaverSync Mesh Commands
// ============================================================================

extern crate alloc;

use alloc::format;
use crate::vga::VgaColor;
use crate::{kprintln, kprint_color};
use super::Shell;

pub fn try_execute(shell: &mut Shell, cmd: &str, args: &[&str]) -> bool {
    match cmd {
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
                kprintln!("Uso: ping <direccion_ipv4>  (ejemplo: ping 10.0.0.2)");
                return true;
            }
            let target_ip = match crate::drivers::net::Ipv4Addr::parse(args[0]) {
                Some(ip) => ip,
                None => {
                    kprintln!("Direccion IP invalida: '{}'", args[0]);
                    return true;
                }
            };

            let driver_ready = {
                crate::drivers::e1000::E1000.lock().initialized
            };
            if !driver_ready {
                kprintln!("Error: driver E1000 no inicializado. Tarjeta no detectada o DMA fallo.");
                kprintln!("Ejecuta 'ifconfig' para verificar el estado de la tarjeta de red.");
                return true;
            }

            kprintln!("PING {} 56(84) bytes de datos.", args[0]);
            let mut tx_count: u32 = 0;
            let mut rx_count: u32 = 0;

            for seq in 1u16..=4 {
                {
                    let mut n = crate::drivers::net::NET_INTERFACE.lock();
                    n.last_ping_replied = false;
                }

                crate::drivers::net::send_icmp_ping(target_ip, seq);
                tx_count += 1;

                let mut replied = false;
                let mut ttl_val: u8 = 0;
                for _attempt in 0..40_000 {
                    for _ in 0..1_000 { core::hint::spin_loop(); }
                    crate::drivers::net::net_poll_rx();
                    let n = crate::drivers::net::NET_INTERFACE.lock();
                    if n.last_ping_replied {
                        replied = true;
                        ttl_val = n.last_ping_ttl;
                        break;
                    }
                }

                if replied {
                    kprintln!("64 bytes desde {}: icmp_seq={} ttl={}", target_ip.to_string(), seq, ttl_val);
                    rx_count += 1;
                } else {
                    kprintln!("Tiempo de espera agotado para icmp_seq={}", seq);
                }
            }

            let lost = tx_count.saturating_sub(rx_count);
            let pct_loss = if tx_count > 0 { (lost * 100) / tx_count } else { 100 };
            kprintln!("--- estadisticas de ping {} ---", args[0]);
            kprintln!("{} paquetes transmitidos, {} recibidos, {}% perdida de paquetes",
                tx_count, rx_count, pct_loss);
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
                kprintln!("[DEMO][DHCP] Respuesta simulada - servidor DHCP externo opcional");
                kprintln!("[DHCP] Iniciando negociacion broadcast DHCP DISCOVER (RFC 2131)...");
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
                return true;
            }
            let domain = args[0];
            match crate::drivers::net::resolve_dns(domain) {
                Ok(ip) => {
                    kprintln!("[DEMO] Respuesta simulada de resolucion local");
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
                return true;
            }
            let url = args[0];
            if url.starts_with("https://") {
                kprintln!("[DEMO][CURL] Respuesta local simulada TLS 1.3");
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
                kprintln!("[DEMO][CURL] Respuesta HTTP local simulada");
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
                return true;
            }
            let url = args[0];
            let dest = if args.len() >= 3 && args[1] == "-O" {
                shell.resolve_path(args[2])
            } else {
                let filename = url.rsplit('/').next().unwrap_or("descarga.txt");
                shell.resolve_path(filename)
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

        "mesh" | "nodes" | "meshpeers" | "netdiscover" => {
            let subcmd = args.get(0).copied().unwrap_or("list");
            match subcmd {
                "announce" | "broadcast" => {
                    kprintln!("[MESH] Emitiendo anuncio de presencia UDP Broadcast a 255.255.255.255:4242...");
                    crate::drivers::mesh::send_mesh_announce_now();
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Datagrama BEAVMESH transmitido exitosamente.\n");
                }
                "query" | "discover" => {
                    kprintln!("[MESH] Consultando nodos en la subred local (UDP :4242)...");
                    crate::drivers::mesh::trigger_mesh_sync();
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Consulta global emitida.\n");
                }
                "cluster" | "skynet" | "genisys" => {
                    let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
                    let total_cores = mgr.total_cluster_cores();
                    let total_ram = mgr.total_cluster_ram_mb();
                    let online_n = mgr.online_count();
                    kprintln!("================== CLUSTER DE MALLA BEAVER OS (BEAVMESH) ==================");
                    kprintln!("  * Arquitectura:            Red Peer-to-Peer Descentralizada (UDP :4242)");
                    kprintln!("  * Cores Totales Agregados: {} nucleos x86_64", total_cores);
                    kprintln!("  * Memoria Total en Malla:  {} MB", total_ram);
                    kprintln!("  * Nodos Remotos Online:    {} ({} registrados)", online_n, mgr.get_nodes().len());
                    kprintln!("===========================================================================");
                }
                "sync" => {
                    crate::drivers::mesh::trigger_mesh_sync();
                    crate::drivers::audio::click_feedback();
                    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Query + Anuncio + /disk/notas.txt emitidos en la malla BeaverSync.\n");
                }
                _ => {
                    let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
                    kprintln!("================== RED DISTRIBUIDA BEAVER MESH (:4242) ==================");
                    kprintln!("Nombre Local: {} | Protocolo: BEAVMESH v1 | VFS Montado: /net", mgr.local_name);
                    kprintln!("-------------------------------------------------------------------------");
                    kprintln!("{:<16} {:<15} {:<6} {:<10} {:<8} {:<8} {:<10}", "Nodo", "IP", "Cores", "RAM", "Uptime", "Estado", "Compartido");
                    kprintln!("-------------------------------------------------------------------------");
                    if mgr.get_nodes().is_empty() {
                        kprintln!("  (sin nodos remotos descubiertos aun)");
                    }
                    for node in mgr.get_nodes() {
                        let shares_str = node.shares.join(",");
                        kprintln!("{:<16} {:<15} {:<6} {:<10} {:<8} {:<8} {:<10}",
                            node.name,
                            node.ip.to_string(),
                            node.cores,
                            format!("{} MB", node.ram_mb),
                            format!("{}s", node.uptime_secs),
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
            kprintln!("Sincronizando /disk/notas.txt + MSG_QUERY + MSG_ANNOUNCE por UDP :4242...");
            crate::drivers::mesh::trigger_mesh_sync();
            crate::drivers::audio::click_feedback();
            let mgr = crate::drivers::mesh::MESH_MANAGER.lock();
            let online_n = mgr.online_count();
            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Sincronizacion completada. Nodos remotos online: {}\n", online_n);
            for node in mgr.get_nodes() {
                kprintln!("  * {} ({}) -> Sinc: {}% | Archivo: {}", node.name, node.ip.to_string(), node.sync_percent, node.last_sync_file);
            }
            kprintln!("=================================================================");
        }

        _ => return false,
    }
    true
}

