// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Primary Master ATA / IDE PIO Hard Drive Driver (Ports 0x1F0..0x1F7)
// ============================================================================

use crate::io::{inb, inw, outb, outw};
use crate::sync::SpinMutex;

const ATA_DATA: u16 = 0x1F0;
const ATA_FEATURES: u16 = 0x1F1;
const ATA_SECTOR_COUNT: u16 = 0x1F2;
const ATA_LBA_LOW: u16 = 0x1F3;
const ATA_LBA_MID: u16 = 0x1F4;
const ATA_LBA_HIGH: u16 = 0x1F5;
const ATA_DRIVE_HEAD: u16 = 0x1F6;
const ATA_STATUS_COMMAND: u16 = 0x1F7;

const STATUS_BSY: u8 = 0x80;
const STATUS_DRQ: u8 = 0x08;
const STATUS_ERR: u8 = 0x01;
const STATUS_DF: u8  = 0x20;

const CMD_READ_SECTORS: u8 = 0x20;
const CMD_WRITE_SECTORS: u8 = 0x30;
const CMD_IDENTIFY: u8 = 0xEC;
const CMD_CACHE_FLUSH: u8 = 0xE7;

#[derive(Debug, Clone, Copy)]
pub enum AtaError {
    NoDrive,
    Timeout,
    DeviceFault,
    ReadError,
    WriteError,
}

pub struct AtaDrive {
    present: bool,
    total_sectors: u32,
}

impl AtaDrive {
    pub const fn new() -> Self {
        Self {
            present: false,
            total_sectors: 0,
        }
    }

    pub fn is_present(&self) -> bool {
        self.present
    }

    pub fn total_sectors(&self) -> u32 {
        self.total_sectors
    }

    pub fn total_capacity_mb(&self) -> u32 {
        (self.total_sectors as u64 * 512 / (1024 * 1024)) as u32
    }

    // SAFETY: Acceso exclusivo a los puertos I/O del canal ATA primario (`0x1F0..0x1F7`) en Ring 0.
    pub unsafe fn init(&mut self) -> Result<(), AtaError> {
        // Seleccionar Maestro (Drive 0)
        outb(ATA_DRIVE_HEAD, 0xA0);
        outb(ATA_SECTOR_COUNT, 0);
        outb(ATA_LBA_LOW, 0);
        outb(ATA_LBA_MID, 0);
        outb(ATA_LBA_HIGH, 0);

        // Enviar comando IDENTIFY
        outb(ATA_STATUS_COMMAND, CMD_IDENTIFY);

        let status = inb(ATA_STATUS_COMMAND);
        if status == 0 || status == 0xFF {
            self.present = false;
            return Err(AtaError::NoDrive);
        }

        // Esperar a que BSY se limpie
        let mut timeout = 100_000;
        while (inb(ATA_STATUS_COMMAND) & STATUS_BSY) != 0 {
            timeout -= 1;
            if timeout == 0 {
                self.present = false;
                return Err(AtaError::Timeout);
            }
        }

        // Comprobar si el dispositivo no es ATA estándar (ej: ATAPI)
        let mid = inb(ATA_LBA_MID);
        let high = inb(ATA_LBA_HIGH);
        if mid != 0 || high != 0 {
            self.present = false;
            return Err(AtaError::NoDrive);
        }

        // Esperar a que DRQ o ERR se activen
        timeout = 100_000;
        loop {
            let s = inb(ATA_STATUS_COMMAND);
            if (s & STATUS_ERR) != 0 {
                self.present = false;
                return Err(AtaError::DeviceFault);
            }
            if (s & STATUS_DRQ) != 0 {
                break;
            }
            timeout -= 1;
            if timeout == 0 {
                self.present = false;
                return Err(AtaError::Timeout);
            }
        }

        // Leer los 512 bytes de información del IDENTIFY
        let mut identify_buf = [0_u16; 256];
        for i in 0..256 {
            identify_buf[i] = inw(ATA_DATA);
        }

        // Sectores LBA28 totales en las palabras 60 y 61
        let lba28_sectors = (identify_buf[60] as u32) | ((identify_buf[61] as u32) << 16);
        self.total_sectors = lba28_sectors;
        self.present = true;

        Ok(())
    }

    // SAFETY: Sondeo en Ring 0 de los registros de estado ATA (`0x3F6` y `0x1F7`) con el cerrojo del controlador adquirido.
    unsafe fn wait_ready(&self) -> Result<(), AtaError> {
        // Pausa de 400 ns leyendo el puerto de estado alterno (0x3F6)
        for _ in 0..4 {
            let _ = inb(0x3F6);
        }
        let mut timeout = 500_000;
        loop {
            let status = inb(ATA_STATUS_COMMAND);
            if (status & STATUS_BSY) == 0 {
                if (status & STATUS_DF) != 0 || (status & STATUS_ERR) != 0 {
                    return Err(AtaError::DeviceFault);
                }
                if (status & STATUS_DRQ) != 0 {
                    return Ok(());
                }
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(AtaError::Timeout);
            }
        }
    }

    // SAFETY: Lectura PIO de 256 palabras (`u16`) desde `ATA_DATA` hacia un búfer válido de 512 bytes en Ring 0.
    pub unsafe fn read_sector(&self, lba: u32, buf: &mut [u8; 512]) -> Result<(), AtaError> {
        if !self.present {
            return Err(AtaError::NoDrive);
        }

        // Modo LBA28 en Maestro (0xE0)
        outb(ATA_DRIVE_HEAD, 0xE0 | (((lba >> 24) & 0x0F) as u8));
        outb(ATA_FEATURES, 0x00);
        outb(ATA_SECTOR_COUNT, 1);
        outb(ATA_LBA_LOW, lba as u8);
        outb(ATA_LBA_MID, (lba >> 8) as u8);
        outb(ATA_LBA_HIGH, (lba >> 16) as u8);
        outb(ATA_STATUS_COMMAND, CMD_READ_SECTORS);

        self.wait_ready()?;

        // Leer 256 palabras de 16 bits (512 bytes)
        let words = buf.as_mut_ptr() as *mut u16;
        for i in 0..256 {
            let word = inw(ATA_DATA);
            core::ptr::write_volatile(words.add(i), word);
        }

        Ok(())
    }

    // SAFETY: Escritura PIO de 256 palabras (`u16`) desde un búfer válido de 512 bytes hacia `ATA_DATA` en Ring 0.
    pub unsafe fn write_sector(&self, lba: u32, buf: &[u8; 512]) -> Result<(), AtaError> {
        if !self.present {
            return Err(AtaError::NoDrive);
        }

        outb(ATA_DRIVE_HEAD, 0xE0 | (((lba >> 24) & 0x0F) as u8));
        outb(ATA_FEATURES, 0x00);
        outb(ATA_SECTOR_COUNT, 1);
        outb(ATA_LBA_LOW, lba as u8);
        outb(ATA_LBA_MID, (lba >> 8) as u8);
        outb(ATA_LBA_HIGH, (lba >> 16) as u8);
        outb(ATA_STATUS_COMMAND, CMD_WRITE_SECTORS);

        self.wait_ready()?;

        // Escribir 256 palabras de 16 bits (512 bytes)
        let words = buf.as_ptr() as *const u16;
        for i in 0..256 {
            let word = core::ptr::read_volatile(words.add(i));
            outw(ATA_DATA, word);
        }

        // Pausa de 400 ns y esperar que el disco limpie BSY
        for _ in 0..4 {
            let _ = inb(0x3F6);
        }
        let mut timeout = 500_000;
        while (inb(ATA_STATUS_COMMAND) & STATUS_BSY) != 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(AtaError::Timeout);
            }
        }

        // Vaciar la caché del disco para persistencia inmediata
        outb(ATA_STATUS_COMMAND, CMD_CACHE_FLUSH);
        for _ in 0..4 {
            let _ = inb(0x3F6);
        }
        timeout = 500_000;
        while (inb(ATA_STATUS_COMMAND) & STATUS_BSY) != 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(AtaError::Timeout);
            }
        }

        Ok(())
    }
}

pub static ATA_PRIMARY_MASTER: SpinMutex<AtaDrive> = SpinMutex::new(AtaDrive::new());
