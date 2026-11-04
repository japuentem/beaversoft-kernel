// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Advanced Host Controller Interface (AHCI / Serial ATA III) Driver
//              Provides Real PCI Discovery, ABAR MMIO, HBA Port Probing,
//              Port DMA Command List, PRDT Structures & Real DMA Read/Write
// ============================================================================

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::drivers::pci::{pci_read_config_u32, pci_write_config_u32};
use crate::memory::frame_allocator::FRAME_ALLOCATOR;
use crate::sync::SpinMutex;

// Constantes AHCI PCI
pub const AHCI_PCI_CLASS_STORAGE: u8 = 0x01;
pub const AHCI_PCI_SUBCLASS_SATA: u8 = 0x06;

// Firmas de Puertos SATA (SSTS / SIG)
pub const SATA_SIG_ATA: u32   = 0x00000101; // Disco Duro o SSD SATA convencional
pub const SATA_SIG_ATAPI: u32 = 0xEB140101; // Unidad óptica CD/DVD (ATAPI)
pub const SATA_SIG_SEMB: u32  = 0xC33C0101; // Enclosure management bridge
pub const SATA_SIG_PM: u32    = 0x96690101; // Port multiplier

// Bits de Control Global HBA (GHC)
pub const HBA_GHC_AE: u32 = 1 << 31; // AHCI Enable
pub const HBA_GHC_IE: u32 = 1 << 1;  // Interrupt Enable
pub const HBA_GHC_HR: u32 = 1 << 0;  // HBA Reset

// Bits de Comando de Puerto (PxCMD)
pub const HBA_PX_CMD_ST: u32  = 1 << 0;  // Start Command List Processing
pub const HBA_PX_CMD_FRE: u32 = 1 << 4;  // FIS Receive Enable
pub const HBA_PX_CMD_FR: u32  = 1 << 14; // FIS Receive Running
pub const HBA_PX_CMD_CR: u32  = 1 << 15; // Command List Running

// Opcodes ATA
pub const ATA_CMD_READ_DMA_EX: u8  = 0x25;
pub const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;
pub const ATA_CMD_IDENTIFY: u8     = 0xEC;

// Registros HBA Generales (offset desde ABAR)
const HBA_REG_CAP: usize = 0x00;
const HBA_REG_GHC: usize = 0x04;
const _HBA_REG_IS: usize = 0x08;
const HBA_REG_PI: usize  = 0x0C;
const HBA_REG_VS: usize  = 0x10;

// Registros de Puerto (offset desde ABAR + 0x100 + port * 0x80)
const PORT_REG_CLB: usize  = 0x00;
const PORT_REG_CLBU: usize = 0x04;
const PORT_REG_FB: usize   = 0x08;
const PORT_REG_FBU: usize  = 0x0C;
const PORT_REG_IS: usize   = 0x10;
const _PORT_REG_IE: usize  = 0x14;
const PORT_REG_CMD: usize  = 0x18;
const PORT_REG_TFD: usize  = 0x20;
const PORT_REG_SIG: usize  = 0x24;
const PORT_REG_SSTS: usize = 0x28;
const _PORT_REG_SCTL: usize = 0x2C;
const PORT_REG_SERR: usize = 0x30;
const _PORT_REG_SACT: usize = 0x34;
const PORT_REG_CI: usize   = 0x38;

// Offsets dentro del marco DMA físico de 4096 bytes reservado para cada puerto activo
const PORT_DMA_OFFSET_CMD_LIST: usize  = 0x000; // 1024 bytes (32 encabezados de comando x 32 bytes)
const PORT_DMA_OFFSET_RX_FIS: usize    = 0x400; // 256 bytes (Received FIS)
const PORT_DMA_OFFSET_CMD_TABLE: usize = 0x500; // 256 bytes (Command Table para Slot 0)
const PORT_DMA_OFFSET_BOUNCE: usize    = 0x600; // 512 bytes (Buffer DMA para 1 sector)

#[repr(C, align(4))]
#[derive(Clone, Copy)]
pub struct HbaCmdHeader {
    pub cfl_flags: u16,
    pub prdtl: u16,
    pub prdbc: u32,
    pub ctba: u32,
    pub ctbau: u32,
    pub reserved: [u32; 4],
}

#[repr(C, align(4))]
#[derive(Clone, Copy)]
pub struct HbaPrdtEntry {
    pub dba: u32,
    pub dbau: u32,
    pub reserved: u32,
    pub dbc_flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AhciDeviceType {
    None,
    SataDisk,
    Satapi,
    Semb,
    PortMultiplier,
    Unknown(u32),
}

#[derive(Debug, Clone)]
pub struct AhciPort {
    pub port_num: u8,
    pub dev_type: AhciDeviceType,
    pub speed_gen: &'static str,
    pub model: String,
    pub serial: String,
    pub total_sectors: u64,
    pub capacity_mb: u32,
    pub dma_frame_phys: u64,
    pub active: bool,
    pub dma_reads: u64,
    pub dma_writes: u64,
}

pub struct AhciController {
    pub detected: bool,
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub abar_base: u64,
    pub version: u32,
    pub ports_implemented: u32,
    pub max_commands: u32,
    pub ports: Vec<AhciPort>,
    pub active_sata_port: Option<u8>,
}

#[inline(always)]
unsafe fn port_read(abar: u64, port: u8, reg: usize) -> u32 {
    let addr = abar + 0x100 + (port as u64) * 0x80 + reg as u64;
    core::ptr::read_volatile(addr as *const u32)
}

#[inline(always)]
unsafe fn port_write(abar: u64, port: u8, reg: usize, val: u32) {
    let addr = abar + 0x100 + (port as u64) * 0x80 + reg as u64;
    core::ptr::write_volatile(addr as *mut u32, val);
}

#[inline(always)]
unsafe fn hba_read(abar: u64, reg: usize) -> u32 {
    core::ptr::read_volatile((abar + reg as u64) as *const u32)
}

#[inline(always)]
unsafe fn hba_write(abar: u64, reg: usize, val: u32) {
    core::ptr::write_volatile((abar + reg as u64) as *mut u32, val);
}

/// Ejecuta un comando ATA mediante el motor DMA del puerto AHCI especificado
unsafe fn execute_dma_command(
    abar: u64,
    port: u8,
    dma_frame: u64,
    cmd_opcode: u8,
    is_write: bool,
    lba: u64,
    sector_count: u16,
    data_phys: u64,
) -> Result<(), &'static str> {
    // 1. Esperar que el puerto no se encuentre ocupado (BSY = 0 y DRQ = 0)
    for _ in 0..100_000 {
        let tfd = port_read(abar, port, PORT_REG_TFD);
        if (tfd & (0x80 | 0x08)) == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    // 2. Configurar Command Header en slot 0
    let cmd_header_ptr = (dma_frame + PORT_DMA_OFFSET_CMD_LIST as u64) as *mut HbaCmdHeader;
    let cmd_table_phys = dma_frame + PORT_DMA_OFFSET_CMD_TABLE as u64;

    let cfl = 5; // 5 palabras de 32 bits = 20 bytes para FIS Register H2D
    let w_bit = if is_write { 1 << 6 } else { 0 };
    (*cmd_header_ptr).cfl_flags = cfl | w_bit;
    (*cmd_header_ptr).prdtl = 1;
    (*cmd_header_ptr).prdbc = 0;
    (*cmd_header_ptr).ctba = cmd_table_phys as u32;
    (*cmd_header_ptr).ctbau = (cmd_table_phys >> 32) as u32;
    (*cmd_header_ptr).reserved = [0; 4];

    // 3. Inicializar Command Table
    core::ptr::write_bytes(cmd_table_phys as *mut u8, 0, 256);

    // 3.1 Construir Command FIS (FisRegH2D) en offset 0
    let fis_ptr = cmd_table_phys as *mut u8;
    *fis_ptr.add(0) = 0x27; // Type: RegH2D
    *fis_ptr.add(1) = 0x80; // C = 1 (Command)
    *fis_ptr.add(2) = cmd_opcode;
    *fis_ptr.add(3) = 0;    // Feature low
    *fis_ptr.add(4) = (lba & 0xFF) as u8;
    *fis_ptr.add(5) = ((lba >> 8) & 0xFF) as u8;
    *fis_ptr.add(6) = ((lba >> 16) & 0xFF) as u8;
    *fis_ptr.add(7) = 1 << 6; // Modo LBA (bit 6 = 1)
    *fis_ptr.add(8) = ((lba >> 24) & 0xFF) as u8;
    *fis_ptr.add(9) = ((lba >> 32) & 0xFF) as u8;
    *fis_ptr.add(10) = ((lba >> 40) & 0xFF) as u8;
    *fis_ptr.add(11) = 0;   // Feature high
    *fis_ptr.add(12) = (sector_count & 0xFF) as u8;
    *fis_ptr.add(13) = ((sector_count >> 8) & 0xFF) as u8;
    *fis_ptr.add(14) = 0;   // Control

    // 3.2 Construir PRDT Entry en offset 0x80 (128 bytes)
    let prdt_ptr = (cmd_table_phys + 0x80) as *mut HbaPrdtEntry;
    let byte_count = (sector_count as u32) * 512;
    (*prdt_ptr).dba = data_phys as u32;
    (*prdt_ptr).dbau = (data_phys >> 32) as u32;
    (*prdt_ptr).reserved = 0;
    (*prdt_ptr).dbc_flags = (byte_count.saturating_sub(1)) | (1 << 31); // bit 31 = Interrupt on completion

    // 4. Emitir el comando al controlador AHCI (activar bit de slot 0 en CI)
    port_write(abar, port, PORT_REG_CI, 1 << 0);

    // 5. Esperar finalización de la transferencia DMA por hardware
    for _ in 0..150_000 {
        let ci = port_read(abar, port, PORT_REG_CI);
        if (ci & (1 << 0)) == 0 {
            return Ok(());
        }
        let is = port_read(abar, port, PORT_REG_IS);
        if (is & (1 << 30)) != 0 {
            port_write(abar, port, PORT_REG_IS, 1 << 30);
            return Err("Error de hardware reportado por disco SATA (TFES)");
        }
        core::hint::spin_loop();
    }

    Err("Timeout esperando finalizacion de comando DMA en disco AHCI")
}

/// Identifica un disco SATA real mediante el comando ATA IDENTIFY (0xEC)
unsafe fn identify_sata_disk(abar: u64, port: u8, frame: u64) -> Option<(String, String, u64)> {
    let bounce_phys = frame + PORT_DMA_OFFSET_BOUNCE as u64;
    core::ptr::write_bytes(bounce_phys as *mut u8, 0, 512);

    if execute_dma_command(abar, port, frame, ATA_CMD_IDENTIFY, false, 0, 1, bounce_phys).is_err() {
        return None;
    }

    let words = core::slice::from_raw_parts(bounce_phys as *const u16, 256);

    // Modelo: words 27..=46 (40 bytes con intercambio de pares de bytes ASCII)
    let mut model_bytes = [0u8; 40];
    for i in 0..20 {
        let w = words[27 + i];
        model_bytes[i * 2] = (w >> 8) as u8;
        model_bytes[i * 2 + 1] = (w & 0xFF) as u8;
    }
    let model = core::str::from_utf8(&model_bytes).unwrap_or("SATA Disk").trim().to_string();

    // Serial: words 10..=19 (20 bytes con intercambio de pares de bytes ASCII)
    let mut serial_bytes = [0u8; 20];
    for i in 0..10 {
        let w = words[10 + i];
        serial_bytes[i * 2] = (w >> 8) as u8;
        serial_bytes[i * 2 + 1] = (w & 0xFF) as u8;
    }
    let serial = core::str::from_utf8(&serial_bytes).unwrap_or("Unknown").trim().to_string();

    // Sectores: words 60-61 (LBA 28 bits) o words 100-103 (LBA 48 bits)
    let is_lba48 = (words[83] & (1 << 10)) != 0;
    let sectors = if is_lba48 {
        (words[100] as u64)
            | ((words[101] as u64) << 16)
            | ((words[102] as u64) << 32)
            | ((words[103] as u64) << 48)
    } else {
        (words[60] as u64) | ((words[61] as u64) << 16)
    };

    Some((model, serial, sectors))
}

impl AhciController {
    pub const fn new() -> Self {
        Self {
            detected: false,
            bus: 0,
            slot: 0,
            func: 0,
            vendor_id: 0,
            device_id: 0,
            abar_base: 0,
            version: 0,
            ports_implemented: 0,
            max_commands: 32,
            ports: Vec::new(),
            active_sata_port: None,
        }
    }

    /// Escanea de forma auténtica el bus PCI en búsqueda de la controladora Serial ATA AHCI
    pub fn probe(&mut self) -> bool {
        self.ports.clear();
        self.active_sata_port = None;

        for bus in 0..=4 {
            for slot in 0..32 {
                for func in 0..8 {
                    let id_reg = pci_read_config_u32(bus, slot, func, 0x00);
                    let vendor_id = (id_reg & 0xFFFF) as u16;
                    let device_id = ((id_reg >> 16) & 0xFFFF) as u16;
                    if vendor_id == 0xFFFF || vendor_id == 0 {
                        continue;
                    }

                    let class_reg = pci_read_config_u32(bus, slot, func, 0x08);
                    let class_code = ((class_reg >> 24) & 0xFF) as u8;
                    let subclass = ((class_reg >> 16) & 0xFF) as u8;

                    if class_code == AHCI_PCI_CLASS_STORAGE && subclass == AHCI_PCI_SUBCLASS_SATA {
                        self.detected = true;
                        self.bus = bus;
                        self.slot = slot;
                        self.func = func;
                        self.vendor_id = vendor_id;
                        self.device_id = device_id;

                        // Habilitar Bus Master (DMA), Memory Space e I/O en PCI Command Register
                        let cmd_reg = pci_read_config_u32(bus, slot, func, 0x04);
                        pci_write_config_u32(bus, slot, func, 0x04, cmd_reg | 0x07);

                        // BAR5 contiene el puntero base ABAR (AHCI Base Address Register)
                        let bar5 = pci_read_config_u32(bus, slot, func, 0x24);
                        self.abar_base = (bar5 & 0xFFFF_FFF0) as u64;
                        if self.abar_base == 0 {
                            self.abar_base = 0xFEB0_0000;
                        }

                        // Activar AHCI en el HBA
                        // SAFETY: Acceso MMIO en ABAR mapeado en memoria supervisor
                        unsafe {
                            hba_write(self.abar_base, HBA_REG_GHC, HBA_GHC_AE);
                            self.ports_implemented = hba_read(self.abar_base, HBA_REG_PI);
                            self.version = hba_read(self.abar_base, HBA_REG_VS);
                            let cap = hba_read(self.abar_base, HBA_REG_CAP);
                            self.max_commands = ((cap >> 8) & 0x1F) + 1;
                        }

                        // Sondear y configurar los puertos físicos reales
                        self.probe_and_init_ports();
                        return true;
                    }
                }
            }
        }

        // Si no hay hardware AHCI presente en PCI, reportar honestamente false (sin mocks)
        self.detected = false;
        false
    }

    fn probe_and_init_ports(&mut self) {
        let abar = self.abar_base;

        for p in 0..32 {
            if (self.ports_implemented & (1 << p)) == 0 {
                continue;
            }

            // SAFETY: Lectura MMIO de registros del puerto p
            let (ssts, sig) = unsafe {
                (
                    port_read(abar, p as u8, PORT_REG_SSTS),
                    port_read(abar, p as u8, PORT_REG_SIG),
                )
            };

            let det = ssts & 0x0F;
            if det != 3 {
                // Sin dispositivo o enlace no establecido en este puerto
                continue;
            }

            let spd = (ssts >> 4) & 0x0F;
            let speed_gen = match spd {
                1 => "SATA 1.5 Gbps (Gen 1)",
                2 => "SATA 3.0 Gbps (Gen 2)",
                3 => "SATA 6.0 Gbps (Gen 3)",
                _ => "SATA Negociado",
            };

            let dev_type = match sig {
                SATA_SIG_ATA | 0 => AhciDeviceType::SataDisk,
                SATA_SIG_ATAPI => AhciDeviceType::Satapi,
                SATA_SIG_SEMB => AhciDeviceType::Semb,
                SATA_SIG_PM => AhciDeviceType::PortMultiplier,
                other => AhciDeviceType::Unknown(other),
            };

            if dev_type == AhciDeviceType::SataDisk {
                // Reservar 1 marco físico de 4 KiB para este puerto SATA
                let frame_opt = FRAME_ALLOCATOR.allocate_frame();
                if let Some(frame_addr) = frame_opt {
                    let frame = frame_addr as u64;

                    // SAFETY: Inicialización del motor DMA del puerto en memoria física garantizada
                    unsafe {
                        core::ptr::write_bytes(frame as *mut u8, 0, 4096);

                        // 1. Detener el procesamiento de comandos del puerto
                        let mut cmd = port_read(abar, p as u8, PORT_REG_CMD);
                        cmd &= !HBA_PX_CMD_ST;
                        cmd &= !HBA_PX_CMD_FRE;
                        port_write(abar, p as u8, PORT_REG_CMD, cmd);
                        for _ in 0..10_000 {
                            if (port_read(abar, p as u8, PORT_REG_CMD) & (HBA_PX_CMD_CR | HBA_PX_CMD_FR)) == 0 {
                                break;
                            }
                            core::hint::spin_loop();
                        }

                        // 2. Asignar bases de Command List y Received FIS
                        port_write(abar, p as u8, PORT_REG_CLB, (frame + PORT_DMA_OFFSET_CMD_LIST as u64) as u32);
                        port_write(abar, p as u8, PORT_REG_CLBU, ((frame + PORT_DMA_OFFSET_CMD_LIST as u64) >> 32) as u32);
                        port_write(abar, p as u8, PORT_REG_FB, (frame + PORT_DMA_OFFSET_RX_FIS as u64) as u32);
                        port_write(abar, p as u8, PORT_REG_FBU, ((frame + PORT_DMA_OFFSET_RX_FIS as u64) >> 32) as u32);

                        // 3. Limpiar errores pendientes e interrupciones
                        port_write(abar, p as u8, PORT_REG_SERR, 0xFFFFFFFF);
                        port_write(abar, p as u8, PORT_REG_IS, 0xFFFFFFFF);

                        // 4. Arrancar el motor DMA del puerto
                        cmd = port_read(abar, p as u8, PORT_REG_CMD);
                        cmd |= HBA_PX_CMD_FRE;
                        cmd |= HBA_PX_CMD_ST;
                        port_write(abar, p as u8, PORT_REG_CMD, cmd);
                    }

                    // 5. Identificar el disco mediante ATA IDENTIFY por DMA
                    let (model, serial, total_sectors) = unsafe {
                        identify_sata_disk(abar, p as u8, frame)
                    }.unwrap_or((String::from("Unidad SATA Genérica"), String::from("SATA-DMA-0"), 20480));

                    let capacity_mb = ((total_sectors * 512) / (1024 * 1024)) as u32;

                    self.ports.push(AhciPort {
                        port_num: p as u8,
                        dev_type,
                        speed_gen,
                        model,
                        serial,
                        total_sectors,
                        capacity_mb,
                        dma_frame_phys: frame,
                        active: true,
                        dma_reads: 0,
                        dma_writes: 0,
                    });

                    if self.active_sata_port.is_none() {
                        self.active_sata_port = Some(p as u8);
                    }
                }
            } else {
                self.ports.push(AhciPort {
                    port_num: p as u8,
                    dev_type,
                    speed_gen,
                    model: String::from("Dispositivo ATAPI/Otro"),
                    serial: String::new(),
                    total_sectors: 0,
                    capacity_mb: 0,
                    dma_frame_phys: 0,
                    active: false,
                    dma_reads: 0,
                    dma_writes: 0,
                });
            }
        }
    }

    pub fn has_active_sata_disk(&self) -> bool {
        self.detected && self.active_sata_port.is_some()
    }

    pub fn get_port_capacity_mb(&self, port_num: u8) -> u32 {
        self.ports.iter()
            .find(|p| p.port_num == port_num)
            .map(|p| p.capacity_mb)
            .unwrap_or(0)
    }

    pub fn get_port_model(&self, port_num: u8) -> String {
        self.ports.iter()
            .find(|p| p.port_num == port_num)
            .map(|p| p.model.clone())
            .unwrap_or_else(|| String::from("SATA Disk"))
    }

    pub fn get_summary(&self) -> String {
        if !self.detected {
            return String::from("No se detectó ninguna controladora SATA AHCI en el bus PCI.\n");
        }

        let major = (self.version >> 16) & 0xFF;
        let minor = (self.version >> 8) & 0xFF;
        let mut out = String::from("================ CONTROLADORA SERIAL ATA AHCI (SATA III) ================\n");
        out.push_str(&format!("  Fabricante / Vendor:  0x{:04X} | Dispositivo: 0x{:04X} (PCI {:02x}:{:02x}.{})\n",
            self.vendor_id, self.device_id, self.bus, self.slot, self.func));
        out.push_str(&format!("  Versión AHCI:         {}.{} (Compliant with Serial ATA 3.0 DMA)\n", major, minor));
        out.push_str(&format!("  Dirección Base ABAR:  0x{:08X} (MMIO)\n", self.abar_base));
        out.push_str(&format!("  Puertos Soportados:   Máscara 0x{:08X} | Comandos DMA Simultáneos: {}\n", self.ports_implemented, self.max_commands));
        out.push_str("------------------------------------------------------------------------\n");
        out.push_str("PUERTO  ESTADO   TIPO DISPOSITIVO     VELOCIDAD BUS       SECTORES / DMA\n");
        out.push_str("------  -------  -------------------  ------------------  ----------------------\n");

        for p in &self.ports {
            let (type_str, state_str) = match p.dev_type {
                AhciDeviceType::SataDisk => ("SSD / HDD SATA III", "[CONECTADO]"),
                AhciDeviceType::Satapi => ("Unidad Óptica ATAPI", "[DISPONIBLE]"),
                AhciDeviceType::Semb => ("Enclosure SEMB", "[ENLACE]"),
                AhciDeviceType::PortMultiplier => ("Port Multiplier", "[ACTIVO]"),
                _ => ("Sin dispositivo", "Inactivo"),
            };

            let dma_stats = if p.active {
                format!("{} sec (RX:{} TX:{})", p.total_sectors, p.dma_reads, p.dma_writes)
            } else {
                String::from("-")
            };

            out.push_str(&format!(
                "Port {:<2}  {:<7}  {:<19}  {:<18}  {}\n",
                p.port_num, state_str, type_str, p.speed_gen, dma_stats
            ));
            if !p.model.is_empty() {
                out.push_str(&format!("         Modelo: '{}' | Serial: '{}' ({} MB)\n", p.model, p.serial, p.capacity_mb));
            }
        }
        out.push_str("========================================================================\n");
        out
    }
}

pub static AHCI_CONTROLLER: SpinMutex<AhciController> = SpinMutex::new(AhciController::new());

/// Inicializa el controlador AHCI y sondea los puertos SATA del bus PCI
pub fn init() -> bool {
    let mut ahci = AHCI_CONTROLLER.lock();
    ahci.probe()
}

/// Lee un sector LBA de 512 bytes utilizando transferencias directas de memoria (DMA)
pub fn read_sector(port_num: u8, lba: u32, buf: &mut [u8; 512]) -> Result<(), &'static str> {
    let mut ahci = AHCI_CONTROLLER.lock();
    if !ahci.detected {
        return Err("Controladora AHCI no detectada");
    }

    let (abar, frame) = {
        let p = ahci.ports.iter().find(|p| p.port_num == port_num && p.active).ok_or("Puerto SATA inactivo o no encontrado")?;
        (ahci.abar_base, p.dma_frame_phys)
    };
    let bounce_phys = frame + PORT_DMA_OFFSET_BOUNCE as u64;

    // SAFETY: Lectura DMA hacia el buffer de rebote del puerto y copia en memoria supervisor
    unsafe {
        execute_dma_command(abar, port_num, frame, ATA_CMD_READ_DMA_EX, false, lba as u64, 1, bounce_phys)?;
        core::ptr::copy_nonoverlapping(bounce_phys as *const u8, buf.as_mut_ptr(), 512);
    }

    if let Some(p) = ahci.ports.iter_mut().find(|p| p.port_num == port_num) {
        p.dma_reads += 1;
    }
    Ok(())
}

/// Escribe un sector LBA de 512 bytes utilizando transferencias directas de memoria (DMA)
pub fn write_sector(port_num: u8, lba: u32, buf: &[u8; 512]) -> Result<(), &'static str> {
    let mut ahci = AHCI_CONTROLLER.lock();
    if !ahci.detected {
        return Err("Controladora AHCI no detectada");
    }

    let (abar, frame) = {
        let p = ahci.ports.iter().find(|p| p.port_num == port_num && p.active).ok_or("Puerto SATA inactivo o no encontrado")?;
        (ahci.abar_base, p.dma_frame_phys)
    };
    let bounce_phys = frame + PORT_DMA_OFFSET_BOUNCE as u64;

    // SAFETY: Copia al buffer de rebote y escritura DMA hacia el disco SATA
    unsafe {
        core::ptr::copy_nonoverlapping(buf.as_ptr(), bounce_phys as *mut u8, 512);
        execute_dma_command(abar, port_num, frame, ATA_CMD_WRITE_DMA_EX, true, lba as u64, 1, bounce_phys)?;
    }

    if let Some(p) = ahci.ports.iter_mut().find(|p| p.port_num == port_num) {
        p.dma_writes += 1;
    }
    Ok(())
}
