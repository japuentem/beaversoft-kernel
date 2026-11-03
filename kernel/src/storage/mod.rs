// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Storage Subsystem, Block Devices & Resilient Anti-Brick Boot Architecture
// ============================================================================

pub mod ata;
pub mod block_device;
pub mod crc32;
pub mod ab_slot;
pub mod beaverfs;
pub mod ahci;
pub mod gpt;
pub mod nvme;
pub mod installer;

use crate::storage::ata::ATA_PRIMARY_MASTER;
use crate::storage::block_device::BLOCK_DEVICE;
use crate::storage::ab_slot::BOOT_SLOT_MANAGER;
use crate::vga::VgaColor;
use crate::{kprint, kprint_color};

pub fn init_storage() {
    kprint!("[INIT] Detectando bus de almacenamiento ATA/IDE primario...    ");

    {
        let mut ata = ATA_PRIMARY_MASTER.lock();
        let detected = unsafe { ata.init() };

        match detected {
            Ok(()) => {
                let total_mb = ata.total_capacity_mb();
                let total_sectors = ata.total_sectors();
                kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] ATA Disco Activo ({} MB, {} sectores)\n", total_mb, total_sectors);
                BLOCK_DEVICE.lock().set_use_ata(true);
            }
            Err(_) => {
                kprint_color!(VgaColor::Yellow, VgaColor::Black, "[RAM] Sin ATA físico -> RamDisk de Respaldo Activo\n");
                BLOCK_DEVICE.lock().set_use_ata(false);
            }
        }
    }

    // Inicializar el subsistema A/B y verificar la salud del Sector Cero
    kprint!("[INIT] Verificando Sector Cero y Topología de Ranuras A/B...    ");
    BOOT_SLOT_MANAGER.lock().init();
    kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] Protegido con Espejo LBA 16\n");

    // Marcar el arranque actual como exitoso
    BOOT_SLOT_MANAGER.lock().mark_boot_successful();

    // 9.0 Inicializar y montar sistema de archivos persistente BeaverFS en disco ATA
    beaverfs::init_beaverfs();

    // 9.1 Inicializar Controladores de Almacenamiento Moderno (AHCI, NVMe y GPT)
    ahci::init();
    gpt::init();
    nvme::init();
}
