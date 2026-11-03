// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Anti-Brick A/B Slot Manager & Sector-Zero Self-Healing Mirroring
// ============================================================================

use crate::storage::block_device::{BLOCK_DEVICE, SECTOR_SIZE};
use crate::sync::SpinMutex;
use crate::vga::VgaColor;
use crate::{kprintln, kprint_color};

pub const PRIMARY_BOOT_LBA: u32 = 0;
pub const BACKUP_BOOT_LBA: u32  = 16; // Sector de respaldo LBA 16

const MAGIC: [u8; 8] = *b"BEAVR_OS";
const BOOT_SIGNATURE_55AA: [u8; 2] = [0x55, 0xAA];

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct SlotMetadata {
    pub start_lba: u32,
    pub sector_count: u32,
    pub version_major: u16,
    pub version_minor: u16,
    pub slot_crc32: u32,
    pub boot_attempts_left: u8,
    pub boot_successful: u8,
    pub is_valid: u8,
    pub reserved: u8,
}

impl SlotMetadata {
    pub const fn new(start_lba: u32, count: u32, major: u16, minor: u16) -> Self {
        Self {
            start_lba,
            sector_count: count,
            version_major: major,
            version_minor: minor,
            slot_crc32: 0x12345678,
            boot_attempts_left: 3,
            boot_successful: 1,
            is_valid: 1,
            reserved: 0,
        }
    }
}

// Tamaño total = 8 + 4 + 4 + 1 + 8 + 20 + 20 + 4 + 441 + 2 = 512 bytes exactos (1 sector)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct BeaverBootRecord {
    pub magic: [u8; 8],
    pub version: u32,
    pub header_crc32: u32,
    pub active_slot: u8, // 0 = Slot A, 1 = Slot B
    pub boot_generation: u64,
    pub slot_a: SlotMetadata,
    pub slot_b: SlotMetadata,
    pub backup_lba: u32,
    pub reserved: [u8; 441],
    pub boot_signature: [u8; 2], // 0x55, 0xAA (Offset 510..512)
}

impl BeaverBootRecord {
    pub const fn default_config() -> Self {
        Self {
            magic: MAGIC,
            version: 1,
            header_crc32: 0,
            active_slot: 0, // Slot A por defecto
            boot_generation: 1,
            slot_a: SlotMetadata::new(32, 64, 0, 4), // Slot A en LBA 32
            slot_b: SlotMetadata::new(100, 64, 0, 1), // Slot B en LBA 100 (Golden Image)
            backup_lba: BACKUP_BOOT_LBA,
            reserved: [0; 441],
            boot_signature: BOOT_SIGNATURE_55AA,
        }
    }

    pub fn to_bytes(&self) -> [u8; SECTOR_SIZE] {
        let mut buf = [0_u8; SECTOR_SIZE];
        unsafe {
            core::ptr::copy_nonoverlapping(
                self as *const Self as *const u8,
                buf.as_mut_ptr(),
                core::mem::size_of::<Self>(),
            );
        }
        buf
    }

    pub fn from_bytes(buf: &[u8; SECTOR_SIZE]) -> Self {
        unsafe {
            core::ptr::read_unaligned(buf.as_ptr() as *const Self)
        }
    }

    pub fn compute_crc(&self) -> u32 {
        let raw = self as *const Self as *const u8;
        // Calcular CRC excluyendo el campo header_crc32 (offset 12..16)
        let mut sum: u32 = 0xFFFF_FFFF;
        unsafe {
            for i in 0..SECTOR_SIZE {
                if i >= 12 && i < 16 {
                    continue; // Saltar el campo de checksum
                }
                let byte = *raw.add(i);
                sum ^= byte as u32;
                for _ in 0..8 {
                    if (sum & 1) != 0 {
                        sum = (sum >> 1) ^ 0xEDB88320;
                    } else {
                        sum >>= 1;
                    }
                }
            }
        }
        !sum
    }

    pub fn is_valid(&self) -> bool {
        if self.magic != MAGIC {
            return false;
        }
        if self.boot_signature != BOOT_SIGNATURE_55AA {
            return false;
        }
        self.compute_crc() == self.header_crc32
    }
}

pub struct BootSlotController {
    record: BeaverBootRecord,
    initialized: bool,
}

impl BootSlotController {
    pub const fn new() -> Self {
        Self {
            record: BeaverBootRecord::default_config(),
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        let mut sec0_buf = [0_u8; SECTOR_SIZE];
        let mut backup_buf = [0_u8; SECTOR_SIZE];

        let _ = BLOCK_DEVICE.lock().read_sector(PRIMARY_BOOT_LBA, &mut sec0_buf);
        let sec0 = BeaverBootRecord::from_bytes(&sec0_buf);

        let _ = BLOCK_DEVICE.lock().read_sector(BACKUP_BOOT_LBA, &mut backup_buf);
        let backup = BeaverBootRecord::from_bytes(&backup_buf);

        if sec0.is_valid() {
            // El sector cero está íntegro y validado por CRC32
            self.record = sec0;
            self.initialized = true;

            // Sincronizar copia de respaldo si fuera necesario
            if !backup.is_valid() {
                let _ = BLOCK_DEVICE.lock().write_sector(BACKUP_BOOT_LBA, &sec0_buf);
            }
        } else if backup.is_valid() {
            // ¡SELF-HEALING ACTIVO! El Sector 0 falló, pero el Backup nos rescata
            kprint_color!(
                VgaColor::LightRed, VgaColor::Black,
                "\n[CRITICAL WARNING] ¡Sector Cero (LBA 0) dañado o corrupto!\n"
            );
            kprint_color!(
                VgaColor::LightGreen, VgaColor::Black,
                "[SELF-HEALING] Restaurando Sector Cero desde Copia Espejo (LBA 16)... "
            );

            // Reconstruir sector 0 a partir de la copia secundaria
            let _ = BLOCK_DEVICE.lock().write_sector(PRIMARY_BOOT_LBA, &backup_buf);
            self.record = backup;
            self.initialized = true;
            kprint_color!(VgaColor::LightGreen, VgaColor::Black, "[RESTAURADO]\n");
        } else {
            // Primer arranque en disco nuevo: formatear topología A/B inicial
            let mut fresh = BeaverBootRecord::default_config();
            fresh.header_crc32 = fresh.compute_crc();
            let fresh_bytes = fresh.to_bytes();

            let mut dev = BLOCK_DEVICE.lock();
            let _ = dev.write_sector(PRIMARY_BOOT_LBA, &fresh_bytes);
            let _ = dev.write_sector(BACKUP_BOOT_LBA, &fresh_bytes);

            self.record = fresh;
            self.initialized = true;
        }

        // Lógica Anti-Brickeo (Verificación de salud de ranuras A/B)
        self.check_anti_brick_policy();
    }

    fn check_anti_brick_policy(&mut self) {
        let active_is_b = self.record.active_slot == 1;
        let active_slot = if active_is_b { &mut self.record.slot_b } else { &mut self.record.slot_a };

        // Si la ranura no se ha validado y se quedó sin intentos
        if active_slot.boot_successful == 0 && active_slot.boot_attempts_left == 0 {
            kprint_color!(
                VgaColor::LightRed, VgaColor::Black,
                "[ANTI-BRICK] ¡La ranura actual agotó sus intentos de arranque! Conmutando a ranura de rescate...\n"
            );
            active_slot.is_valid = 0; // Marcar como fallida
            self.record.active_slot = if active_is_b { 0 } else { 1 }; // Conmutar al slot alterno
            self.save_metadata();
        }
    }

    pub fn mark_boot_successful(&mut self) {
        if self.record.active_slot == 0 {
            self.record.slot_a.boot_successful = 1;
            self.record.slot_a.boot_attempts_left = 3;
            self.record.slot_a.is_valid = 1;
        } else {
            self.record.slot_b.boot_successful = 1;
            self.record.slot_b.boot_attempts_left = 3;
            self.record.slot_b.is_valid = 1;
        }
        self.save_metadata();
    }

    pub fn active_slot(&self) -> u8 {
        self.record.active_slot
    }

    pub fn slot_a_attempts(&self) -> u8 {
        self.record.slot_a.boot_attempts_left
    }

    pub fn slot_b_attempts(&self) -> u8 {
        self.record.slot_b.boot_attempts_left
    }

    pub fn switch_slot(&mut self) -> u8 {
        self.record.active_slot = if self.record.active_slot == 0 { 1 } else { 0 };
        self.record.boot_generation += 1;
        self.save_metadata();
        self.record.active_slot
    }

    pub fn simulate_corrupt_sector_zero(&mut self) {
        // Sobreescribir el Sector Cero con basura deliberada para simular daño físico
        let garbage = [0xFF_u8; SECTOR_SIZE];
        let mut dev = BLOCK_DEVICE.lock();
        let _ = dev.write_sector(PRIMARY_BOOT_LBA, &garbage);
        kprint_color!(
            VgaColor::Yellow, VgaColor::Black,
            "[SIMULATION] Sector Cero (LBA 0) sobrescrito con bytes corruptos (0xFF).\n\
             En un SO tradicional (MBR), el sistema ya NO arrancaría nunca más.\n"
        );
    }

    pub fn self_heal_from_backup(&mut self) -> bool {
        let mut backup_buf = [0_u8; SECTOR_SIZE];
        let _ = BLOCK_DEVICE.lock().read_sector(BACKUP_BOOT_LBA, &mut backup_buf);
        let backup = BeaverBootRecord::from_bytes(&backup_buf);

        if backup.is_valid() {
            let mut dev = BLOCK_DEVICE.lock();
            let _ = dev.write_sector(PRIMARY_BOOT_LBA, &backup_buf);
            self.record = backup;
            true
        } else {
            false
        }
    }

    pub fn save_metadata(&mut self) {
        self.record.header_crc32 = self.record.compute_crc();
        let bytes = self.record.to_bytes();
        let mut dev = BLOCK_DEVICE.lock();
        // Escribir simultáneamente a Sector Cero y al Sector Espejo de Respaldo
        let _ = dev.write_sector(PRIMARY_BOOT_LBA, &bytes);
        let _ = dev.write_sector(BACKUP_BOOT_LBA, &bytes);
    }

    pub fn print_status(&self) {
        let is_slot_b = self.record.active_slot == 1;
        let boot_gen = self.record.boot_generation;
        let crc = self.record.header_crc32;
        let a_start = self.record.slot_a.start_lba;
        let a_end = a_start + self.record.slot_a.sector_count;
        let a_maj = self.record.slot_a.version_major;
        let a_min = self.record.slot_a.version_minor;
        let a_attempts = self.record.slot_a.boot_attempts_left;
        let a_ok = self.record.slot_a.boot_successful == 1;

        let b_start = self.record.slot_b.start_lba;
        let b_end = b_start + self.record.slot_b.sector_count;
        let b_maj = self.record.slot_b.version_major;
        let b_min = self.record.slot_b.version_minor;
        let b_attempts = self.record.slot_b.boot_attempts_left;
        let b_ok = self.record.slot_b.boot_successful == 1;

        kprintln!("==================== ESTADO DE RANURAS A/B Y DISCO ====================");
        kprint_color!(
            VgaColor::Yellow, VgaColor::Black,
            "  Ranura Activa:   {}\n",
            if !is_slot_b { "SLOT A (Producción)" } else { "SLOT B (Golden Recovery)" }
        );
        kprintln!("  Generación Boot: #{}", boot_gen);
        kprintln!("  Sector Cero:     LBA {} (Integridad CRC32: {:#010x})", PRIMARY_BOOT_LBA, crc);
        kprintln!("  Sector Espejo:   LBA {} (Copia de Seguridad Inmutable)", BACKUP_BOOT_LBA);

        kprint_color!(VgaColor::LightCyan, VgaColor::Black, "\n[SLOT A - Production Kernel]\n");
        kprintln!("  Ubicación:       LBA {}..{}", a_start, a_end);
        kprintln!("  Versión Kernel:  v{}.{}", a_maj, a_min);
        kprintln!("  Intentos Rest.:  {}/3", a_attempts);
        kprintln!("  Estado:          {}", if a_ok { "Saludable [OK]" } else { "Pendiente [TEST]" });

        kprint_color!(VgaColor::LightCyan, VgaColor::Black, "\n[SLOT B - Golden Recovery Mirror]\n");
        kprintln!("  Ubicación:       LBA {}..{}", b_start, b_end);
        kprintln!("  Versión Kernel:  v{}.{}", b_maj, b_min);
        kprintln!("  Intentos Rest.:  {}/3", b_attempts);
        kprintln!("  Estado:          {}", if b_ok { "Saludable [OK]" } else { "Pendiente [TEST]" });
        kprintln!("=======================================================================");
    }
}

pub static BOOT_SLOT_MANAGER: SpinMutex<BootSlotController> = SpinMutex::new(BootSlotController::new());
