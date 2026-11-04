// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Bare-Metal Network Stack (Intel E1000 PCI, Ethernet II, ARP, IPv4 & ICMP Ping)
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use crate::drivers::pci::{pci_read_config_u32, pci_write_config_u32};
use crate::sync::SpinLock;

// IDs de PCI para Intel PRO/1000 (E1000)
pub const INTEL_VENDOR_ID: u16 = 0x8086;
pub const E1000_DEV_82540EM: u16 = 0x100E; // Tarjeta estándar en QEMU y VirtualBox
pub const E1000_DEV_82545EM: u16 = 0x100F;
pub const E1000_DEV_82543GC: u16 = 0x1004;

// Registros de control de E1000 (Offsets MMIO)
pub const REG_CTRL: u32 = 0x0000;
pub const REG_STATUS: u32 = 0x0008;
pub const REG_EERD: u32 = 0x0014;
pub const REG_RAL: u32 = 0x5400; // Receive Address Low
pub const REG_RAH: u32 = 0x5404; // Receive Address High
pub const REG_RCTL: u32 = 0x0100; // Receive Control
pub const REG_TCTL: u32 = 0x0400; // Transmit Control

// EtherTypes
pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

// Protocolos IPv4
pub const IP_PROTO_ICMP: u8 = 1;
pub const IP_PROTO_TCP: u8 = 6;
pub const IP_PROTO_UDP: u8 = 17;

// Opcodes ARP
pub const ARP_REQUEST: u16 = 1;
pub const ARP_REPLY: u16 = 2;

// ICMP Types
pub const ICMP_ECHO_REPLY: u8 = 0;
pub const ICMP_ECHO_REQUEST: u8 = 8;

#[derive(Debug, Clone, Copy)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 4 {
            return None;
        }
        let a = parts[0].parse::<u8>().ok()?;
        let b = parts[1].parse::<u8>().ok()?;
        let c = parts[2].parse::<u8>().ok()?;
        let d = parts[3].parse::<u8>().ok()?;
        Some(Self([a, b, c, d]))
    }

    pub fn to_string(&self) -> String {
        format!("{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArpEntry {
    pub ip: Ipv4Addr,
    pub mac: [u8; 6],
    pub valid: bool,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub content_type: String,
    pub content_length: usize,
    pub body: String,
    pub raw_body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DhcpLease {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub dns: Ipv4Addr,
    pub server_ip: Ipv4Addr,
    pub lease_time_secs: u32,
}

pub struct NetworkInterface {
    pub card_detected: bool,
    pub card_name: &'static str,
    pub mmio_base: u64,
    pub mac_address: [u8; 6],
    pub ip_address: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub dns_server: Ipv4Addr,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub link_up: bool,
    pub arp_table: [ArpEntry; 8],
    pub last_ping_replied: bool,
    pub last_ping_ttl: u8,
    pub last_ping_seq: u16,
}

pub static NET_INTERFACE: SpinLock<NetworkInterface> = SpinLock::new(NetworkInterface {
    card_detected: false,
    card_name: "Loopback Virtual (Sin tarjeta física)",
    mmio_base: 0,
    mac_address: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56], // Dirección MAC Beaver / QEMU
    ip_address: Ipv4Addr::new(10, 0, 2, 15),
    netmask: Ipv4Addr::new(255, 255, 255, 0),
    gateway: Ipv4Addr::new(10, 0, 2, 2),
    dns_server: Ipv4Addr::new(10, 0, 2, 3), // Servidor DNS QEMU / Virtual Router
    rx_packets: 0,
    tx_packets: 0,
    rx_bytes: 0,
    tx_bytes: 0,
    link_up: true,
    arp_table: [ArpEntry {
        ip: Ipv4Addr::new(0, 0, 0, 0),
        mac: [0; 6],
        valid: false,
    }; 8],
    last_ping_replied: false,
    last_ping_ttl: 64,
    last_ping_seq: 0,
});

pub static DHCP_PENDING_LEASE: SpinLock<Option<DhcpLease>> = SpinLock::new(None);
pub static DNS_PENDING_RESPONSE: SpinLock<Option<(u16, Ipv4Addr)>> = SpinLock::new(None);

/// Inicializa el controlador de red buscando la tarjeta Intel PRO/1000 en el bus PCI
pub fn init() {
    let mut net = NET_INTERFACE.lock();

    for bus in 0..=4 {
        for device in 0..32 {
            let id_reg = pci_read_config_u32(bus, device, 0, 0x00);
            let vendor_id = (id_reg & 0xFFFF) as u16;
            if vendor_id != INTEL_VENDOR_ID {
                continue;
            }
            let device_id = ((id_reg >> 16) & 0xFFFF) as u16;

            if device_id == E1000_DEV_82540EM || device_id == E1000_DEV_82545EM || device_id == E1000_DEV_82543GC {
                // Habilitar Bus Master y Memory Space en PCI Command Register (Offset 0x04)
                let cmd = pci_read_config_u32(bus, device, 0, 0x04);
                pci_write_config_u32(bus, device, 0, 0x04, cmd | 0x0007);

                // Leer BAR0 (Dirección base MMIO)
                let bar0 = pci_read_config_u32(bus, device, 0, 0x10) & 0xFFFFFFF0;

                net.card_detected = true;
                net.card_name = "Intel PRO/1000 Gigabit Ethernet (82540EM)";
                net.mmio_base = bar0 as u64;

                // Leer MAC address física de registros RAL/RAH si MMIO está disponible
                if bar0 != 0 {
                    let mmio_ptr = bar0 as *const u32;
                    unsafe {
                        let ral = mmio_ptr.add((REG_RAL / 4) as usize).read_volatile();
                        let rah = mmio_ptr.add((REG_RAH / 4) as usize).read_volatile();

                        if (rah & (1 << 31)) != 0 || ral != 0 {
                            net.mac_address[0] = (ral & 0xFF) as u8;
                            net.mac_address[1] = ((ral >> 8) & 0xFF) as u8;
                            net.mac_address[2] = ((ral >> 16) & 0xFF) as u8;
                            net.mac_address[3] = ((ral >> 24) & 0xFF) as u8;
                            net.mac_address[4] = (rah & 0xFF) as u8;
                            net.mac_address[5] = ((rah >> 8) & 0xFF) as u8;
                        }
                    }

                    // Asignar IP estatica segun MAC (para banco de pruebas con dos QEMU)
                    // MAC ...:BE:A0:01 -> 10.0.0.1  (Nodo A)
                    // MAC ...:BE:A0:02 -> 10.0.0.2  (Nodo B)
                    let mac4 = net.mac_address[4];
                    let mac5 = net.mac_address[5];
                    if net.mac_address[3] == 0xBE && mac4 == 0xA0 {
                        net.ip_address  = Ipv4Addr::new(10, 0, 0, mac5);
                        net.netmask     = Ipv4Addr::new(255, 255, 255, 0);
                        net.gateway     = Ipv4Addr::new(10, 0, 0, 1);
                        net.dns_server  = Ipv4Addr::new(10, 0, 0, 1);
                        net.link_up     = true;
                    }

                    // Inicializar E1000 driver DMA (Fase 4)
                    drop(net); // liberar lock antes de llamar al driver (evitar deadlock)
                    let init_ok = crate::drivers::e1000::E1000.lock().init(bar0 as u64);
                    if !init_ok {
                        crate::kprintln!("[E1000] ADVERTENCIA: init DMA fallo (RAM insuficiente?)");
                    } else {
                        crate::kprintln!("[E1000] Driver DMA inicializado. TX/RX rings listos.");
                    }
                    return;
                }

                // Registrar Gateway predeterminado en tabla ARP
                net.arp_table[0] = ArpEntry {
                    ip: net.gateway,
                    mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x02], // MAC típica de gateway QEMU
                    valid: true,
                };

                return;
            }
        }
    }

    // Configuración fallback en caso de no hallar hardware PCI
    net.arp_table[0] = ArpEntry {
        ip: net.gateway,
        mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x02],
        valid: true,
    };
}

/// Calcula el Checksum estándar de Internet (RFC 1071 - One's complement sum de palabras de 16 bits)
pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        let word = u16::from_be_bytes([data[i], data[i + 1]]);
        sum = sum.wrapping_add(word as u32);
        i += 2;
    }
    if i < data.len() {
        let word = u16::from_be_bytes([data[i], 0]);
        sum = sum.wrapping_add(word as u32);
    }
    while (sum >> 16) > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Construye y envía una trama Ethernet II.
/// Si el driver E1000 DMA está inicializado, transmite por hardware real.
/// Si no, descarta silenciosamente (sin loopback — el loopback enmascaraba fallos).
pub fn send_ethernet_frame(dest_mac: [u8; 6], ethertype: u16, payload: &[u8]) {
    let frame_len = 14 + payload.len();
    let mut frame = alloc::vec::Vec::with_capacity(frame_len);

    {
        let net = NET_INTERFACE.lock();
        frame.extend_from_slice(&dest_mac);
        frame.extend_from_slice(&net.mac_address);
        frame.extend_from_slice(&ethertype.to_be_bytes());
        frame.extend_from_slice(payload);
    }

    // Actualizar contadores TX
    {
        let mut net = NET_INTERFACE.lock();
        net.tx_packets += 1;
        net.tx_bytes += frame.len() as u64;
    }

    // Transmitir por hardware real E1000 DMA (Fase 4).
    // Si el driver no está inicializado (sin tarjeta), el paquete se descarta.
    // NO hay loopback — eso enmascaraba que la red no funcionaba.
    if let Some(mut e1000) = crate::drivers::e1000::E1000.try_lock() {
        if e1000.initialized {
            e1000.send(&frame);
        }
    }
}

/// Construye y transmite una solicitud ARP (¿Quién tiene la IP X? Avisa a Y)
pub fn send_arp_request(target_ip: Ipv4Addr) {
    let mut arp_payload = Vec::with_capacity(28);
    let my_mac = NET_INTERFACE.lock().mac_address;
    let my_ip = NET_INTERFACE.lock().ip_address;

    arp_payload.extend_from_slice(&1_u16.to_be_bytes()); // Hardware: Ethernet (1)
    arp_payload.extend_from_slice(&ETHERTYPE_IPV4.to_be_bytes()); // Protocolo: IPv4 (0x0800)
    arp_payload.push(6); // Longitud MAC (6)
    arp_payload.push(4); // Longitud IP (4)
    arp_payload.extend_from_slice(&ARP_REQUEST.to_be_bytes()); // Opcode: Request (1)
    arp_payload.extend_from_slice(&my_mac);
    arp_payload.extend_from_slice(&my_ip.0);
    arp_payload.extend_from_slice(&[0x00; 6]); // Target MAC desconocido
    arp_payload.extend_from_slice(&target_ip.0);

    send_ethernet_frame([0xFF; 6], ETHERTYPE_ARP, &arp_payload);
}

/// Resuelve la dirección MAC física de destino mediante la tabla ARP o la puerta de enlace
pub fn resolve_mac_for_ip(dest_ip: Ipv4Addr) -> [u8; 6] {
    let (my_ip, my_netmask, gateway) = {
        let net = NET_INTERFACE.lock();
        (net.ip_address, net.netmask, net.gateway)
    };

    if dest_ip.0 == [255, 255, 255, 255] || dest_ip.0 == [0, 0, 0, 0] {
        return [0xFF; 6];
    }

    let is_local = (dest_ip.0[0] & my_netmask.0[0] == my_ip.0[0] & my_netmask.0[0])
        && (dest_ip.0[1] & my_netmask.0[1] == my_ip.0[1] & my_netmask.0[1])
        && (dest_ip.0[2] & my_netmask.0[2] == my_ip.0[2] & my_netmask.0[2]);

    let target = if is_local { dest_ip } else { gateway };

    // 1. Buscar en la tabla ARP
    {
        let net = NET_INTERFACE.lock();
        for entry in &net.arp_table {
            if entry.valid && entry.ip.0 == target.0 {
                return entry.mac;
            }
        }
    }

    // 2. Si no se encuentra, emitir ARP request y sondear brevemente
    send_arp_request(target);
    for _ in 0..5_000 {
        net_poll_rx();
        {
            let net = NET_INTERFACE.lock();
            for entry in &net.arp_table {
                if entry.valid && entry.ip.0 == target.0 {
                    return entry.mac;
                }
            }
        }
        for _ in 0..100 { core::hint::spin_loop(); }
    }

    // En emulador QEMU, la MAC del router virtual (10.0.2.2) es 52:54:00:12:34:02
    if target.0 == [10, 0, 2, 2] {
        return [0x52, 0x54, 0x00, 0x12, 0x34, 0x02];
    }

    [0xFF; 6]
}

static IP_PACKET_ID: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0x2000);

/// Encapsula una carga útil dentro de una cabecera IPv4 estándar (RFC 791) y la envía por Ethernet
pub fn send_ipv4_packet(dest_ip: Ipv4Addr, proto: u8, payload: &[u8]) {
    let my_ip = NET_INTERFACE.lock().ip_address;

    // Si el destino es loopback (127.0.0.1) o nuestra propia IP local, enrutar directamente por loopback
    if dest_ip.0 == [127, 0, 0, 1] || dest_ip.0 == my_ip.0 {
        deliver_loopback_ipv4(my_ip, dest_ip, proto, payload);
        return;
    }

    let total_len = (20 + payload.len()) as u16;
    let mut ip_hdr = Vec::with_capacity(20 + payload.len());
    ip_hdr.push(0x45); // Version 4, IHL 5 (20 bytes)
    ip_hdr.push(0x00); // DSCP/ECN
    ip_hdr.extend_from_slice(&total_len.to_be_bytes());
    let id = IP_PACKET_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    ip_hdr.extend_from_slice(&id.to_be_bytes());
    ip_hdr.extend_from_slice(&0x4000_u16.to_be_bytes()); // DF bit
    ip_hdr.push(64); // TTL
    ip_hdr.push(proto);
    ip_hdr.extend_from_slice(&0_u16.to_be_bytes()); // Checksum temporal
    ip_hdr.extend_from_slice(&my_ip.0);
    ip_hdr.extend_from_slice(&dest_ip.0);

    let ip_csum = internet_checksum(&ip_hdr);
    ip_hdr[10] = (ip_csum >> 8) as u8;
    ip_hdr[11] = (ip_csum & 0xFF) as u8;

    ip_hdr.extend_from_slice(payload);

    let dest_mac = resolve_mac_for_ip(dest_ip);
    send_ethernet_frame(dest_mac, ETHERTYPE_IPV4, &ip_hdr);
}

/// Construye y envía un paquete IPv4 conteniendo un datagrama ICMP Echo Request (Ping)
pub fn send_icmp_ping(target_ip: Ipv4Addr, sequence: u16) {
    let mut icmp_pkt = Vec::with_capacity(64);
    icmp_pkt.push(ICMP_ECHO_REQUEST); // Type 8
    icmp_pkt.push(0);                 // Code 0
    icmp_pkt.extend_from_slice(&0_u16.to_be_bytes()); // Checksum temporal
    icmp_pkt.extend_from_slice(&0xBEEF_u16.to_be_bytes()); // Identificador Beaver
    icmp_pkt.extend_from_slice(&sequence.to_be_bytes()); // Secuencia

    // Carga útil de datos de prueba (32 bytes)
    for i in 0..32 {
        icmp_pkt.push((b'a' + (i % 26)) as u8);
    }

    let csum = internet_checksum(&icmp_pkt);
    icmp_pkt[2] = (csum >> 8) as u8;
    icmp_pkt[3] = (csum & 0xFF) as u8;

    send_ipv4_packet(target_ip, IP_PROTO_ICMP, &icmp_pkt);
}

// Banderas de control de segmentos TCP (RFC 793)
pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;
pub const TCP_FLAG_PSH: u8 = 0x08;
pub const TCP_FLAG_ACK: u8 = 0x10;
pub const TCP_FLAG_URG: u8 = 0x20;

/// Calcula el Checksum TCP incluyendo la pseudo-cabecera IPv4 (RFC 793 / RFC 1071)
pub fn calculate_tcp_checksum(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, tcp_segment: &[u8]) -> u16 {
    let mut pseudo_header = Vec::with_capacity(12 + tcp_segment.len());
    pseudo_header.extend_from_slice(&src_ip.0);
    pseudo_header.extend_from_slice(&dst_ip.0);
    pseudo_header.push(0); // Byte reservado cero
    pseudo_header.push(IP_PROTO_TCP); // Protocolo 6
    let tcp_len = tcp_segment.len() as u16;
    pseudo_header.extend_from_slice(&tcp_len.to_be_bytes());
    pseudo_header.extend_from_slice(tcp_segment);
    internet_checksum(&pseudo_header)
}

/// Construye y envía un segmento TCP a través de la pila IP estándar
pub fn send_tcp_segment(
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    window_size: u16,
    data: &[u8],
) {
    let mut tcp_seg = Vec::with_capacity(20 + data.len());
    tcp_seg.extend_from_slice(&src_port.to_be_bytes());
    tcp_seg.extend_from_slice(&dst_port.to_be_bytes());
    tcp_seg.extend_from_slice(&seq.to_be_bytes());
    tcp_seg.extend_from_slice(&ack.to_be_bytes());
    tcp_seg.push(0x50); // Data offset (5 palabras de 32 bits = 20 bytes)
    tcp_seg.push(flags);
    tcp_seg.extend_from_slice(&window_size.to_be_bytes());
    tcp_seg.extend_from_slice(&0_u16.to_be_bytes()); // Checksum temporal
    tcp_seg.extend_from_slice(&0_u16.to_be_bytes()); // Urgent pointer
    tcp_seg.extend_from_slice(data);

    let csum = calculate_tcp_checksum(src_ip, dst_ip, &tcp_seg);
    tcp_seg[16] = (csum >> 8) as u8;
    tcp_seg[17] = (csum & 0xFF) as u8;

    send_ipv4_packet(dst_ip, IP_PROTO_TCP, &tcp_seg);
}

/// Transmite un segmento TCP directamente desde el contexto de interrupción/poll_rx sin re-adquirir locks
pub fn send_tcp_segment_direct(
    net: &mut NetworkInterface,
    src_mac: [u8; 6],
    dst_mac: [u8; 6],
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    window_size: u16,
    data: &[u8],
) {
    let mut tcp_seg = Vec::with_capacity(20 + data.len());
    tcp_seg.extend_from_slice(&src_port.to_be_bytes());
    tcp_seg.extend_from_slice(&dst_port.to_be_bytes());
    tcp_seg.extend_from_slice(&seq.to_be_bytes());
    tcp_seg.extend_from_slice(&ack.to_be_bytes());
    tcp_seg.push(0x50);
    tcp_seg.push(flags);
    tcp_seg.extend_from_slice(&window_size.to_be_bytes());
    tcp_seg.extend_from_slice(&0_u16.to_be_bytes());
    tcp_seg.extend_from_slice(&0_u16.to_be_bytes());
    tcp_seg.extend_from_slice(data);

    let csum = calculate_tcp_checksum(src_ip, dst_ip, &tcp_seg);
    tcp_seg[16] = (csum >> 8) as u8;
    tcp_seg[17] = (csum & 0xFF) as u8;

    let total_len = (20 + tcp_seg.len()) as u16;
    let mut ip_hdr = Vec::with_capacity(20 + tcp_seg.len());
    ip_hdr.push(0x45);
    ip_hdr.push(0x00);
    ip_hdr.extend_from_slice(&total_len.to_be_bytes());
    let id = IP_PACKET_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    ip_hdr.extend_from_slice(&id.to_be_bytes());
    ip_hdr.extend_from_slice(&0x4000_u16.to_be_bytes());
    ip_hdr.push(64);
    ip_hdr.push(IP_PROTO_TCP);
    ip_hdr.extend_from_slice(&0_u16.to_be_bytes());
    ip_hdr.extend_from_slice(&src_ip.0);
    ip_hdr.extend_from_slice(&dst_ip.0);

    let ip_csum = internet_checksum(&ip_hdr);
    ip_hdr[10] = (ip_csum >> 8) as u8;
    ip_hdr[11] = (ip_csum & 0xFF) as u8;
    ip_hdr.extend_from_slice(&tcp_seg);

    let mut eth = Vec::with_capacity(14 + ip_hdr.len());
    eth.extend_from_slice(&dst_mac);
    eth.extend_from_slice(&src_mac);
    eth.extend_from_slice(&ETHERTYPE_IPV4.to_be_bytes());
    eth.extend_from_slice(&ip_hdr);

    net.tx_packets += 1;
    net.tx_bytes += eth.len() as u64;

    if let Some(mut e1000) = crate::drivers::e1000::E1000.try_lock() {
        if e1000.initialized {
            e1000.send(&eth);
        }
    }
}

/// Procesa paquetes entrantes: actualiza contadores, tabla ARP,
/// responde ARP requests e ICMP echo requests por hardware real.
/// Devuelve `true` si se recibio un MSG_QUERY de BeaverSync que requiere responder fuera del lock.
fn process_incoming_packet(frame: &[u8], net: &mut NetworkInterface) -> bool {
    if frame.len() < 14 {
        return false;
    }

    net.rx_packets += 1;
    net.rx_bytes += frame.len() as u64;

    let src_eth = {
        let mut m = [0u8; 6];
        m.copy_from_slice(&frame[6..12]);
        m
    };
    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
    let payload = &frame[14..];

    if ethertype == ETHERTYPE_ARP && payload.len() >= 28 {
        let opcode     = u16::from_be_bytes([payload[6], payload[7]]);
        let sender_ip  = Ipv4Addr::new(payload[14], payload[15], payload[16], payload[17]);
        let target_ip  = Ipv4Addr::new(payload[24], payload[25], payload[26], payload[27]);
        let mut sender_mac = [0u8; 6];
        sender_mac.copy_from_slice(&payload[8..14]);

        // Registrar o actualizar tabla ARP
        for entry in net.arp_table.iter_mut() {
            if !entry.valid || entry.ip.0 == sender_ip.0 {
                entry.ip  = sender_ip;
                entry.mac = sender_mac;
                entry.valid = true;
                break;
            }
        }

        // Responder ARP Request dirigido a nuestra IP
        if opcode == ARP_REQUEST && target_ip.0 == net.ip_address.0 {
            let my_mac = net.mac_address;
            let my_ip  = net.ip_address;
            // Construir ARP Reply (28 bytes de payload)
            let mut arp_reply = alloc::vec::Vec::with_capacity(28);
            arp_reply.extend_from_slice(&1u16.to_be_bytes());             // HW: Ethernet
            arp_reply.extend_from_slice(&ETHERTYPE_IPV4.to_be_bytes());   // Proto: IPv4
            arp_reply.push(6); arp_reply.push(4);                          // HW+proto len
            arp_reply.extend_from_slice(&ARP_REPLY.to_be_bytes());         // Opcode: Reply
            arp_reply.extend_from_slice(&my_mac);                          // Sender MAC (nosotros)
            arp_reply.extend_from_slice(&my_ip.0);                         // Sender IP
            arp_reply.extend_from_slice(&sender_mac);                      // Target MAC
            arp_reply.extend_from_slice(&sender_ip.0);                     // Target IP

            // Transmitir por E1000 sin re-adquirir net lock (ya lo tenemos)
            let frame_len = 14 + arp_reply.len();
            let mut eth = alloc::vec::Vec::with_capacity(frame_len);
            eth.extend_from_slice(&sender_mac);
            eth.extend_from_slice(&my_mac);
            eth.extend_from_slice(&ETHERTYPE_ARP.to_be_bytes());
            eth.extend_from_slice(&arp_reply);
            net.tx_packets += 1;
            net.tx_bytes += eth.len() as u64;
            if let Some(mut e1000) = crate::drivers::e1000::E1000.try_lock() {
                if e1000.initialized { e1000.send(&eth); }
            }
        }

    } else if ethertype == ETHERTYPE_IPV4 && payload.len() >= 20 {
        let proto   = payload[9];
        let src_ip  = Ipv4Addr::new(payload[12], payload[13], payload[14], payload[15]);
        let dst_ip  = Ipv4Addr::new(payload[16], payload[17], payload[18], payload[19]);
        let ihl     = ((payload[0] & 0x0F) * 4) as usize;

        // Descartar paquetes con nuestra propia IP o nuestra propia MAC como origen (anti-eco)
        if src_ip.0 == net.ip_address.0 || src_eth == net.mac_address {
            return false;
        }

        // Aprender MAC del remitente IPv4 en la tabla ARP
        if src_ip.0 != [0, 0, 0, 0] {
            for entry in net.arp_table.iter_mut() {
                if !entry.valid || entry.ip.0 == src_ip.0 {
                    entry.ip = src_ip;
                    entry.mac = src_eth;
                    entry.valid = true;
                    break;
                }
            }
        }

        if proto == IP_PROTO_ICMP && payload.len() >= ihl + 8 {
            let icmp_data = &payload[ihl..];
            let icmp_type = icmp_data[0];

            if icmp_type == ICMP_ECHO_REPLY {
                // Respuesta a nuestro ping — registrar
                net.last_ping_replied = true;
                net.last_ping_ttl = payload[8];
                net.last_ping_seq = u16::from_be_bytes([icmp_data[6], icmp_data[7]]);

            } else if icmp_type == ICMP_ECHO_REQUEST && dst_ip.0 == net.ip_address.0 {
                // Alguien nos hace ping — responder con ICMP Echo Reply real
                let my_ip  = net.ip_address;
                let my_mac = net.mac_address;
                let seq    = u16::from_be_bytes([icmp_data[6], icmp_data[7]]);
                let id     = u16::from_be_bytes([icmp_data[4], icmp_data[5]]);
                let data_section = if icmp_data.len() > 8 { &icmp_data[8..] } else { &[] };

                // ICMP Echo Reply
                let mut icmp_reply: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(8 + data_section.len());
                icmp_reply.push(ICMP_ECHO_REPLY); // Type 0
                icmp_reply.push(0);                // Code 0
                icmp_reply.extend_from_slice(&0u16.to_be_bytes()); // checksum placeholder
                icmp_reply.extend_from_slice(&id.to_be_bytes());
                icmp_reply.extend_from_slice(&seq.to_be_bytes());
                icmp_reply.extend_from_slice(data_section);
                let csum = internet_checksum(&icmp_reply);
                icmp_reply[2] = (csum >> 8) as u8;
                icmp_reply[3] = (csum & 0xFF) as u8;

                // IP header para el reply
                let total_len = (20 + icmp_reply.len()) as u16;
                let mut ip_hdr: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(20 + icmp_reply.len());
                ip_hdr.push(0x45); ip_hdr.push(0x00);
                ip_hdr.extend_from_slice(&total_len.to_be_bytes());
                ip_hdr.extend_from_slice(&0x0000u16.to_be_bytes()); // ID
                ip_hdr.extend_from_slice(&0x4000u16.to_be_bytes()); // DF bit
                ip_hdr.push(64); ip_hdr.push(IP_PROTO_ICMP);
                ip_hdr.extend_from_slice(&0u16.to_be_bytes()); // checksum placeholder
                ip_hdr.extend_from_slice(&my_ip.0);
                ip_hdr.extend_from_slice(&src_ip.0);
                let ip_csum = internet_checksum(&ip_hdr);
                ip_hdr[10] = (ip_csum >> 8) as u8;
                ip_hdr[11] = (ip_csum & 0xFF) as u8;
                ip_hdr.extend_from_slice(&icmp_reply);

                // Trama Ethernet
                let mut eth: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(14 + ip_hdr.len());
                eth.extend_from_slice(&src_eth);
                eth.extend_from_slice(&my_mac);
                eth.extend_from_slice(&ETHERTYPE_IPV4.to_be_bytes());
                eth.extend_from_slice(&ip_hdr);
                net.tx_packets += 1;
                net.tx_bytes += eth.len() as u64;
                if let Some(mut e1000) = crate::drivers::e1000::E1000.try_lock() {
                    if e1000.initialized { e1000.send(&eth); }
                }
            }

        } else if proto == IP_PROTO_UDP && payload.len() >= ihl + 8 {
            let udp_data = &payload[ihl..];
            let src_port = u16::from_be_bytes([udp_data[0], udp_data[1]]);
            let dst_port = u16::from_be_bytes([udp_data[2], udp_data[3]]);
            let udp_len  = u16::from_be_bytes([udp_data[4], udp_data[5]]) as usize;
            let end = udp_len.min(udp_data.len());
            let app_payload = if end > 8 { &udp_data[8..end] } else { &[] };

            if dst_port == 4242 {
                if let Some(mut mgr) = crate::drivers::mesh::MESH_MANAGER.try_lock() {
                    return mgr.handle_packet(src_ip, src_port, app_payload);
                }
            } else if dst_port == 68 && app_payload.len() >= 240 {
                // Datagrama DHCP BOOTREPLY (RFC 2131)
                let op = app_payload[0];
                if op == 2 {
                    let yiaddr = Ipv4Addr::new(app_payload[16], app_payload[17], app_payload[18], app_payload[19]);
                    let mut netmask = Ipv4Addr::new(255, 255, 255, 0);
                    let mut gateway = src_ip;
                    let mut dns = src_ip;
                    let mut server_ip = src_ip;
                    let mut lease_secs = 86400u32;

                    // Opciones DHCP a partir del byte 240 (tras el magic cookie 0x63, 0x82, 0x53, 0x63)
                    if app_payload.len() >= 240 && &app_payload[236..240] == &[0x63, 0x82, 0x53, 0x63] {
                        let mut opt_idx = 240;
                        while opt_idx < app_payload.len() {
                            let opt_code = app_payload[opt_idx];
                            if opt_code == 255 {
                                break;
                            } else if opt_code == 0 {
                                opt_idx += 1;
                                continue;
                            }
                            if opt_idx + 1 >= app_payload.len() { break; }
                            let opt_len = app_payload[opt_idx + 1] as usize;
                            let opt_start = opt_idx + 2;
                            let opt_end = opt_start + opt_len;
                            if opt_end > app_payload.len() { break; }

                            match opt_code {
                                1 if opt_len >= 4 => { // Mascara
                                    netmask = Ipv4Addr::new(app_payload[opt_start], app_payload[opt_start + 1], app_payload[opt_start + 2], app_payload[opt_start + 3]);
                                }
                                3 if opt_len >= 4 => { // Gateway
                                    gateway = Ipv4Addr::new(app_payload[opt_start], app_payload[opt_start + 1], app_payload[opt_start + 2], app_payload[opt_start + 3]);
                                }
                                6 if opt_len >= 4 => { // DNS
                                    dns = Ipv4Addr::new(app_payload[opt_start], app_payload[opt_start + 1], app_payload[opt_start + 2], app_payload[opt_start + 3]);
                                }
                                51 if opt_len >= 4 => { // Lease time
                                    lease_secs = u32::from_be_bytes([
                                        app_payload[opt_start], app_payload[opt_start + 1],
                                        app_payload[opt_start + 2], app_payload[opt_start + 3],
                                    ]);
                                }
                                54 if opt_len >= 4 => { // Server Identifier
                                    server_ip = Ipv4Addr::new(app_payload[opt_start], app_payload[opt_start + 1], app_payload[opt_start + 2], app_payload[opt_start + 3]);
                                }
                                _ => {}
                            }
                            opt_idx = opt_end;
                        }
                    }

                    let lease = DhcpLease {
                        ip: yiaddr,
                        netmask,
                        gateway,
                        dns,
                        server_ip,
                        lease_time_secs: lease_secs,
                    };
                    *DHCP_PENDING_LEASE.lock() = Some(lease);
                }
            } else if (src_port == 53 || dst_port == 49153) && app_payload.len() >= 12 {
                // Respuesta DNS RFC 1035
                let tx_id = u16::from_be_bytes([app_payload[0], app_payload[1]]);
                let flags = u16::from_be_bytes([app_payload[2], app_payload[3]]);
                let qdcount = u16::from_be_bytes([app_payload[4], app_payload[5]]);
                let ancount = u16::from_be_bytes([app_payload[6], app_payload[7]]);

                if (flags & 0x8000) != 0 && ancount > 0 {
                    let mut cursor = 12usize;
                    for _ in 0..qdcount {
                        while cursor < app_payload.len() {
                            let len = app_payload[cursor] as usize;
                            if len == 0 {
                                cursor += 1;
                                break;
                            } else if (len & 0xC0) == 0xC0 {
                                cursor += 2;
                                break;
                            } else {
                                cursor += 1 + len;
                            }
                        }
                        cursor += 4;
                    }

                    for _ in 0..ancount {
                        if cursor >= app_payload.len() { break; }
                        if (app_payload[cursor] & 0xC0) == 0xC0 {
                            cursor += 2;
                        } else {
                            while cursor < app_payload.len() && app_payload[cursor] != 0 {
                                cursor += 1 + app_payload[cursor] as usize;
                            }
                            cursor += 1;
                        }
                        if cursor + 10 > app_payload.len() { break; }
                        let rtype = u16::from_be_bytes([app_payload[cursor], app_payload[cursor + 1]]);
                        let rdlength = u16::from_be_bytes([app_payload[cursor + 8], app_payload[cursor + 9]]) as usize;
                        cursor += 10;
                        if rtype == 1 && rdlength == 4 && cursor + 4 <= app_payload.len() {
                            let resolved = Ipv4Addr::new(
                                app_payload[cursor],
                                app_payload[cursor + 1],
                                app_payload[cursor + 2],
                                app_payload[cursor + 3],
                            );
                            *DNS_PENDING_RESPONSE.lock() = Some((tx_id, resolved));
                            break;
                        }
                        cursor += rdlength;
                    }
                }
            }
        } else if proto == IP_PROTO_TCP && payload.len() >= ihl + 20 {
            let tcp_data = &payload[ihl..];
            let src_port = u16::from_be_bytes([tcp_data[0], tcp_data[1]]);
            let dst_port = u16::from_be_bytes([tcp_data[2], tcp_data[3]]);
            let seq_num  = u32::from_be_bytes([tcp_data[4], tcp_data[5], tcp_data[6], tcp_data[7]]);
            let ack_num  = u32::from_be_bytes([tcp_data[8], tcp_data[9], tcp_data[10], tcp_data[11]]);
            let data_offset = ((tcp_data[12] >> 4) as usize) * 4;
            let flags = tcp_data[13];
            let window = u16::from_be_bytes([tcp_data[14], tcp_data[15]]);
            let seg_payload = if tcp_data.len() > data_offset {
                &tcp_data[data_offset..]
            } else {
                &[]
            };

            handle_incoming_tcp_segment(net, src_eth, src_ip, dst_ip, src_port, dst_port, seq_num, ack_num, flags, window, seg_payload);
        }
    }
    false
}

/// Sondea el anillo RX del E1000 y procesa todos los paquetes pendientes.
/// Llamar desde el bucle principal del kernel o desde el tick del PIT.
/// Usa try_lock para no bloquear nunca el hilo principal.
pub fn net_poll_rx() {
    // Intentar adquirir el driver; si está ocupado, omitir esta ronda
    let e1000_guard = match crate::drivers::e1000::E1000.try_lock() {
        Some(g) => g,
        None    => return,
    };
    if !e1000_guard.initialized {
        return;
    }

    // Recolectar tramas en un buffer temporal (sin mantener el lock de e1000 durante el proceso)
    // Necesitamos liberar e1000 antes de llamar process_incoming_packet que puede lock net.
    // Usamos un arreglo de trama fija en el stack (sin heap extra).
    // Procesamos hasta 8 tramas por llamada para no monopolizar la CPU.
    const MAX_FRAMES: usize = 8;
    let mut frames: [([u8; 1536], usize); MAX_FRAMES] = [([0u8; 1536], 0); MAX_FRAMES];
    let mut count = 0usize;

    // Necesitamos &mut para poll_rx — drop el guard inmutabel y re-acquire como mutable
    drop(e1000_guard);

    if let Some(mut e1000) = crate::drivers::e1000::E1000.try_lock() {
        e1000.poll_rx(|frame| {
            if count < MAX_FRAMES && frame.len() <= 1536 {
                let len = frame.len();
                frames[count].0[..len].copy_from_slice(frame);
                frames[count].1 = len;
                count += 1;
            }
        });
    }

    // Procesar cada trama fuera del lock del E1000
    let mut need_mesh_announce = false;
    if count > 0 {
        if let Some(mut net) = NET_INTERFACE.try_lock() {
            for i in 0..count {
                let len = frames[i].1;
                if len > 0 {
                    if process_incoming_packet(&frames[i].0[..len], &mut net) {
                        need_mesh_announce = true;
                    }
                }
            }
        }
    }

    // Si recibimos MSG_QUERY o MSG_FILE_SYNC, procesar acciones diferidas fuera de NET_INTERFACE lock
    if need_mesh_announce {
        crate::drivers::mesh::flush_mesh_pending_actions();
    }
}

/// Transmite un datagrama UDP encapsulado en IPv4 y Ethernet II
pub fn send_udp_datagram(dest_ip: Ipv4Addr, src_port: u16, dest_port: u16, data: &[u8]) {
    let udp_len = (8 + data.len()) as u16;
    let mut udp_seg = Vec::with_capacity(8 + data.len());
    udp_seg.extend_from_slice(&src_port.to_be_bytes());
    udp_seg.extend_from_slice(&dest_port.to_be_bytes());
    udp_seg.extend_from_slice(&udp_len.to_be_bytes());
    udp_seg.extend_from_slice(&0_u16.to_be_bytes()); // Checksum opcional en IPv4
    udp_seg.extend_from_slice(data);

    send_ipv4_packet(dest_ip, IP_PROTO_UDP, &udp_seg);
}

/// Resuelve un nombre de dominio (FQDN) a una dirección IPv4 utilizando DNS real sobre UDP puerto 53 (RFC 1035)
pub fn resolve_dns(domain: &str) -> Result<Ipv4Addr, String> {
    let clean = domain.trim();
    if let Some(ip) = Ipv4Addr::parse(clean) {
        return Ok(ip);
    }
    if clean.eq_ignore_ascii_case("localhost") {
        return Ok(Ipv4Addr::new(127, 0, 0, 1));
    }
    // Resolución local de nombres del sistema Beaver (tabla estática tipo /etc/hosts)
    if clean.eq_ignore_ascii_case("beaver.lan") || clean.eq_ignore_ascii_case("beaver") {
        let my_ip = NET_INTERFACE.lock().ip_address;
        return Ok(my_ip);
    }
    if clean.eq_ignore_ascii_case("router.lan") || clean.eq_ignore_ascii_case("gateway") {
        let gw = NET_INTERFACE.lock().gateway;
        return Ok(gw);
    }

    *DNS_PENDING_RESPONSE.lock() = None;

    let dns_server = NET_INTERFACE.lock().dns_server;
    let tx_id = ((crate::drivers::timer::get_ticks() & 0x7FFF) as u16) ^ 0x4200;

    let mut udp_dns_pkt = Vec::with_capacity(64);
    // Header DNS RFC 1035: ID, Flags (0x0100 Recursion Desired), QDCOUNT=1, ANCOUNT=0, NSCOUNT=0, ARCOUNT=0
    udp_dns_pkt.extend_from_slice(&tx_id.to_be_bytes());
    udp_dns_pkt.extend_from_slice(&0x0100_u16.to_be_bytes());
    udp_dns_pkt.extend_from_slice(&1_u16.to_be_bytes());
    udp_dns_pkt.extend_from_slice(&[0; 6]);

    // Formatear etiquetas de dominio (3www6google3com0)
    for part in clean.split('.') {
        if !part.is_empty() {
            if part.len() > 63 {
                return Err(String::from("Etiqueta DNS excede 63 bytes"));
            }
            udp_dns_pkt.push(part.len() as u8);
            udp_dns_pkt.extend_from_slice(part.as_bytes());
        }
    }
    udp_dns_pkt.push(0); // Terminador
    udp_dns_pkt.extend_from_slice(&1_u16.to_be_bytes()); // Tipo A (IPv4)
    udp_dns_pkt.extend_from_slice(&1_u16.to_be_bytes()); // Clase IN (Internet)

    // Enviar datagrama UDP real al puerto 53 del servidor DNS
    send_udp_datagram(dns_server, 49153, 53, &udp_dns_pkt);

    // Sondeo de hardware con timeout esperando la respuesta real de la red
    for _ in 0..40_000 {
        net_poll_rx();
        if let Some((resp_id, ip)) = *DNS_PENDING_RESPONSE.lock() {
            if resp_id == tx_id {
                return Ok(ip);
            }
        }
        for _ in 0..200 {
            core::hint::spin_loop();
        }
    }

    Err(format!("Timeout esperando resolucion DNS de '{}' desde servidor {}", clean, dns_server.to_string()))
}

/// Negociación y autoconfiguración dinámica de parámetros de red mediante DHCP (RFC 2131)
pub fn dhcp_discover_and_configure() -> Result<DhcpLease, &'static str> {
    let mac = {
        let net = NET_INTERFACE.lock();
        if !net.card_detected {
            return Err("Tarjeta de red no detectada o no inicializada");
        }
        net.mac_address
    };

    *DHCP_PENDING_LEASE.lock() = None;

    // 1. Construir datagrama DHCPDISCOVER (BOOTREQUEST op=1, htype=1, hlen=6)
    let mut dhcp_pkt = Vec::with_capacity(300);
    dhcp_pkt.push(1); // op: BOOTREQUEST
    dhcp_pkt.push(1); // htype: Ethernet
    dhcp_pkt.push(6); // hlen: 6 bytes MAC
    dhcp_pkt.push(0); // hops
    let xid = 0xBEA50F71_u32;
    dhcp_pkt.extend_from_slice(&xid.to_be_bytes()); // xid (Transaction ID)
    dhcp_pkt.extend_from_slice(&0_u16.to_be_bytes()); // secs
    dhcp_pkt.extend_from_slice(&0x8000_u16.to_be_bytes()); // flags: Broadcast
    dhcp_pkt.extend_from_slice(&[0; 16]); // ciaddr, yiaddr, siaddr, giaddr (0.0.0.0)

    // chaddr (16 bytes: 6 bytes MAC + 10 ceros)
    dhcp_pkt.extend_from_slice(&mac);
    dhcp_pkt.extend_from_slice(&[0; 10]);

    // sname (64 bytes) y file (128 bytes)
    dhcp_pkt.extend_from_slice(&[0; 64]);
    dhcp_pkt.extend_from_slice(&[0; 128]);

    // Magic Cookie (RFC 1497: 0x63, 0x82, 0x53, 0x63)
    dhcp_pkt.extend_from_slice(&[0x63, 0x82, 0x53, 0x63]);

    // Opción 53: DHCP Message Type (DHCPDISCOVER = 1)
    dhcp_pkt.extend_from_slice(&[53, 1, 1]);

    // Opción 55: Parameter Request List (1=Subnet Mask, 3=Router, 6=DNS)
    dhcp_pkt.extend_from_slice(&[55, 3, 1, 3, 6]);

    // Opción 255: End
    dhcp_pkt.push(255);

    while dhcp_pkt.len() < 300 {
        dhcp_pkt.push(0);
    }

    // Cabecera UDP (Puerto origen: 68 cliente, Puerto destino: 67 servidor DHCP)
    let udp_len = (8 + dhcp_pkt.len()) as u16;
    let mut udp_hdr = Vec::with_capacity(8);
    udp_hdr.extend_from_slice(&68_u16.to_be_bytes());
    udp_hdr.extend_from_slice(&67_u16.to_be_bytes());
    udp_hdr.extend_from_slice(&udp_len.to_be_bytes());
    udp_hdr.extend_from_slice(&0_u16.to_be_bytes()); // Checksum

    // Cabecera IPv4 Broadcast (255.255.255.255)
    let total_len = (20 + udp_hdr.len() + dhcp_pkt.len()) as u16;
    let mut ip_hdr = Vec::with_capacity(20);
    ip_hdr.push(0x45);
    ip_hdr.push(0x00);
    ip_hdr.extend_from_slice(&total_len.to_be_bytes());
    ip_hdr.extend_from_slice(&0_u16.to_be_bytes());
    ip_hdr.extend_from_slice(&0_u16.to_be_bytes());
    ip_hdr.push(64);
    ip_hdr.push(IP_PROTO_UDP);
    ip_hdr.extend_from_slice(&0_u16.to_be_bytes());
    ip_hdr.extend_from_slice(&[0, 0, 0, 0]); // Origen 0.0.0.0 antes de recibir IP
    ip_hdr.extend_from_slice(&[255, 255, 255, 255]); // Broadcast
    let ip_csum = internet_checksum(&ip_hdr);
    ip_hdr[10] = (ip_csum >> 8) as u8;
    ip_hdr[11] = (ip_csum & 0xFF) as u8;

    let mut full_payload = ip_hdr;
    full_payload.extend_from_slice(&udp_hdr);
    full_payload.extend_from_slice(&dhcp_pkt);

    // Transmitir trama broadcast hacia la red física o virtual
    send_ethernet_frame([0xFF; 6], ETHERTYPE_IPV4, &full_payload);

    // 2. Esperar respuesta real (DHCPOFFER o DHCPACK) sondeando el hardware
    for _ in 0..30_000 {
        net_poll_rx();
        if let Some(lease) = DHCP_PENDING_LEASE.lock().take() {
            // Actualizar dinámicamente la interfaz de red del kernel con datos reales
            {
                let mut net = NET_INTERFACE.lock();
                net.ip_address = lease.ip;
                net.netmask = lease.netmask;
                net.gateway = lease.gateway;
                net.dns_server = lease.dns;
            }
            return Ok(lease);
        }
        for _ in 0..500 {
            core::hint::spin_loop();
        }
    }

    Err("Timeout: sin respuesta del servidor DHCP en la red (RFC 2131)")
}

// ============================================================================
// BSD Sockets API & Full State TCP Machine (RFC 793) & Real HTTP/1.1 Client
// ============================================================================

pub const AF_INET: i32 = 2;
pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Unused,
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait,
    CloseWait,
    ClosedWait, // Compatibilidad retroactiva
}

#[derive(Debug, Clone)]
pub struct BsdSocket {
    pub id: usize,
    pub state: SocketState,
    pub local_port: u16,
    pub remote_ip: Ipv4Addr,
    pub remote_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub window_size: u16,
    pub rx_buffer: Vec<u8>,
}

pub static SOCKET_TABLE: SpinLock<[Option<BsdSocket>; 16]> = SpinLock::new([
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
]);

/// Entrega interna de datagramas IPv4 a través de la interfaz de loopback virtual
pub fn deliver_loopback_ipv4(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, proto: u8, payload: &[u8]) {
    if proto == IP_PROTO_TCP && payload.len() >= 20 {
        let src_port = u16::from_be_bytes([payload[0], payload[1]]);
        let dst_port = u16::from_be_bytes([payload[2], payload[3]]);
        let seq_num  = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        let ack_num  = u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]);
        let data_offset = ((payload[12] >> 4) as usize) * 4;
        let flags = payload[13];
        let window = u16::from_be_bytes([payload[14], payload[15]]);
        let seg_payload = if payload.len() > data_offset {
            &payload[data_offset..]
        } else {
            &[]
        };

        if dst_port == 80 {
            handle_local_http_server(src_ip, src_port, seq_num, ack_num, flags, seg_payload);
            return;
        }

        handle_loopback_client_segment(src_ip, dst_ip, src_port, dst_port, seq_num, ack_num, flags, window, seg_payload);
    }
}

/// Servidor HTTP/1.1 nativo del kernel de Beaver OS en el puerto 80 (telemetría y archivos)
pub fn handle_local_http_server(
    client_ip: Ipv4Addr,
    client_port: u16,
    client_seq: u32,
    _client_ack: u32,
    flags: u8,
    payload: &[u8],
) {
    let my_ip = NET_INTERFACE.lock().ip_address;

    // 1. Handshake SYN: responder SYN | ACK
    if (flags & TCP_FLAG_SYN) != 0 {
        let server_seq = 50_000_u32;
        let server_ack = client_seq.wrapping_add(1);
        send_tcp_segment(
            my_ip, client_ip, 80, client_port,
            server_seq, server_ack,
            TCP_FLAG_SYN | TCP_FLAG_ACK, 64240, &[]
        );
        return;
    }

    // 2. Recepción de petición HTTP
    if (flags & TCP_FLAG_PSH) != 0 && !payload.is_empty() {
        let req_str = match core::str::from_utf8(payload) {
            Ok(s) => s,
            Err(_) => return,
        };

        let mut lines = req_str.lines();
        let request_line = lines.next().unwrap_or("");
        let mut parts = request_line.split_whitespace();
        let _method = parts.next().unwrap_or("GET");
        let path = parts.next().unwrap_or("/");

        let (status, content_type, body_data) = if path == "/info.json" {
            let uptime = crate::drivers::timer::get_uptime_seconds();
            let cores = crate::apic::SMP.lock().cores.len().max(1);
            let total_frames = crate::memory::frame_allocator::FRAME_ALLOCATOR.total_usable_frames();
            let free_frames = crate::memory::frame_allocator::FRAME_ALLOCATOR.free_frames_count();
            let total_ram_mb = (total_frames * 4) / 1024;
            let free_ram_mb = (free_frames * 4) / 1024;
            let ahci_dma = crate::storage::block_device::BLOCK_DEVICE.lock().is_using_ahci();

            let json = alloc::format!(
                "{{\n  \"os\": \"Beaver OS\",\n  \"version\": \"0.9.0\",\n  \"brand\": \"Beaversoft\",\n  \"author\": \"José Alberto Puente Maldonado (japuentem)\",\n  \"arch\": \"x86_64\",\n  \"uptime_secs\": {},\n  \"smp_cores\": {},\n  \"ram_total_mb\": {},\n  \"ram_free_mb\": {},\n  \"storage_dma\": {}\n}}\n",
                uptime, cores, total_ram_mb, free_ram_mb, ahci_dma
            );
            ("200 OK", "application/json", json.into_bytes())
        } else if path.starts_with("/disk/") {
            let vfs = crate::fs::vfs::VFS.lock();
            match vfs.read_file_bytes(path, 0, 0) {
                Ok(bytes) => ("200 OK", "text/plain", bytes),
                Err(_) => ("404 Not Found", "text/plain", alloc::format!("Error 404: Archivo '{}' no encontrado en BeaverFS\n", path).into_bytes()),
            }
        } else {
            let html = alloc::format!(
                "<!DOCTYPE html>\n<html>\n<head><title>Beaver OS Web Server</title></head>\n<body>\n<h1>Bienvenido a Beaver OS v0.9.0</h1>\n<p>Servidor HTTP nativo en Ring 0 / x86_64 Long Mode.</p>\n<p>Prueba consultar <a href=\"/info.json\">/info.json</a> para telemetria en vivo.</p>\n</body>\n</html>\n"
            );
            ("200 OK", "text/html", html.into_bytes())
        };

        let resp_header = alloc::format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nServer: BeaverOS-Kernel-HTTP/0.9.0\r\n\r\n",
            status, content_type, body_data.len()
        );

        let mut full_resp = resp_header.into_bytes();
        full_resp.extend_from_slice(&body_data);

        let server_seq = 50_001_u32;
        let server_ack = client_seq.wrapping_add(payload.len() as u32);

        send_tcp_segment(
            my_ip, client_ip, 80, client_port,
            server_seq, server_ack,
            TCP_FLAG_PSH | TCP_FLAG_ACK | TCP_FLAG_FIN, 64240, &full_resp
        );
    }
}

/// Actualiza el estado del socket cliente local al recibir segmentos por loopback
pub fn handle_loopback_client_segment(
    _src_ip: Ipv4Addr,
    _dst_ip: Ipv4Addr,
    _src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    _window: u16,
    payload: &[u8],
) {
    let mut table = SOCKET_TABLE.lock();
    for slot in table.iter_mut() {
        if let Some(ref mut sock) = slot {
            if sock.local_port == dst_port {
                match sock.state {
                    SocketState::SynSent => {
                        if (flags & (TCP_FLAG_SYN | TCP_FLAG_ACK)) == (TCP_FLAG_SYN | TCP_FLAG_ACK) {
                            sock.ack_num = seq.wrapping_add(1);
                            sock.seq_num = ack;
                            sock.state = SocketState::Established;
                            return;
                        } else if (flags & TCP_FLAG_RST) != 0 {
                            sock.state = SocketState::Closed;
                            return;
                        }
                    }
                    SocketState::Established => {
                        if !payload.is_empty() {
                            sock.rx_buffer.extend_from_slice(payload);
                            sock.ack_num = seq.wrapping_add(payload.len() as u32);
                        }
                        if (flags & TCP_FLAG_FIN) != 0 {
                            sock.state = SocketState::CloseWait;
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Procesa segmentos TCP entrantes en el contexto de interrupción de red
pub fn handle_incoming_tcp_segment(
    net: &mut NetworkInterface,
    src_eth: [u8; 6],
    src_ip: Ipv4Addr,
    _dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    _window: u16,
    payload: &[u8],
) {
    if dst_port == 80 {
        handle_local_http_server(src_ip, src_port, seq, ack, flags, payload);
        return;
    }

    let mut table = SOCKET_TABLE.lock();
    for slot in table.iter_mut() {
        if let Some(ref mut sock) = slot {
            let match_conn = sock.local_port == dst_port
                && (sock.remote_ip.0 == [0, 0, 0, 0] || sock.remote_ip.0 == src_ip.0)
                && (sock.remote_port == 0 || sock.remote_port == src_port);

            if match_conn {
                match sock.state {
                    SocketState::SynSent => {
                        if (flags & (TCP_FLAG_SYN | TCP_FLAG_ACK)) == (TCP_FLAG_SYN | TCP_FLAG_ACK) {
                            sock.ack_num = seq.wrapping_add(1);
                            sock.seq_num = ack;
                            sock.state = SocketState::Established;

                            // Responder con ACK sin re-adquirir el lock de net
                            let my_ip = net.ip_address;
                            let my_mac = net.mac_address;
                            send_tcp_segment_direct(
                                net, my_mac, src_eth, my_ip, src_ip,
                                dst_port, src_port, sock.seq_num, sock.ack_num,
                                TCP_FLAG_ACK, sock.window_size, &[]
                            );
                            return;
                        } else if (flags & TCP_FLAG_RST) != 0 {
                            sock.state = SocketState::Closed;
                            return;
                        }
                    }
                    SocketState::Established => {
                        if (flags & TCP_FLAG_RST) != 0 {
                            sock.state = SocketState::Closed;
                            return;
                        }

                        let mut need_ack = false;
                        if !payload.is_empty() {
                            sock.rx_buffer.extend_from_slice(payload);
                            sock.ack_num = seq.wrapping_add(payload.len() as u32);
                            need_ack = true;
                        }

                        if (flags & TCP_FLAG_FIN) != 0 {
                            sock.ack_num = seq.wrapping_add(if payload.is_empty() { 1 } else { payload.len() as u32 + 1 });
                            sock.state = SocketState::CloseWait;
                            need_ack = true;
                        }

                        if need_ack {
                            let my_ip = net.ip_address;
                            let my_mac = net.mac_address;
                            send_tcp_segment_direct(
                                net, my_mac, src_eth, my_ip, src_ip,
                                dst_port, src_port, sock.seq_num, sock.ack_num,
                                TCP_FLAG_ACK, sock.window_size, &[]
                            );
                        }
                        return;
                    }
                    SocketState::FinWait => {
                        if (flags & TCP_FLAG_ACK) != 0 {
                            sock.state = SocketState::Closed;
                            return;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

pub fn bsd_socket(domain: i32, sock_type: i32, _protocol: i32) -> Result<usize, &'static str> {
    if domain != AF_INET || sock_type != SOCK_STREAM {
        return Err("Dominio o tipo de socket no soportado (solo AF_INET / SOCK_STREAM)");
    }
    let mut table = SOCKET_TABLE.lock();
    for (i, slot) in table.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(BsdSocket {
                id: i,
                state: SocketState::Closed,
                local_port: 49152 + (i as u16),
                remote_ip: Ipv4Addr::new(0, 0, 0, 0),
                remote_port: 0,
                seq_num: 1000 + (i as u32 * 500),
                ack_num: 0,
                window_size: 64240,
                rx_buffer: Vec::new(),
            });
            return Ok(i);
        }
    }
    Err("Tabla de sockets llena (máximo 16 sockets simultáneos)")
}

/// Establece una conexión TCP mediante el 3-way handshake estándar (SYN -> SYN-ACK -> ACK)
pub fn bsd_connect(sock_id: usize, remote_ip: Ipv4Addr, remote_port: u16) -> Result<(), &'static str> {
    let (my_ip, src_port, seq) = {
        let mut table = SOCKET_TABLE.lock();
        let sock = table.get_mut(sock_id).and_then(|s| s.as_mut()).ok_or("Socket descriptor inválido")?;

        sock.remote_ip = remote_ip;
        sock.remote_port = remote_port;
        sock.state = SocketState::SynSent;
        sock.seq_num = 1000 + (sock_id as u32 * 500);
        let my_ip = NET_INTERFACE.lock().ip_address;
        (my_ip, sock.local_port, sock.seq_num)
    };

    // 1. Enviar segmento TCP SYN inicial
    send_tcp_segment(my_ip, remote_ip, src_port, remote_port, seq, 0, TCP_FLAG_SYN, 64240, &[]);

    // 2. Esperar respuesta del servidor (SYN-ACK) mediante sondeo del hardware E1000
    for _ in 0..15_000 {
        net_poll_rx();
        {
            let table = SOCKET_TABLE.lock();
            if let Some(Some(ref sock)) = table.get(sock_id) {
                if sock.state == SocketState::Established {
                    return Ok(());
                } else if sock.state == SocketState::Closed {
                    return Err("Conexión rechazada por el servidor remoto (RST)");
                }
            }
        }
        for _ in 0..300 {
            core::hint::spin_loop();
        }
    }

    // Timeout: restaurar socket
    {
        let mut table = SOCKET_TABLE.lock();
        if let Some(Some(ref mut sock)) = table.get_mut(sock_id) {
            sock.state = SocketState::Closed;
        }
    }
    Err("Timeout esperando establecimiento de conexión TCP (SYN-ACK)")
}

/// Envía datos a través de una conexión TCP establecida (flags PSH | ACK)
pub fn bsd_send(sock_id: usize, data: &[u8]) -> Result<usize, &'static str> {
    let (my_ip, remote_ip, local_port, remote_port, seq, ack, win) = {
        let table = SOCKET_TABLE.lock();
        let sock = table.get(sock_id).and_then(|s| s.as_ref()).ok_or("Socket descriptor inválido")?;
        if sock.state != SocketState::Established {
            return Err("Socket no conectado");
        }
        let my_ip = NET_INTERFACE.lock().ip_address;
        (my_ip, sock.remote_ip, sock.local_port, sock.remote_port, sock.seq_num, sock.ack_num, sock.window_size)
    };

    let mss = 1460_usize;
    let mut total_sent = 0;
    let mut current_seq = seq;

    for chunk in data.chunks(mss) {
        send_tcp_segment(my_ip, remote_ip, local_port, remote_port, current_seq, ack, TCP_FLAG_PSH | TCP_FLAG_ACK, win, chunk);
        current_seq = current_seq.wrapping_add(chunk.len() as u32);
        total_sent += chunk.len();
    }

    {
        let mut table = SOCKET_TABLE.lock();
        if let Some(Some(ref mut sock)) = table.get_mut(sock_id) {
            sock.seq_num = current_seq;
        }
    }

    Ok(total_sent)
}

/// Lee datos recibidos en el búfer del socket TCP
pub fn bsd_recv(sock_id: usize, buf: &mut [u8]) -> Result<usize, &'static str> {
    bsd_recv_timeout(sock_id, buf, 10_000)
}

/// Lee datos con sondeo activo y límite de espera
pub fn bsd_recv_timeout(sock_id: usize, buf: &mut [u8], timeout_spins: usize) -> Result<usize, &'static str> {
    for _ in 0..timeout_spins {
        net_poll_rx();
        {
            let mut table = SOCKET_TABLE.lock();
            if let Some(Some(ref mut sock)) = table.get_mut(sock_id) {
                if !sock.rx_buffer.is_empty() {
                    let count = buf.len().min(sock.rx_buffer.len());
                    buf[..count].copy_from_slice(&sock.rx_buffer[..count]);
                    sock.rx_buffer.drain(..count);
                    return Ok(count);
                }
                if sock.state == SocketState::CloseWait || sock.state == SocketState::Closed {
                    return Ok(0); // Fin de flujo (EOF / FIN)
                }
            } else {
                return Err("Socket descriptor inválido");
            }
        }
        for _ in 0..200 {
            core::hint::spin_loop();
        }
    }
    Ok(0)
}

/// Cierra ordenadamente el socket TCP (enviando segmento FIN | ACK)
pub fn bsd_close(sock_id: usize) {
    let (send_fin, my_ip, remote_ip, local_port, remote_port, seq, ack, win) = {
        let mut table = SOCKET_TABLE.lock();
        if let Some(Some(ref mut sock)) = table.get_mut(sock_id) {
            let active = sock.state == SocketState::Established || sock.state == SocketState::CloseWait;
            let my_ip = NET_INTERFACE.lock().ip_address;
            let params = (active, my_ip, sock.remote_ip, sock.local_port, sock.remote_port, sock.seq_num, sock.ack_num, sock.window_size);
            sock.state = SocketState::Closed;
            params
        } else {
            (false, Ipv4Addr::new(0, 0, 0, 0), Ipv4Addr::new(0, 0, 0, 0), 0, 0, 0, 0, 0)
        }
    };

    if send_fin && remote_ip.0 != [0, 0, 0, 0] {
        send_tcp_segment(my_ip, remote_ip, local_port, remote_port, seq, ack, TCP_FLAG_FIN | TCP_FLAG_ACK, win, &[]);
    }

    let mut table = SOCKET_TABLE.lock();
    if sock_id < table.len() {
        table[sock_id] = None;
    }
}

/// Parsea una respuesta HTTP/1.1 auténtica (RFC 7230)
pub fn parse_http_response(raw: &[u8]) -> Result<HttpResponse, String> {
    if raw.is_empty() {
        return Err(String::from("Respuesta HTTP vacia recibida"));
    }

    let (header_bytes, body_bytes) = if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
        (&raw[..pos], &raw[pos + 4..])
    } else if let Some(pos) = raw.windows(2).position(|w| w == b"\n\n") {
        (&raw[..pos], &raw[pos + 2..])
    } else {
        (raw, &[][..])
    };

    let header_str = core::str::from_utf8(header_bytes)
        .map_err(|_| String::from("Cabeceras HTTP con codificacion no valida"))?;

    let mut lines = header_str.lines();
    let status_line = lines.next().ok_or_else(|| String::from("Respuesta HTTP sin linea de estado"))?;

    let mut parts = status_line.splitn(3, ' ');
    let _proto = parts.next().unwrap_or("");
    let status_code = parts.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(200);
    let status_text = parts.next().unwrap_or("OK").trim();

    let mut content_type = String::from("text/plain");
    let mut content_length = body_bytes.len();

    for line in lines {
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim();
            let val = line[colon + 1..].trim();
            if key.eq_ignore_ascii_case("content-type") {
                content_type = String::from(val);
            } else if key.eq_ignore_ascii_case("content-length") {
                if let Ok(len) = val.parse::<usize>() {
                    content_length = len;
                }
            }
        }
    }

    let body_str = core::str::from_utf8(body_bytes)
        .map(|s| String::from(s))
        .unwrap_or_else(|_| format!("[Datos binarios: {} bytes]", body_bytes.len()));

    Ok(HttpResponse {
        status_code,
        status_text: String::from(status_text),
        content_type,
        content_length,
        body: body_str,
        raw_body: body_bytes.to_vec(),
    })
}

/// Cliente HTTP/1.1 auténtico que opera sobre conexiones TCP nativas
pub fn http_get(url: &str) -> Result<HttpResponse, String> {
    let clean_url = url.trim();
    let url_without_scheme = if let Some(stripped) = clean_url.strip_prefix("http://") {
        stripped
    } else if let Some(stripped) = clean_url.strip_prefix("https://") {
        stripped
    } else {
        clean_url
    };

    let (host_port, path) = match url_without_scheme.find('/') {
        Some(idx) => (&url_without_scheme[..idx], &url_without_scheme[idx..]),
        None => (url_without_scheme, "/"),
    };

    let (host, port) = match host_port.find(':') {
        Some(idx) => (&host_port[..idx], host_port[idx + 1..].parse::<u16>().unwrap_or(80)),
        None => (host_port, 80),
    };

    let target_ip = resolve_dns(host)?;

    let sock_id = bsd_socket(AF_INET, SOCK_STREAM, 0).map_err(|e| String::from(e))?;

    // Conectar vía TCP real
    if let Err(e) = bsd_connect(sock_id, target_ip, port) {
        bsd_close(sock_id);
        return Err(format!("Fallo de conexion TCP hacia {}:{}: {}", host, port, e));
    }

    // Petición HTTP/1.1 estándar (RFC 7230)
    let request_str = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: BeaverOS/0.9.0 (Bare-Metal x86_64)\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        path, host
    );

    if let Err(e) = bsd_send(sock_id, request_str.as_bytes()) {
        bsd_close(sock_id);
        return Err(format!("Error enviando peticion HTTP: {}", e));
    }

    // Recibir respuesta del servidor
    let mut response_bytes = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match bsd_recv_timeout(sock_id, &mut chunk, 15_000) {
            Ok(0) => break,
            Ok(n) => {
                response_bytes.extend_from_slice(&chunk[..n]);
                if let Some(pos) = response_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let body_received = response_bytes.len() - (pos + 4);
                    if let Ok(headers) = core::str::from_utf8(&response_bytes[..pos]) {
                        for line in headers.lines() {
                            if let Some(idx) = line.find(':') {
                                if line[..idx].trim().eq_ignore_ascii_case("content-length") {
                                    if let Ok(expected_len) = line[idx + 1..].trim().parse::<usize>() {
                                        if body_received >= expected_len {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                bsd_close(sock_id);
                return Err(format!("Error de recepcion HTTP/TCP: {}", e));
            }
        }
    }
    bsd_close(sock_id);

    if response_bytes.is_empty() {
        return Err(format!("Servidor {}:{} cerro la conexion sin enviar datos", host, port));
    }

    parse_http_response(&response_bytes)
}
