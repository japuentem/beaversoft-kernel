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
// CMOS Real-Time Clock (RTC) Driver - Fecha y Hora del Hardware
// ============================================================================

const CMOS_ADDR: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

fn cmos_read(reg: u8) -> u8 {
    unsafe {
        outb(CMOS_ADDR, reg);
        crate::io::inb(CMOS_DATA)
    }
}

fn is_updating_rtc() -> bool {
    (cmos_read(0x0A) & 0x80) != 0
}

fn bcd_to_bin(val: u8) -> u8 {
    ((val >> 4) * 10) + (val & 0x0F)
}

#[derive(Copy, Clone, Debug)]
pub struct RtcDateTime {
    pub second: u8,
    pub minute: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

/// Lee la fecha y hora física del hardware desde el chip CMOS RTC
pub fn read_rtc_datetime() -> RtcDateTime {
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

    RtcDateTime { second: sec, minute: min, hour, day, month, year }
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
