// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Unified Block Device Interface (AHCI SATA DMA + ATA PIO + RamDisk)
// ============================================================================

use crate::storage::ata::ATA_PRIMARY_MASTER;
use crate::sync::SpinMutex;

pub const SECTOR_SIZE: usize = 512;
const RAMDISK_SECTORS: usize = 128; // 64 KiB para contingencia en memoria

pub struct BlockDeviceManager {
    ramdisk: [[u8; SECTOR_SIZE]; RAMDISK_SECTORS],
    use_ata: bool,
    use_ahci: bool,
    use_virtio: bool,
    ahci_port: u8,
}

impl BlockDeviceManager {
    pub const fn new() -> Self {
        Self {
            ramdisk: [[0; SECTOR_SIZE]; RAMDISK_SECTORS],
            use_ata: false,
            use_ahci: false,
            use_virtio: false,
            ahci_port: 0,
        }
    }

    pub fn set_use_ata(&mut self, enabled: bool) {
        self.use_ata = enabled;
    }

    pub fn set_use_ahci(&mut self, enabled: bool, port: u8) {
        self.use_ahci = enabled;
        self.ahci_port = port;
    }

    pub fn set_use_virtio(&mut self, enabled: bool) {
        self.use_virtio = enabled;
    }

    pub fn is_using_hardware_ata(&self) -> bool {
        self.use_ata
    }

    pub fn is_using_ahci(&self) -> bool {
        self.use_ahci
    }

    pub fn is_using_virtio(&self) -> bool {
        self.use_virtio
    }

    pub fn get_storage_medium_name(&self) -> &'static str {
        if self.use_ahci {
            "Serial ATA AHCI DMA (Aceleración Directa)"
        } else if self.use_virtio {
            "VirtIO-blk MMIO (ARM64 Nativo)"
        } else if self.use_ata {
            "ATA/IDE PIO Primario Maestro"
        } else {
            "RamDisk de Respaldo en Memoria"
        }
    }

    pub fn read_sector(&self, lba: u32, buf: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        #[cfg(target_arch = "aarch64")]
        if self.use_virtio {
            return crate::arch::aarch64::virtio_mmio::read_sector(lba as u64, buf);
        }
        if self.use_ahci {
            return crate::storage::ahci::read_sector(self.ahci_port, lba, buf);
        }
        if self.use_ata {
            let ata = ATA_PRIMARY_MASTER.lock();
            // SAFETY: Lectura de 256 palabras por PIO desde ATA Primary Master bajo el cerrojo ATA_PRIMARY_MASTER.
            unsafe {
                ata.read_sector(lba, buf).map_err(|_| "Error al leer sector de disco ATA")?;
            }
            return Ok(());
        }
        let idx = (lba as usize) % RAMDISK_SECTORS;
        buf.copy_from_slice(&self.ramdisk[idx]);
        Ok(())
    }

    pub fn write_sector(&mut self, lba: u32, buf: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        #[cfg(target_arch = "aarch64")]
        if self.use_virtio {
            return crate::arch::aarch64::virtio_mmio::write_sector(lba as u64, buf);
        }
        if self.use_ahci {
            return crate::storage::ahci::write_sector(self.ahci_port, lba, buf);
        }
        if self.use_ata {
            let ata = ATA_PRIMARY_MASTER.lock();
            // SAFETY: Escritura de 256 palabras por PIO hacia ATA Primary Master bajo el cerrojo ATA_PRIMARY_MASTER.
            unsafe {
                ata.write_sector(lba, buf).map_err(|_| "Error al escribir sector en disco ATA")?;
            }
            return Ok(());
        }
        let idx = (lba as usize) % RAMDISK_SECTORS;
        self.ramdisk[idx].copy_from_slice(buf);
        Ok(())
    }
}

pub static BLOCK_DEVICE: SpinMutex<BlockDeviceManager> = SpinMutex::new(BlockDeviceManager::new());
