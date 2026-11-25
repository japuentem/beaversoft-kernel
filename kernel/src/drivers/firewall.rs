// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: BeaverGuard Native Mini-Firewall, Rate Limiter & Auto-Ban Engine
//              Bare-metal packet filtering (UFW-equivalent) and brute-force
//              mitigation (Fail2Ban-equivalent) in pure Rust no_std
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use crate::sync::SpinMutex;
use crate::drivers::net::{Ipv4Addr, IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP};
use crate::drivers::timer::get_ticks;

pub const MAX_ALLOWED_PORTS: usize = 32;
pub const MAX_BAN_ENTRIES: usize = 32;
pub const MAX_RATE_TRACKERS: usize = 32;

/// Ventana de 1 segundo asumiendo PIT a 100 Hz (100 ticks/seg)
pub const RATE_WINDOW_TICKS: u64 = 100;
/// Límite máximo de paquetes por segundo permitidos desde una misma IP
pub const MAX_PKTS_PER_WINDOW: u32 = 120;
/// Fallos de autenticación HMAC consecutivos antes de baneo automático (Fail2Ban)
pub const MAX_AUTH_FAILURES: u32 = 5;
/// Duración del baneo automático: 60 segundos (6000 ticks)
pub const DEFAULT_BAN_TICKS: u64 = 6000;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum FirewallMode {
    /// Bloquea todo tráfico entrante no solicitado salvo puertos explícitamente permitidos
    DefaultDeny,
    /// Permite puertos abiertos salvo IPs baneadas o que excedan el Rate Limiter
    Permissive,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FirewallVerdict {
    Allow,
    DropPolicy,
    DropRateLimit,
    DropBanned,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PortRule {
    pub proto: u8,
    pub port: u16,
}

#[derive(Copy, Clone)]
pub struct BanEntry {
    pub ip: [u8; 4],
    pub until_tick: u64, // u64::MAX = baneo permanente manual
    pub reason: &'static str,
    pub active: bool,
}

#[derive(Copy, Clone)]
pub struct RateTracker {
    pub ip: [u8; 4],
    pub window_start: u64,
    pub pkt_count: u32,
    pub auth_failures: u32,
    pub active: bool,
}

pub struct FirewallState {
    pub enabled: bool,
    pub mode: FirewallMode,
    pub allowed_ports: Vec<PortRule>,
    pub bans: [BanEntry; MAX_BAN_ENTRIES],
    pub trackers: [RateTracker; MAX_RATE_TRACKERS],
    pub total_inspected: u64,
    pub total_allowed: u64,
    pub total_dropped_policy: u64,
    pub total_dropped_ratelimit: u64,
    pub total_dropped_banned: u64,
}

impl FirewallState {
    pub const fn new_empty() -> Self {
        const EMPTY_BAN: BanEntry = BanEntry {
            ip: [0; 4],
            until_tick: 0,
            reason: "",
            active: false,
        };
        const EMPTY_TRACKER: RateTracker = RateTracker {
            ip: [0; 4],
            window_start: 0,
            pkt_count: 0,
            auth_failures: 0,
            active: false,
        };
        Self {
            enabled: true,
            mode: FirewallMode::DefaultDeny,
            allowed_ports: Vec::new(),
            bans: [EMPTY_BAN; MAX_BAN_ENTRIES],
            trackers: [EMPTY_TRACKER; MAX_RATE_TRACKERS],
            total_inspected: 0,
            total_allowed: 0,
            total_dropped_policy: 0,
            total_dropped_ratelimit: 0,
            total_dropped_banned: 0,
        }
    }

    pub fn ensure_defaults(&mut self) {
        if self.allowed_ports.is_empty() {
            // Puertos esenciales por defecto: BeaverMesh (:4242 UDP) y cliente DHCP (:68 UDP)
            self.allowed_ports.push(PortRule { proto: IP_PROTO_UDP, port: 4242 });
            self.allowed_ports.push(PortRule { proto: IP_PROTO_UDP, port: 68 });
        }
    }

    pub fn is_banned(&mut self, ip: [u8; 4], now: u64) -> bool {
        for entry in self.bans.iter_mut() {
            if entry.active && entry.ip == ip {
                if entry.until_tick != u64::MAX && now >= entry.until_tick {
                    entry.active = false;
                    return false;
                }
                return true;
            }
        }
        false
    }

    pub fn ban_ip(&mut self, ip: [u8; 4], duration_ticks: Option<u64>, reason: &'static str, now: u64) {
        let until = match duration_ticks {
            Some(d) => now.saturating_add(d),
            None => u64::MAX,
        };
        // Actualizar si ya existe
        for entry in self.bans.iter_mut() {
            if entry.active && entry.ip == ip {
                entry.until_tick = until;
                entry.reason = reason;
                return;
            }
        }
        // Buscar hueco libre o expirado
        for entry in self.bans.iter_mut() {
            if !entry.active || (entry.until_tick != u64::MAX && now >= entry.until_tick) {
                *entry = BanEntry {
                    ip,
                    until_tick: until,
                    reason,
                    active: true,
                };
                return;
            }
        }
        // Si la tabla está llena, reemplazar el primer baneo temporal
        for entry in self.bans.iter_mut() {
            if entry.until_tick != u64::MAX {
                *entry = BanEntry {
                    ip,
                    until_tick: until,
                    reason,
                    active: true,
                };
                return;
            }
        }
    }

    pub fn unban_ip(&mut self, ip: [u8; 4]) -> bool {
        let mut removed = false;
        for entry in self.bans.iter_mut() {
            if entry.active && entry.ip == ip {
                entry.active = false;
                removed = true;
            }
        }
        for t in self.trackers.iter_mut() {
            if t.active && t.ip == ip {
                t.pkt_count = 0;
                t.auth_failures = 0;
            }
        }
        removed
    }

    fn get_or_insert_tracker(&mut self, ip: [u8; 4], now: u64) -> usize {
        for (i, t) in self.trackers.iter().enumerate() {
            if t.active && t.ip == ip {
                return i;
            }
        }
        for (i, t) in self.trackers.iter_mut().enumerate() {
            if !t.active {
                *t = RateTracker {
                    ip,
                    window_start: now,
                    pkt_count: 0,
                    auth_failures: 0,
                    active: true,
                };
                return i;
            }
        }
        // Evicción del tracker más antiguo
        let mut oldest_idx = 0;
        let mut oldest_tick = u64::MAX;
        for (i, t) in self.trackers.iter().enumerate() {
            if t.window_start < oldest_tick {
                oldest_tick = t.window_start;
                oldest_idx = i;
            }
        }
        self.trackers[oldest_idx] = RateTracker {
            ip,
            window_start: now,
            pkt_count: 0,
            auth_failures: 0,
            active: true,
        };
        oldest_idx
    }

    pub fn allow_port(&mut self, proto: u8, port: u16) -> bool {
        self.ensure_defaults();
        if self.allowed_ports.iter().any(|r| r.proto == proto && r.port == port) {
            return true;
        }
        if self.allowed_ports.len() >= MAX_ALLOWED_PORTS {
            return false;
        }
        self.allowed_ports.push(PortRule { proto, port });
        true
    }

    pub fn deny_port(&mut self, proto: u8, port: u16) -> bool {
        self.ensure_defaults();
        let before = self.allowed_ports.len();
        self.allowed_ports.retain(|r| !(r.proto == proto && r.port == port));
        self.allowed_ports.len() < before
    }
}

pub static FIREWALL: SpinMutex<FirewallState> = SpinMutex::new(FirewallState::new_empty());

/// Inspecciona cada paquete IPv4 entrante aplicando Auto-Ban, Rate Limiting y reglas de puerto
pub fn inspect_incoming_ipv4(
    src_ip: Ipv4Addr,
    proto: u8,
    src_port: u16,
    dst_port: u16,
) -> FirewallVerdict {
    let mut fw = match FIREWALL.try_lock() {
        Some(guard) => guard,
        None => return FirewallVerdict::Allow,
    };
    fw.ensure_defaults();
    fw.total_inspected += 1;

    if !fw.enabled {
        fw.total_allowed += 1;
        return FirewallVerdict::Allow;
    }

    let now = get_ticks();
    let ip = src_ip.0;

    // 1. Verificar tabla de IPs baneadas (Fail2Ban / bloqueo manual)
    if fw.is_banned(ip, now) {
        fw.total_dropped_banned += 1;
        return FirewallVerdict::DropBanned;
    }

    // 2. Rate Limiter por IP en ventana deslizante de 1 segundo
    let idx = fw.get_or_insert_tracker(ip, now);
    if now.saturating_sub(fw.trackers[idx].window_start) >= RATE_WINDOW_TICKS {
        fw.trackers[idx].window_start = now;
        fw.trackers[idx].pkt_count = 0;
    }
    fw.trackers[idx].pkt_count = fw.trackers[idx].pkt_count.saturating_add(1);
    if fw.trackers[idx].pkt_count > MAX_PKTS_PER_WINDOW {
        fw.ban_ip(ip, Some(DEFAULT_BAN_TICKS), "Rate-Limit Flood", now);
        fw.total_dropped_ratelimit += 1;
        return FirewallVerdict::DropRateLimit;
    }

    // 3. Reglas de puerto (en modo DefaultDeny)
    if fw.mode == FirewallMode::DefaultDeny {
        let allowed = match proto {
            IP_PROTO_ICMP => true, // ICMP pasa rate-limit y permite ping/diagnóstico
            IP_PROTO_UDP => {
                // Permitir respuestas DNS (src_port == 53) y puertos en allowlist (ej. 4242, 68)
                src_port == 53 || fw.allowed_ports.iter().any(|r| r.proto == IP_PROTO_UDP && r.port == dst_port)
            }
            IP_PROTO_TCP => {
                // Permitir conexiones salientes establecidas (src_port == 80 / 443) o puertos permitidos
                src_port == 80
                    || src_port == 443
                    || fw.allowed_ports.iter().any(|r| r.proto == IP_PROTO_TCP && r.port == dst_port)
            }
            _ => false,
        };
        if !allowed {
            fw.total_dropped_policy += 1;
            return FirewallVerdict::DropPolicy;
        }
    }

    fw.total_allowed += 1;
    FirewallVerdict::Allow
}

/// Registra un fallo de autenticación criptográfica (ej. HMAC inválido en BeaverMesh)
/// y banea automáticamente la IP si supera `MAX_AUTH_FAILURES` (comportamiento Fail2Ban).
pub fn record_auth_failure(src_ip: Ipv4Addr) -> bool {
    let mut fw = match FIREWALL.try_lock() {
        Some(guard) => guard,
        None => return false,
    };
    fw.ensure_defaults();
    let now = get_ticks();
    let ip = src_ip.0;
    let idx = fw.get_or_insert_tracker(ip, now);
    fw.trackers[idx].auth_failures = fw.trackers[idx].auth_failures.saturating_add(1);
    if fw.trackers[idx].auth_failures >= MAX_AUTH_FAILURES {
        fw.ban_ip(ip, Some(DEFAULT_BAN_TICKS), "HMAC Auth Brute-Force", now);
        return true;
    }
    false
}
