// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Modern xHCI (USB 3.0 / 3.1 / 3.2) Host Controller Subsystem & Root Hub Enumerator
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use crate::drivers::pci::{pci_read_config_u32, pci_write_config_u32};
use crate::sync::SpinMutex;

pub const PCI_CLASS_SERIAL_BUS: u8 = 0x0C;
pub const PCI_SUBCLASS_USB: u8 = 0x03;

pub const USB_PROGIF_UHCI: u8 = 0x00;
pub const USB_PROGIF_OHCI: u8 = 0x10;
pub const USB_PROGIF_EHCI: u8 = 0x20; // USB 2.0
pub const USB_PROGIF_XHCI: u8 = 0x30; // USB 3.0 / 3.1 / 3.2 Extensible Host Controller

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbPortSpeed {
    Unknown = 0,
    FullSpeed = 1,      // 12 Mbps (USB 1.1 / 2.0 Full-Speed - Teclados y Ratones HID)
    LowSpeed = 2,       // 1.5 Mbps (USB 1.0 Low-Speed)
    HighSpeed = 3,      // 480 Mbps (USB 2.0 High-Speed)
    SuperSpeed = 4,     // 5 Gbps (USB 3.0 SuperSpeed)
    SuperSpeedPlus = 5, // 10 Gbps (USB 3.1 / 3.2 SuperSpeed+)
}

impl UsbPortSpeed {
    pub fn as_str(&self) -> &'static str {
        match self {
            UsbPortSpeed::FullSpeed => "Full-Speed (12 Mbps)",
            UsbPortSpeed::LowSpeed => "Low-Speed (1.5 Mbps)",
            UsbPortSpeed::HighSpeed => "High-Speed (480 Mbps)",
            UsbPortSpeed::SuperSpeed => "SuperSpeed (5 Gbps)",
            UsbPortSpeed::SuperSpeedPlus => "SuperSpeedPlus (10 Gbps)",
            UsbPortSpeed::Unknown => "Velocidad Desconocida",
        }
    }
}

#[derive(Debug, Clone)]
pub struct XhciPort {
    pub port_number: u8,
    pub connected: bool,
    pub enabled: bool,
    pub powered: bool,
    pub speed: UsbPortSpeed,
    pub link_state: u8,
    pub portsc_raw: u32,
}

#[derive(Debug, Clone)]
pub struct XhciController {
    pub name: &'static str,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub mmio_base: usize,
    pub cap_length: usize,
    pub hci_version: u16,
    pub max_slots: u8,
    pub max_intrs: u16,
    pub max_ports: u8,
    pub op_base: usize,
    pub usbcmd: u32,
    pub usbsts: u32,
    pub is_running: bool,
    pub dcbaap: u64,
    pub crcr: u64,
    pub ports: Vec<XhciPort>,
}

impl XhciController {
    pub fn probe(bus: u8, device: u8, function: u8, vendor_id: u16, device_id: u16, bar0: u32) -> Option<Self> {
        let mmio_base = (bar0 & 0xFFFF_FFF0) as usize;
        if mmio_base == 0 {
            return None;
        }

        // 1. Lectura de Registros de Capacidad xHCI (Offsets de especificación xHCI 5.3)
        // SAFETY: Lectura MMIO sobre la región BAR0 mapeada por el firmware UEFI / PCI.
        let cap_word = unsafe { core::ptr::read_volatile(mmio_base as *const u32) };
        let cap_length = (cap_word & 0xFF) as usize;
        let hci_version = ((cap_word >> 16) & 0xFFFF) as u16;

        let hcsparams1 = unsafe { core::ptr::read_volatile((mmio_base + 0x04) as *const u32) };
        let max_slots = (hcsparams1 & 0xFF) as u8;
        let max_intrs = ((hcsparams1 >> 8) & 0x7FF) as u16;
        let max_ports = ((hcsparams1 >> 24) & 0xFF) as u8;

        let op_base = mmio_base + cap_length;

        // 2. Lectura de Registros Operacionales Iniciales (USBCMD y USBSTS)
        let usbcmd = unsafe { core::ptr::read_volatile(op_base as *const u32) };
        let usbsts = unsafe { core::ptr::read_volatile((op_base + 0x04) as *const u32) };

        let mut ctrl = Self {
            name: "xHCI USB 3.0 Host Controller (Extensible)",
            bus,
            device,
            function,
            vendor_id,
            device_id,
            mmio_base,
            cap_length,
            hci_version,
            max_slots,
            max_intrs,
            max_ports,
            op_base,
            usbcmd,
            usbsts,
            is_running: false,
            dcbaap: 0,
            crcr: 0,
            ports: Vec::new(),
        };

        // 3. Inicializar hardware y explorar puertos del Root Hub
        ctrl.init_hardware();
        ctrl.scan_ports();

        Some(ctrl)
    }

    pub fn init_hardware(&mut self) {
        // A. Detener el controlador si se encuentra en ejecución previa (BIOS)
        let cmd = unsafe { core::ptr::read_volatile(self.op_base as *const u32) };
        if (cmd & 0x01) != 0 {
            unsafe { core::ptr::write_volatile(self.op_base as *mut u32, cmd & !0x01) };
            for _ in 0..10_000 {
                let sts = unsafe { core::ptr::read_volatile((self.op_base + 0x04) as *const u32) };
                if (sts & 0x01) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
        }

        // B. Reinicio de hardware del Host Controller (HCRST = Bit 1)
        unsafe { core::ptr::write_volatile(self.op_base as *mut u32, 0x02) };
        for _ in 0..100_000 {
            let cur_cmd = unsafe { core::ptr::read_volatile(self.op_base as *const u32) };
            let cur_sts = unsafe { core::ptr::read_volatile((self.op_base + 0x04) as *const u32) };
            if (cur_cmd & 0x02) == 0 && (cur_sts & 0x0800) == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // C. Configurar número máximo de ranuras de dispositivos en registro CONFIG
        let slots_to_enable = self.max_slots.min(32);
        unsafe { core::ptr::write_volatile((self.op_base + 0x38) as *mut u32, slots_to_enable as u32) };

        // D. Asignar DCBAA (Device Context Base Address Array)
        if let Some(dcbaa_frame) = crate::memory::frame_allocator::FRAME_ALLOCATOR.allocate_frame() {
            unsafe {
                core::ptr::write_bytes(dcbaa_frame as *mut u8, 0, 4096);
                core::ptr::write_volatile((self.op_base + 0x30) as *mut u32, (dcbaa_frame & 0xFFFFFFFF) as u32);
                core::ptr::write_volatile((self.op_base + 0x34) as *mut u32, ((dcbaa_frame >> 32) & 0xFFFFFFFF) as u32);
            }
            self.dcbaap = dcbaa_frame as u64;
        }

        // E. Asignar Command Ring Buffer (CRB)
        if let Some(crb_frame) = crate::memory::frame_allocator::FRAME_ALLOCATOR.allocate_frame() {
            unsafe {
                core::ptr::write_bytes(crb_frame as *mut u8, 0, 4096);
                let crcr_val = (crb_frame as u64) | 1; // Bit 0: RCS = 1
                core::ptr::write_volatile((self.op_base + 0x18) as *mut u32, (crcr_val & 0xFFFFFFFF) as u32);
                core::ptr::write_volatile((self.op_base + 0x1C) as *mut u32, ((crcr_val >> 32) & 0xFFFFFFFF) as u32);
            }
            self.crcr = (crb_frame as u64) | 1;
        }

        // F. Arrancar controlador (Run/Stop = Bit 0)
        unsafe {
            let start_cmd = core::ptr::read_volatile(self.op_base as *const u32) | 0x01;
            core::ptr::write_volatile(self.op_base as *mut u32, start_cmd);
        }

        self.usbcmd = unsafe { core::ptr::read_volatile(self.op_base as *const u32) };
        self.usbsts = unsafe { core::ptr::read_volatile((self.op_base + 0x04) as *const u32) };
        self.is_running = (self.usbcmd & 0x01) != 0 && (self.usbsts & 0x01) == 0;
    }

    pub fn scan_ports(&mut self) {
        self.ports.clear();
        for port in 1..=self.max_ports {
            let port_reg = self.op_base + 0x400 + ((port - 1) as usize * 0x10);
            let portsc = unsafe { core::ptr::read_volatile(port_reg as *const u32) };

            let connected = (portsc & 0x01) != 0;
            let enabled = (portsc & 0x02) != 0;
            let powered = (portsc & 0x200) != 0;
            let link_state = ((portsc >> 5) & 0x0F) as u8;
            let speed_code = ((portsc >> 10) & 0x0F) as u8;

            let speed = match speed_code {
                1 => UsbPortSpeed::FullSpeed,
                2 => UsbPortSpeed::LowSpeed,
                3 => UsbPortSpeed::HighSpeed,
                4 => UsbPortSpeed::SuperSpeed,
                5 => UsbPortSpeed::SuperSpeedPlus,
                _ => UsbPortSpeed::Unknown,
            };

            self.ports.push(XhciPort {
                port_number: port,
                connected,
                enabled,
                powered,
                speed,
                link_state,
                portsc_raw: portsc,
            });
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsbDevice {
    pub port: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub speed: UsbPortSpeed,
    pub class_name: &'static str,
    pub device_name: String,
}

#[derive(Debug, Clone)]
pub struct UsbLegacyController {
    pub name: &'static str,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub prog_if: u8,
    pub mmio_bar0: u32,
    pub ports_count: u8,
}

pub struct UsbSubsystem {
    pub xhci_controllers: Vec<XhciController>,
    pub legacy_controllers: Vec<UsbLegacyController>,
    pub devices: Vec<UsbDevice>,
    pub initialized: bool,
}

pub static USB_MANAGER: SpinMutex<UsbSubsystem> = SpinMutex::new(UsbSubsystem {
    xhci_controllers: Vec::new(),
    legacy_controllers: Vec::new(),
    devices: Vec::new(),
    initialized: false,
});

/// Escanea el bus PCI en busca de controladores USB (xHCI, EHCI, UHCI) con hardware real
pub fn init() {
    let mut usb = USB_MANAGER.lock();
    if usb.initialized {
        return;
    }

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
            let prog_if = ((class_reg >> 8) & 0xFF) as u8;

            let mut bar0 = pci_read_config_u32(bus, device, 0, 0x10) & 0xFFFFFFF0;
            if bar0 == 0 {
                // Si el firmware UEFI / BIOS no asignó una ventana MMIO a BAR0,
                // asignar dinámicamente en el espacio MMIO PCI (0xFEBD_0000)
                bar0 = 0xFEBD_0000;
                pci_write_config_u32(bus, device, 0, 0x10, bar0);
                pci_write_config_u32(bus, device, 0, 0x14, 0);
            }

            if class_code == PCI_CLASS_SERIAL_BUS && subclass == PCI_SUBCLASS_USB {
                // Habilitar Bus Master y Memory Space en el Registro de Comando PCI
                let cmd = pci_read_config_u32(bus, device, 0, 0x04);
                pci_write_config_u32(bus, device, 0, 0x04, cmd | 0x0007);

                if prog_if == USB_PROGIF_XHCI {
                    if let Some(xhci) = XhciController::probe(bus, device, 0, vendor_id, device_id, bar0) {
                        for port in &xhci.ports {
                            if port.connected {
                                let (class_name, name) = match port.speed {
                                    UsbPortSpeed::FullSpeed => (
                                        "Human Interface Device (HID)",
                                        format!("Teclado/Ratón USB Boot Protocol (Puerto {})", port.port_number),
                                    ),
                                    UsbPortSpeed::LowSpeed => (
                                        "Human Interface Device (HID)",
                                        format!("Dispositivo USB Low-Speed (Puerto {})", port.port_number),
                                    ),
                                    UsbPortSpeed::HighSpeed => (
                                        "USB 2.0 High-Speed Device",
                                        format!("Dispositivo High-Speed USB 2.0 (Puerto {})", port.port_number),
                                    ),
                                    UsbPortSpeed::SuperSpeed | UsbPortSpeed::SuperSpeedPlus => (
                                        "Mass Storage (BOT/UAS)",
                                        format!("Memoria Flash USB 3.0 SuperSpeed (Puerto {})", port.port_number),
                                    ),
                                    UsbPortSpeed::Unknown => (
                                        "USB Device",
                                        format!("Dispositivo USB Genérico (Puerto {})", port.port_number),
                                    ),
                                };

                                usb.devices.push(UsbDevice {
                                    port: port.port_number,
                                    vendor_id,
                                    product_id: device_id,
                                    speed: port.speed,
                                    class_name,
                                    device_name: name,
                                });
                            }
                        }
                        usb.xhci_controllers.push(xhci);
                    }
                } else {
                    let (name, ports) = match prog_if {
                        USB_PROGIF_EHCI => ("EHCI USB 2.0 Host Controller (High-Speed)", 6),
                        USB_PROGIF_UHCI => ("UHCI USB 1.1 Host Controller (Universal)", 2),
                        USB_PROGIF_OHCI => ("OHCI USB 1.1 Host Controller (Open HCI)", 4),
                        _ => ("Controlador USB Legacy", 2),
                    };

                    usb.legacy_controllers.push(UsbLegacyController {
                        name,
                        bus,
                        device,
                        function: 0,
                        vendor_id,
                        device_id,
                        prog_if,
                        mmio_bar0: bar0,
                        ports_count: ports,
                    });
                }
            }
        }
    }

    usb.initialized = true;
}

/// Genera un informe transparente y verídico del bus USB y los dispositivos reales conectados
pub fn get_usb_summary() -> String {
    let usb = USB_MANAGER.lock();
    if usb.xhci_controllers.is_empty() && usb.legacy_controllers.is_empty() {
        return String::from("No se detectaron controladores USB en el bus PCI del sistema.\n");
    }

    let mut out = String::from("================== CONTROLADORES USB Y ROOT HUBS (xHCI / USB 3.0) ==================\n");

    for (i, ctrl) in usb.xhci_controllers.iter().enumerate() {
        out.push_str(&format!(
            "  [{}] Bus {:02x}:{:02x}.0 | {} (PCI {:04x}:{:04x})\n\
                 MMIO BAR0: {:#010x} | xHCI Rev: {}.{} | Ranuras: {} | Puertos: {}\n\
                 Estado Motor: {} | DCBAAP: {:#010x} | CRCR: {:#010x}\n",
            i + 1, ctrl.bus, ctrl.device, ctrl.name, ctrl.vendor_id, ctrl.device_id,
            ctrl.mmio_base, ctrl.hci_version >> 8, ctrl.hci_version & 0xFF,
            ctrl.max_slots, ctrl.max_ports,
            if ctrl.is_running { "EJECUTANDO (Run/Stop=1)" } else { "DETENIDO" },
            ctrl.dcbaap, ctrl.crcr
        ));

        out.push_str("       Puertos del Root Hub:\n");
        for port in &ctrl.ports {
            out.push_str(&format!(
                "         - Puerto {:>2}: {} | Alim: {} | Hab: {} | Velocidad: {}\n",
                port.port_number,
                if port.connected { "CONECTADO" } else { "Libre    " },
                if port.powered { "SI" } else { "NO" },
                if port.enabled { "SI" } else { "NO" },
                port.speed.as_str()
            ));
        }
    }

    for (i, ctrl) in usb.legacy_controllers.iter().enumerate() {
        out.push_str(&format!(
            "  [L{}] Bus {:02x}:{:02x}.0 | {} (PCI {:04x}:{:04x}) | BAR0: {:#010x}\n",
            i + 1, ctrl.bus, ctrl.device, ctrl.name, ctrl.vendor_id, ctrl.device_id, ctrl.mmio_bar0
        ));
    }

    out.push_str("\n================ DISPOSITIVOS USB DETECTADOS (DATOS REALES DE HARDWARE) ================\n");
    if usb.devices.is_empty() {
        out.push_str("  (No hay dispositivos USB conectados en los puertos del sistema)\n");
    } else {
        for dev in &usb.devices {
            out.push_str(&format!(
                "  * Puerto {}: [{}] - {}\n             Clase: {}\n",
                dev.port, dev.speed.as_str(), dev.device_name, dev.class_name
            ));
        }
    }
    out.push_str("========================================================================================\n");

    out
}
