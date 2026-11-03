// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: PCI Bus Controller & Device Enumerator (Mechanism #1)
// ============================================================================

use crate::io::{inl, outl};

const PCI_CONFIG_ADDRESS_PORT: u16 = 0xCF8;
const PCI_CONFIG_DATA_PORT: u16 = 0xCFC;

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub bar0: u32,
}

pub fn pci_read_config_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC)
        | 0x80000000;

    unsafe {
        outl(PCI_CONFIG_ADDRESS_PORT, address);
        inl(PCI_CONFIG_DATA_PORT)
    }
}

pub fn pci_write_config_u32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let address = ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC)
        | 0x80000000;

    unsafe {
        outl(PCI_CONFIG_ADDRESS_PORT, address);
        outl(PCI_CONFIG_DATA_PORT, value);
    }
}

/// Busca el primer dispositivo de pantalla / GPU (Clase 0x03) en el bus PCI
pub fn find_vga_device() -> Option<PciDevice> {
    for bus in 0..=4 {
        for device in 0..32 {
            let id_reg = pci_read_config_u32(bus, device, 0, 0x00);
            let vendor_id = (id_reg & 0xFFFF) as u16;
            if vendor_id == 0xFFFF || vendor_id == 0x0000 {
                continue;
            }
            let device_id = ((id_reg >> 16) & 0xFFFF) as u16;

            let class_reg = pci_read_config_u32(bus, device, 0, 0x08);
            let class_code = ((class_reg >> 24) & 0xFF) as u8;
            let subclass = ((class_reg >> 16) & 0xFF) as u8;

            if class_code == 0x03 {
                // Controlador de pantalla encontrado
                let bar0_raw = pci_read_config_u32(bus, device, 0, 0x10);
                let bar0 = bar0_raw & 0xFFFFFFF0; // Máscara para dirección base de memoria (bits 4..31)

                return Some(PciDevice {
                    bus,
                    device,
                    function: 0,
                    vendor_id,
                    device_id,
                    class_code,
                    subclass,
                    bar0,
                });
            }
        }
    }
    None
}
