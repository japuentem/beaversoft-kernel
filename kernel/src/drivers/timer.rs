// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Programmable Interval Timer (PIT 8254) Driver - System Heartbeat
// ============================================================================

use core::sync::atomic::{AtomicU64, Ordering};
use crate::io::outb;

const PIT_CHANNEL_0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;
const PIT_BASE_FREQUENCY: u32 = 1193182;

static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init_pit(target_hz: u32) {
    let divisor = (PIT_BASE_FREQUENCY / target_hz) as u16;

    // SAFETY: Programación de los puertos I/O del temporizador PIT 8254 (0x43 comando y 0x40 canal 0) en Ring 0.
    unsafe {
        // Modo 3: Generador de onda cuadrada, lectura/escritura Byte bajo luego Byte alto
        outb(PIT_COMMAND, 0x36);
        outb(PIT_CHANNEL_0, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL_0, ((divisor >> 8) & 0xFF) as u8);
    }
}

pub fn on_timer_tick() -> u64 {
    TICKS.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn get_ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

pub fn get_uptime_seconds() -> u64 {
    get_ticks() / 100
}

// ============================================================================
// CMOS Real-Time Clock (RTC) Driver - Fecha y Hora del Hardware (x86_64)
// ============================================================================

#[cfg(target_arch = "x86_64")]
const CMOS_ADDR: u16 = 0x70;
#[cfg(target_arch = "x86_64")]
const CMOS_DATA: u16 = 0x71;

#[cfg(target_arch = "x86_64")]
fn cmos_read(reg: u8) -> u8 {
    // SAFETY: Selección de registro CMOS en puerto 0x70 y lectura del dato en 0x71 en modo supervisor.
    unsafe {
        outb(CMOS_ADDR, reg);
        crate::io::inb(CMOS_DATA)
    }
}

#[cfg(target_arch = "x86_64")]
fn is_updating_rtc() -> bool {
    (cmos_read(0x0A) & 0x80) != 0
}

#[cfg(target_arch = "x86_64")]
fn bcd_to_bin(val: u8) -> u8 {
    ((val >> 4) * 10) + (val & 0x0F)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RtcDateTime {
    pub second: u8,
    pub minute: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

// ============================================================================
// ARM PrimeCell PL031 Real-Time Clock (RTC MMIO 0x09010000)
// ============================================================================
pub const PL031_RTC_BASE: usize = 0x0901_0000;

/// Lee el contador de segundos de la época Unix directamente del chip PL031 RTC
pub fn read_pl031_unix_timestamp() -> u64 {
    // SAFETY: Lectura MMIO volátil de 32 bits del registro RTC_DR (offset 0x00)
    // mapeado en la región Device de 0..1 GiB en EL1.
    let secs = unsafe { core::ptr::read_volatile(PL031_RTC_BASE as *const u32) };
    secs as u64
}

/// Comprueba si el hardware PL031 RTC está presente leyendo los registros de identificación periférica
pub fn is_pl031_present() -> bool {
    // Offset 0xFE0 = 0x31 (PartNumber0), 0xFE4 = 0x10 (PartNumber1), 0xFE8 = 0x04 (Designer0 ARM)
    let id0 = unsafe { core::ptr::read_volatile((PL031_RTC_BASE + 0xFE0) as *const u32) } & 0xFF;
    let id1 = unsafe { core::ptr::read_volatile((PL031_RTC_BASE + 0xFE4) as *const u32) } & 0xFF;
    id0 == 0x31 && id1 == 0x10
}

/// Identifica el tipo y bus del chip RTC del hardware actual
pub fn get_rtc_source_name() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        if is_pl031_present() {
            "ARM PrimeCell PL031 RTC (MMIO 0x09010000)"
        } else {
            "ARM Generic Timer (Emulated RTC)"
        }
    }
    #[cfg(target_arch = "x86_64")]
    {
        "CMOS Real-Time Clock MC146818 (I/O 0x70/0x71)"
    }
}

/// Determina si un año gregoriano es bisiesto
pub fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Convierte una estructura de fecha y hora gregoriana a segundos Unix (desde 1970-01-01 00:00:00 UTC)
pub fn datetime_to_unix(dt: &RtcDateTime) -> u64 {
    let mut days = 0u64;
    for y in 1970..dt.year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    for m in 1..dt.month {
        days += days_in_month(m, dt.year) as u64;
    }
    days += (dt.day.saturating_sub(1)) as u64;
    days * 86400 + (dt.hour as u64) * 3600 + (dt.minute as u64) * 60 + (dt.second as u64)
}

/// Convierte un número de segundos de la época Unix a una estructura de fecha y hora gregoriana (UTC)
pub fn unix_to_datetime(secs: u64) -> RtcDateTime {
    let second = (secs % 60) as u8;
    let total_mins = secs / 60;
    let minute = (total_mins % 60) as u8;
    let total_hours = total_mins / 60;
    let hour = (total_hours % 24) as u8;
    let mut remaining_days = (total_hours / 24) as u32;

    let mut year = 1970u16;
    loop {
        let leap = is_leap_year(year);
        let days_in_cur_year = if leap { 366 } else { 365 };
        if remaining_days < days_in_cur_year {
            break;
        }
        remaining_days -= days_in_cur_year;
        year += 1;
    }

    let mut month = 1u8;
    for m in 1..=12 {
        let dim = days_in_month(m, year) as u32;
        if remaining_days < dim {
            month = m;
            break;
        }
        remaining_days -= dim;
    }
    let day = (remaining_days + 1) as u8;

    RtcDateTime { second, minute, hour, day, month, year }
}

/// Obtiene la marca de tiempo de la época Unix actual (segundos UTC) desde el hardware
pub fn get_unix_timestamp() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let secs = read_pl031_unix_timestamp();
        if secs > 0 {
            return secs;
        }
    }

    let dt = read_rtc_datetime();
    datetime_to_unix(&dt)
}

/// Lee la fecha y hora física del hardware desde el chip RTC (CMOS en x86 / PL031 en ARM64)
pub fn read_rtc_datetime() -> RtcDateTime {
    #[cfg(target_arch = "aarch64")]
    {
        let secs = read_pl031_unix_timestamp();
        if secs > 0 {
            return unix_to_datetime(secs);
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        let mut retries = 0;
        while is_updating_rtc() && retries < 1000 {
            core::hint::spin_loop();
            retries += 1;
        }

        let mut sec = cmos_read(0x00);
        let mut min = cmos_read(0x02);
        let mut hour = cmos_read(0x04);
        let mut day = cmos_read(0x07);
        let mut month = cmos_read(0x08);
        let mut year = cmos_read(0x09) as u16;

        let register_b = cmos_read(0x0B);

        // Convertir de BCD a binario si el RTC opera en BCD (típico en PC x86)
        if (register_b & 0x04) == 0 {
            sec = bcd_to_bin(sec);
            min = bcd_to_bin(min);
            hour = bcd_to_bin(hour & 0x7F) | (hour & 0x80);
            day = bcd_to_bin(day);
            month = bcd_to_bin(month);
            year = bcd_to_bin(year as u8) as u16;
        }

        // Reloj 12 horas a 24 horas si aplica
        if (register_b & 0x02) == 0 && (hour & 0x80) != 0 {
            hour = ((hour & 0x7F) + 12) % 24;
        }

        year += 2000;
        if month == 0 || month > 12 {
            month = 1;
        }
        if day == 0 || day > 31 {
            day = 1;
        }

        return RtcDateTime { second: sec, minute: min, hour, day, month, year };
    }

    #[allow(unreachable_code)]
    RtcDateTime { second: 0, minute: 0, hour: 0, day: 1, month: 1, year: 2026 }
}

/// Desfase de zona horaria local respecto a UTC (México Centro: UTC-6).
/// El RTC de QEMU (y de la mayoría de equipos con firmware UEFI) entrega la hora en UTC.
pub const TZ_OFFSET_HOURS: i8 = -6;

/// Número de días del mes indicado (considera años bisiestos gregorianos)
pub fn days_in_month(month: u8, year: u16) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Día de la semana (algoritmo de Tomohiko Sakamoto, aritmética entera).
/// Devuelve 0 = Domingo, 1 = Lunes, ..., 6 = Sábado.
pub fn day_of_week(year: u16, month: u8, day: u8) -> u8 {
    const T: [u16; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = year as u32;
    if month < 3 {
        y -= 1;
    }
    let m = (month.clamp(1, 12) - 1) as usize;
    ((y + y / 4 - y / 100 + y / 400 + T[m] as u32 + day as u32) % 7) as u8
}

/// Fecha y hora local: lee el RTC (UTC) y aplica `TZ_OFFSET_HOURS` con acarreo de día/mes/año
pub fn local_datetime() -> RtcDateTime {
    let mut dt = read_rtc_datetime();
    let mut hour = dt.hour as i16 + TZ_OFFSET_HOURS as i16;

    if hour < 0 {
        hour += 24;
        // Retroceder un día
        if dt.day > 1 {
            dt.day -= 1;
        } else {
            if dt.month > 1 {
                dt.month -= 1;
            } else {
                dt.month = 12;
                dt.year -= 1;
            }
            dt.day = days_in_month(dt.month, dt.year);
        }
    } else if hour >= 24 {
        hour -= 24;
        // Avanzar un día
        if dt.day < days_in_month(dt.month, dt.year) {
            dt.day += 1;
        } else {
            dt.day = 1;
            if dt.month < 12 {
                dt.month += 1;
            } else {
                dt.month = 1;
                dt.year += 1;
            }
        }
    }

    dt.hour = hour as u8;
    dt
}
