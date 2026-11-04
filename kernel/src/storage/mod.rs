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
    // 1. Detectar e inicializar aceleración Serial ATA AHCI moderna en el bus PCI
    kprint!("[INIT] Detectando controladora PCI Serial ATA AHCI...          ");
    let ahci_detected = ahci::init();

    let mut storage_configured = false;

    if ahci_detected {
        let ahci = ahci::AHCI_CONTROLLER.lock();
        if let Some(port) = ahci.active_sata_port {
            let model = ahci.get_port_model(port);
            let mb = ahci.get_port_capacity_mb(port);
            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[OK] SATA Port {} DMA ({}, {} MB)\n", port, model, mb);
            drop(ahci);
            BLOCK_DEVICE.lock().set_use_ahci(true, port);
            storage_configured = true;
        } else {
            kprint_color!(VgaColor::Yellow, VgaColor::Black, "[INFO] Controladora presente sin discos SATA activos\n");
        }
    } else {
        kprint_color!(VgaColor::Yellow, VgaColor::Black, "[INFO] Sin AHCI en bus PCI -> Verificando ATA/IDE legado\n");
    }

    // 2. Si no hay unidad SATA AHCI activa, recurrir al controlador legado ATA/IDE PIO
    if !storage_configured {
        kprint!("[INIT] Detectando bus de almacenamiento ATA/IDE primario...    ");
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

    // Inicializar y montar sistema de archivos persistente BeaverFS
    beaverfs::init_beaverfs();

    gpt::init();
    nvme::init();
}
