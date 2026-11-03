// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Unified Block Device Interface (Hardware ATA PIO + Fallback RamDisk)
// ============================================================================

use crate::storage::ata::ATA_PRIMARY_MASTER;
use crate::sync::SpinMutex;

pub const SECTOR_SIZE: usize = 512;
const RAMDISK_SECTORS: usize = 128; // 64 KiB para contingencia en memoria

pub struct BlockDeviceManager {
    ramdisk: [[u8; SECTOR_SIZE]; RAMDISK_SECTORS],
    use_ata: bool,
}

impl BlockDeviceManager {
    pub const fn new() -> Self {
        Self {
            ramdisk: [[0; SECTOR_SIZE]; RAMDISK_SECTORS],
            use_ata: false,
        }
    }

    pub fn set_use_ata(&mut self, enabled: bool) {
        self.use_ata = enabled;
    }

    pub fn is_using_hardware_ata(&self) -> bool {
        self.use_ata
    }

    pub fn read_sector(&self, lba: u32, buf: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        if self.use_ata {
            let ata = ATA_PRIMARY_MASTER.lock();
            unsafe {
                ata.read_sector(lba, buf).map_err(|_| "Error al leer sector de disco ATA")?;
            }
            Ok(())
        } else {
            let idx = (lba as usize) % RAMDISK_SECTORS;
            buf.copy_from_slice(&self.ramdisk[idx]);
            Ok(())
        }
    }

    pub fn write_sector(&mut self, lba: u32, buf: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        if self.use_ata {
            let ata = ATA_PRIMARY_MASTER.lock();
            unsafe {
                ata.write_sector(lba, buf).map_err(|_| "Error al escribir sector en disco ATA")?;
            }
            Ok(())
        } else {
            let idx = (lba as usize) % RAMDISK_SECTORS;
            self.ramdisk[idx].copy_from_slice(buf);
            Ok(())
        }
    }
}

pub static BLOCK_DEVICE: SpinMutex<BlockDeviceManager> = SpinMutex::new(BlockDeviceManager::new());
