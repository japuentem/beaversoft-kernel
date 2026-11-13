// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: GUID Partition Table (GPT) & Protective MBR Storage Parser (UEFI Compliant)
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use crate::storage::block_device::BLOCK_DEVICE;
use crate::sync::SpinLock;

pub const GPT_SIGNATURE: u64 = 0x5452_4150_2049_4645; // ASCII 'EFI PART' en little-endian
pub const MBR_PROTECTIVE_TYPE: u8 = 0xEE;

// GUIDs estándar de la industria
pub const GUID_EFI_SYSTEM: [u8; 16] = [
    0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B,
];

pub const GUID_BEAVER_ROOT: [u8; 16] = [
    0x71, 0x0F, 0xA5, 0xBE, 0xEF, 0xBE, 0x00, 0x40, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];

pub const GUID_LINUX_DATA: [u8; 16] = [
    0xAF, 0x3D, 0xC6, 0x0F, 0x83, 0x84, 0x72, 0x47, 0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4,
];

pub const GUID_BASIC_DATA: [u8; 16] = [
    0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44, 0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7,
];

#[derive(Debug, Clone)]
pub struct GptPartition {
    pub index: usize,
    pub type_guid: [u8; 16],
    pub unique_guid: [u8; 16],
    pub start_lba: u64,
    pub end_lba: u64,
    pub size_sectors: u64,
    pub size_mb: u64,
    pub name: String,
    pub type_desc: &'static str,
}

pub struct GptManager {
    pub has_protective_mbr: bool,
    pub gpt_valid: bool,
    pub disk_guid: [u8; 16],
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub total_partitions: usize,
    pub partitions: Vec<GptPartition>,
}

impl GptManager {
    pub const fn new() -> Self {
        Self {
            has_protective_mbr: false,
            gpt_valid: false,
            disk_guid: [0; 16],
            current_lba: 0,
            backup_lba: 0,
            first_usable_lba: 0,
            last_usable_lba: 0,
            total_partitions: 0,
            partitions: Vec::new(),
        }
    }

    /// Lee y analiza la tabla de particiones GPT directamente desde los sectores físicos del disco
    pub fn scan(&mut self) -> bool {
        self.partitions.clear();
        self.gpt_valid = false;
        self.has_protective_mbr = false;

        // 1. Verificar Protective MBR en LBA 0
        let mut lba0_buf = [0_u8; 512];
        if BLOCK_DEVICE.lock().read_sector(0, &mut lba0_buf).is_ok() {
            if lba0_buf[510] == 0x55 && lba0_buf[511] == 0xAA {
                let part_type = lba0_buf[446 + 4];
                self.has_protective_mbr = part_type == MBR_PROTECTIVE_TYPE || part_type == 0x83;
            }
        }

        // 2. Leer Cabecera GPT en LBA 1
        let mut lba1_buf = [0_u8; 512];
        if BLOCK_DEVICE.lock().read_sector(1, &mut lba1_buf).is_err() {
            return false;
        }

        let sig = u64::from_le_bytes([
            lba1_buf[0], lba1_buf[1], lba1_buf[2], lba1_buf[3],
            lba1_buf[4], lba1_buf[5], lba1_buf[6], lba1_buf[7],
        ]);

        if sig != GPT_SIGNATURE {
            return false;
        }

        let header_size = u32::from_le_bytes(lba1_buf[12..16].try_into().unwrap_or([0; 4])) as usize;
        if header_size < 92 || header_size > 512 {
            return false;
        }

        // Validar CRC32 de la cabecera (excluyendo el campo CRC32 en offset 16..20)
        let stored_crc = u32::from_le_bytes(lba1_buf[16..20].try_into().unwrap_or([0; 4]));
        let mut crc_check_buf = lba1_buf;
        crc_check_buf[16..20].copy_from_slice(&[0; 4]);
        let computed_crc = crate::storage::crc32::crc32(&crc_check_buf[..header_size]);
        if stored_crc != computed_crc {
            return false;
        }

        self.current_lba = u64::from_le_bytes(lba1_buf[24..32].try_into().unwrap_or([0; 8]));
        self.backup_lba = u64::from_le_bytes(lba1_buf[32..40].try_into().unwrap_or([0; 8]));
        self.first_usable_lba = u64::from_le_bytes(lba1_buf[40..48].try_into().unwrap_or([0; 8]));
        self.last_usable_lba = u64::from_le_bytes(lba1_buf[48..56].try_into().unwrap_or([0; 8]));
        self.disk_guid.copy_from_slice(&lba1_buf[56..72]);

        let part_entry_lba = u64::from_le_bytes(lba1_buf[72..80].try_into().unwrap_or([0; 8]));
        let num_entries = u32::from_le_bytes(lba1_buf[80..84].try_into().unwrap_or([0; 4])) as usize;
        let entry_size = u32::from_le_bytes(lba1_buf[84..88].try_into().unwrap_or([0; 4])) as usize;

        if num_entries == 0 || entry_size < 128 || entry_size > 512 {
            return false;
        }

        // 3. Leer y parsear el array de entradas de partición desde LBA part_entry_lba..
        let entries_per_sector = 512 / entry_size;
        let total_entry_sectors = (num_entries + entries_per_sector - 1) / entries_per_sector;

        let mut entry_index = 0;
        let mut cur_sector_buf = [0u8; 512];

        for sec in 0..total_entry_sectors {
            let lba = (part_entry_lba + sec as u64) as u32;
            if BLOCK_DEVICE.lock().read_sector(lba, &mut cur_sector_buf).is_err() {
                break;
            }

            for e in 0..entries_per_sector {
                if entry_index >= num_entries {
                    break;
                }
                let offset = e * entry_size;
                let raw_entry = &cur_sector_buf[offset..offset + entry_size];

                let mut type_guid = [0u8; 16];
                type_guid.copy_from_slice(&raw_entry[0..16]);

                // Si type_guid son todos ceros, es una entrada vacía
                if type_guid != [0u8; 16] {
                    let start_lba = u64::from_le_bytes(raw_entry[32..40].try_into().unwrap_or([0; 8]));
                    let end_lba = u64::from_le_bytes(raw_entry[40..48].try_into().unwrap_or([0; 8]));

                    // Validación estricta según estándar UEFI: LBA debe ubicarse dentro del rango usable
                    if start_lba < self.first_usable_lba || end_lba > self.last_usable_lba || end_lba < start_lba {
                        entry_index += 1;
                        continue;
                    }

                    let mut unique_guid = [0u8; 16];
                    unique_guid.copy_from_slice(&raw_entry[16..32]);

                    let size_sectors = end_lba - start_lba + 1;
                    let size_mb = (size_sectors * 512) / (1024 * 1024);

                    // Decodificar nombre en UTF-16LE (offset 56..128, hasta 36 caracteres)
                    let mut name = String::new();
                    for i in 0..36 {
                        let code = u16::from_le_bytes([raw_entry[56 + i * 2], raw_entry[56 + i * 2 + 1]]);
                        if code == 0 {
                            break;
                        }
                        if code < 128 {
                            name.push(code as u8 as char);
                        }
                    }

                    let type_desc = match type_guid {
                        GUID_EFI_SYSTEM => "ESP Bootloader UEFI (FAT32)",
                        GUID_BEAVER_ROOT => "BeaverFS Meta-Kernel Root (ATA)",
                        GUID_LINUX_DATA => "Linux Filesystem Data",
                        GUID_BASIC_DATA => "Microsoft Basic Data Partition",
                        _ => "Particion GPT Generica",
                    };

                    self.partitions.push(GptPartition {
                        index: entry_index + 1,
                        type_guid,
                        unique_guid,
                        start_lba,
                        end_lba,
                        size_sectors,
                        size_mb,
                        name,
                        type_desc,
                    });
                }
                entry_index += 1;
            }
        }

        self.total_partitions = self.partitions.len();
        self.gpt_valid = true;
        true
    }

    /// Escribe una tabla de particiones GPT estándar y auténtica en el disco
    pub fn format_gpt(&mut self, total_sectors: u64) -> Result<(), &'static str> {
        if total_sectors < 2048 {
            return Err("Capacidad de disco insuficiente para tabla GPT (< 1 MB)");
        }

        // 1. Escribir Protective MBR en LBA 0 (preserva firma 0x55AA y define partición 0xEE)
        let mut mbr = [0u8; 512];
        let _ = BLOCK_DEVICE.lock().read_sector(0, &mut mbr);

        // MBR Partition Entry 1 en offset 446
        mbr[446] = 0x00; // Bootable: 0
        mbr[447] = 0x00; mbr[448] = 0x02; mbr[449] = 0x00; // Starting CHS
        mbr[450] = MBR_PROTECTIVE_TYPE; // 0xEE Protective MBR
        mbr[451] = 0xFF; mbr[452] = 0xFF; mbr[453] = 0xFF; // Ending CHS
        mbr[454..458].copy_from_slice(&1_u32.to_le_bytes()); // Starting LBA 1
        let disk_size_lba = ((total_sectors - 1).min(0xFFFF_FFFF)) as u32;
        mbr[458..462].copy_from_slice(&disk_size_lba.to_le_bytes()); // Size in LBA

        // Firma MBR 0x55AA
        mbr[510] = 0x55;
        mbr[511] = 0xAA;

        BLOCK_DEVICE.lock().write_sector(0, &mbr)?;

        // 2. Preparar el array de 128 entradas de partición (16384 bytes = 32 sectores en LBA 2..33)
        let mut part_array = [0u8; 16384];

        // Partición 1: ESP (LBA 34..255)
        let p1_offset = 0;
        part_array[p1_offset..p1_offset + 16].copy_from_slice(&GUID_EFI_SYSTEM);
        part_array[p1_offset + 16..p1_offset + 32].copy_from_slice(&[0x11; 16]); // Unique GUID
        part_array[p1_offset + 32..p1_offset + 40].copy_from_slice(&34_u64.to_le_bytes());
        part_array[p1_offset + 40..p1_offset + 48].copy_from_slice(&255_u64.to_le_bytes());
        let p1_name = "EFI System Partition (ESP)";
        for (i, b) in p1_name.bytes().take(35).enumerate() {
            part_array[p1_offset + 56 + i * 2] = b;
        }

        // Partición 2: BeaverOS Root (LBA 256..8448)
        let p2_offset = 128;
        part_array[p2_offset..p2_offset + 16].copy_from_slice(&GUID_BEAVER_ROOT);
        part_array[p2_offset + 16..p2_offset + 32].copy_from_slice(&[0x22; 16]);
        part_array[p2_offset + 32..p2_offset + 40].copy_from_slice(&256_u64.to_le_bytes());
        part_array[p2_offset + 40..p2_offset + 48].copy_from_slice(&8448_u64.to_le_bytes());
        let p2_name = "BeaverOS Root (/disk)";
        for (i, b) in p2_name.bytes().take(35).enumerate() {
            part_array[p2_offset + 56 + i * 2] = b;
        }

        // Partición 3: BeaverOS Dual Slot B Mirror (LBA 8449..16640)
        let p3_offset = 256;
        part_array[p3_offset..p3_offset + 16].copy_from_slice(&GUID_BEAVER_ROOT);
        part_array[p3_offset + 16..p3_offset + 32].copy_from_slice(&[0x33; 16]);
        part_array[p3_offset + 32..p3_offset + 40].copy_from_slice(&8449_u64.to_le_bytes());
        part_array[p3_offset + 40..p3_offset + 48].copy_from_slice(&16640_u64.to_le_bytes());
        let p3_name = "BeaverOS Slot-B Mirror";
        for (i, b) in p3_name.bytes().take(35).enumerate() {
            part_array[p3_offset + 56 + i * 2] = b;
        }

        // Escribir los 32 sectores del array de particiones a disco en LBA 2..33
        for s in 0..32 {
            let mut sec = [0u8; 512];
            sec.copy_from_slice(&part_array[s * 512..(s + 1) * 512]);
            BLOCK_DEVICE.lock().write_sector((2 + s) as u32, &sec)?;
        }

        // Calcular CRC32 del array de particiones
        let part_array_crc = crate::storage::crc32::crc32(&part_array);

        // 3. Preparar y escribir la Cabecera GPT Principal en LBA 1
        let mut hdr = [0u8; 512];
        hdr[0..8].copy_from_slice(&GPT_SIGNATURE.to_le_bytes()); // 'EFI PART'
        hdr[8..12].copy_from_slice(&0x0001_0000_u32.to_le_bytes()); // Revision 1.0
        hdr[12..16].copy_from_slice(&92_u32.to_le_bytes()); // Header size 92 bytes
        hdr[16..20].copy_from_slice(&0_u32.to_le_bytes());
        hdr[20..24].copy_from_slice(&0_u32.to_le_bytes());
        hdr[24..32].copy_from_slice(&1_u64.to_le_bytes()); // MyLBA: 1
        let backup_lba = total_sectors - 1;
        hdr[32..40].copy_from_slice(&backup_lba.to_le_bytes()); // AlternateLBA
        hdr[40..48].copy_from_slice(&34_u64.to_le_bytes()); // FirstUsableLBA: 34
        let last_usable = total_sectors.saturating_sub(34);
        hdr[48..56].copy_from_slice(&last_usable.to_le_bytes()); // LastUsableLBA
        let disk_guid = [
            0xBE, 0xAF, 0x00, 0x01, 0xCA, 0xFE, 0xBA, 0xBE,
            0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x01,
        ];
        hdr[56..72].copy_from_slice(&disk_guid); // Disk GUID
        hdr[72..80].copy_from_slice(&2_u64.to_le_bytes()); // PartitionEntryLBA: 2
        hdr[80..84].copy_from_slice(&128_u32.to_le_bytes()); // NumberOfPartitionEntries: 128
        hdr[84..88].copy_from_slice(&128_u32.to_le_bytes()); // SizeOfPartitionEntry: 128
        hdr[88..92].copy_from_slice(&part_array_crc.to_le_bytes()); // PartitionEntryArrayCRC32

        // Calcular CRC32 de la cabecera sobre los primeros 92 bytes
        let hdr_crc = crate::storage::crc32::crc32(&hdr[..92]);
        hdr[16..20].copy_from_slice(&hdr_crc.to_le_bytes());

        // Escribir LBA 1 a disco
        BLOCK_DEVICE.lock().write_sector(1, &hdr)?;

        // 4. Escribir Cabecera GPT de Respaldo en AlternateLBA (si cabe en el disco)
        if backup_lba < 0xFFFF_FFFF {
            let mut backup_hdr = hdr;
            backup_hdr[24..32].copy_from_slice(&backup_lba.to_le_bytes()); // MyLBA
            backup_hdr[32..40].copy_from_slice(&1_u64.to_le_bytes()); // AlternateLBA
            let backup_part_lba = total_sectors.saturating_sub(33);
            backup_hdr[72..80].copy_from_slice(&backup_part_lba.to_le_bytes());
            backup_hdr[16..20].copy_from_slice(&0_u32.to_le_bytes());
            let backup_crc = crate::storage::crc32::crc32(&backup_hdr[..92]);
            backup_hdr[16..20].copy_from_slice(&backup_crc.to_le_bytes());
            let _ = BLOCK_DEVICE.lock().write_sector(backup_lba as u32, &backup_hdr);
        }

        // 5. Escanear y verificar la tabla recién escrita directamente desde el disco
        if !self.scan() {
            return Err("Fallo de integridad tras escribir la tabla GPT en disco");
        }

        Ok(())
    }

    pub fn get_summary(&self) -> String {
        let mut out = String::from("================ TABLA DE PARTICIONES GPT (GUID PARTITION TABLE) ================\n");
        out.push_str(&format!("  Protective MBR (LBA 0):  {}\n", if self.has_protective_mbr { "[OK] Presente (Type 0xEE)" } else { "Ausente" }));
        out.push_str(&format!("  Cabecera GPT (LBA 1):    {}\n", if self.gpt_valid { "[OK] Válida ('EFI PART')" } else { "Inválida" }));
        out.push_str(&format!(
            "  Disk GUID:               {:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}\n",
            self.disk_guid[0], self.disk_guid[1], self.disk_guid[2], self.disk_guid[3],
            self.disk_guid[4], self.disk_guid[5], self.disk_guid[6], self.disk_guid[7],
            self.disk_guid[8], self.disk_guid[9], self.disk_guid[10], self.disk_guid[11],
            self.disk_guid[12], self.disk_guid[13], self.disk_guid[14], self.disk_guid[15],
        ));
        out.push_str(&format!("  Sectores Usables:        LBA {} a LBA {} (Total: {} MB)\n", self.first_usable_lba, self.last_usable_lba, (self.last_usable_lba - self.first_usable_lba) / 2048));
        out.push_str(&format!("  Copia de Respaldo LBA:   LBA {}\n", self.backup_lba));
        out.push_str("--------------------------------------------------------------------------------\n");
        out.push_str("NUM  NOMBRE PARTICION           TIPO / SISTEMA              INICIO LBA  FIN LBA   TAMANO\n");
        out.push_str("---  -------------------------  --------------------------  ----------  --------  ------\n");

        for p in &self.partitions {
            out.push_str(&format!(
                "{:<3}  {:<25}  {:<26}  {:<10}  {:<8}  {} MB\n",
                p.index, p.name, p.type_desc, p.start_lba, p.end_lba, p.size_mb
            ));
        }

        out.push_str("================================================================================\n");
        out
    }
}

pub static GPT_MANAGER: SpinLock<GptManager> = SpinLock::new(GptManager::new());

pub fn init() {
    let mut gpt = GPT_MANAGER.lock();
    if !gpt.scan() {
        let _ = gpt.format_gpt(20480);
    }
}

