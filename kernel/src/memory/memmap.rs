// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Physical Memory Map Parser - Multiboot 1 y UEFI
//              Lee el mapa de memoria entregado por el cargador y lo convierte
//              en un arreglo fijo de MemRegion (sin heap, se usa antes del allocator).
// ============================================================================

/// Numero maximo de regiones de memoria que podemos representar.
/// 64 regiones son mas que suficientes para cualquier configuracion de PC.
pub const MAX_MEM_REGIONS: usize = 64;

/// Magic numbers para detectar el tipo de arranque.
pub const BOOT_MAGIC_MB1:  u64 = 0x2BADB002;
pub const BOOT_MAGIC_MB2:  u64 = 0x36d76289;
pub const BOOT_MAGIC_UEFI: u64 = 0xBEA0B007;

/// Una region de memoria fisica.
#[derive(Copy, Clone)]
pub struct MemRegion {
    /// Direccion fisica de inicio (alineada a 4 KiB hacia arriba internamente).
    pub start: u64,
    /// Longitud en bytes.
    pub len: u64,
    /// true = RAM utilizable por el kernel/aplicaciones.
    pub usable: bool,
}

impl MemRegion {
    pub const fn zero() -> Self {
        Self { start: 0, len: 0, usable: false }
    }
}

/// Resultado del parseo: arreglo fijo + numero de entradas validas.
pub struct MemMap {
    pub regions: [MemRegion; MAX_MEM_REGIONS],
    pub count: usize,
}

impl MemMap {
    pub const fn empty() -> Self {
        Self {
            regions: [MemRegion::zero(); MAX_MEM_REGIONS],
            count: 0,
        }
    }

    fn push(&mut self, region: MemRegion) {
        if self.count < MAX_MEM_REGIONS {
            self.regions[self.count] = region;
            self.count += 1;
        }
    }

    /// Memoria total utilizable en bytes.
    pub fn total_usable_bytes(&self) -> u64 {
        let mut total = 0u64;
        for i in 0..self.count {
            if self.regions[i].usable {
                total = total.saturating_add(self.regions[i].len);
            }
        }
        total
    }
}

// ============================================================================
// Parser Multiboot 1
// ============================================================================
//
// La estructura de información Multiboot 1 empieza en la dirección que viene
// en ebx. Los primeros 4 bytes son flags que indican qué campos están presentes.
// El mapa de memoria está en offset 44 (mmap_length) y 48 (mmap_addr) cuando
// el bit 6 de flags está activo.
//
// Cada entrada del mapa tiene:
//   [0..3]  size (u32)  — tamaño del resto de la entrada (tipicamente 20)
//   [4..11] base_addr (u64)
//   [12..19] length (u64)
//   [20..23] type (u32)  — 1 = usable RAM, resto = reservado
//
// La siguiente entrada empieza en: ptr + 4 + size

/// Parsea el mapa de memoria de Multiboot 1.
/// # Safety
/// `info_ptr` debe ser la dirección física válida del struct MultibootInfo
/// tal como la entregó GRUB/QEMU en el registro ebx.
pub unsafe fn parse_multiboot1(info_ptr: u64) -> MemMap {
    let mut map = MemMap::empty();

    if info_ptr == 0 {
        return map;
    }

    let base = info_ptr as *const u32;
    let flags = *base; // u32 en offset 0

    // Bit 6 de flags indica que el mmap está presente
    if flags & (1 << 6) == 0 {
        return map;
    }

    // offset 44 = mmap_length (u32), offset 48 = mmap_addr (u32)
    let mmap_length = *(info_ptr.wrapping_add(44) as *const u32) as usize;
    let mmap_addr   = *(info_ptr.wrapping_add(48) as *const u32) as u64;

    let mut offset: usize = 0;
    while offset < mmap_length {
        let entry_base = (mmap_addr as usize + offset) as *const u8;
        // Los primeros 4 bytes son el 'size' (no incluye esos 4 bytes)
        let size = *(entry_base as *const u32) as usize;
        if size == 0 { break; }

        // base_addr está en offset 4, length en offset 12, type en offset 20
        let base_addr  = ((entry_base as usize + 4)  as *const u64).read_unaligned();
        let length     = ((entry_base as usize + 12) as *const u64).read_unaligned();
        let entry_type = ((entry_base as usize + 20) as *const u32).read_unaligned();

        map.push(MemRegion {
            start:  base_addr,
            len:    length,
            usable: entry_type == 1, // 1 = EFI_CONVENTIONAL_MEMORY / usable RAM
        });

        offset += 4 + size;
    }

    map
}

// ============================================================================
// Parser UEFI (EFI_MEMORY_DESCRIPTOR)
// ============================================================================
//
// Cada descriptor tiene (en la ABI UEFI standard):
//   offset 0:  type (u32)    — EFI memory type
//   offset 4:  _pad (u32)   — alineacion
//   offset 8:  physical_start (u64)
//   offset 16: virtual_start (u64)
//   offset 24: number_of_pages (u64)
//   offset 32: attribute (u64)
// El tamaño real del descriptor (desc_size) puede ser > 40 por extensiones del firmware.
//
// Tipos usables:
//   3  = EfiBootServicesCode        — liberado tras exit_boot_services → usable
//   4  = EfiBootServicesData        — liberado tras exit_boot_services → usable
//   7  = EfiConventionalMemory      → usable
//   14 = EfiPersistentMemory        → usable (NVDIMM)

/// Parsea el mapa de memoria entregado por el cargador UEFI.
/// # Safety
/// `map_addr` y `map_size`/`desc_size` deben venir del campo mem_map_* de UefiBootInfo.
pub unsafe fn parse_uefi(map_addr: u64, map_size: u64, desc_size: u64) -> MemMap {
    let mut map = MemMap::empty();

    if map_addr == 0 || map_size == 0 || desc_size == 0 {
        return map;
    }

    let num_descs = (map_size / desc_size) as usize;

    for i in 0..num_descs {
        let desc_base = (map_addr + i as u64 * desc_size) as *const u8;

        let mem_type   = (desc_base as *const u32).read_unaligned();             // offset 0
        let phys_start = ((desc_base as usize + 8) as *const u64).read_unaligned();  // offset 8
        let num_pages  = ((desc_base as usize + 24) as *const u64).read_unaligned(); // offset 24

        let usable = matches!(mem_type, 3 | 4 | 7 | 14);

        map.push(MemRegion {
            start:  phys_start,
            len:    num_pages * 4096,
            usable,
        });
    }

    map
}

/// Detecta el tipo de arranque por magic y parsea el mapa correspondiente.
/// Imprime cada region por serial (kprintln!).
pub fn parse_and_print(boot_magic: u64, boot_info: u64) -> MemMap {
    let map = unsafe {
        if boot_magic == BOOT_MAGIC_MB1 {
            parse_multiboot1(boot_info)
        } else if boot_magic == BOOT_MAGIC_UEFI {
            // boot_info apunta a UefiBootInfo en 0x9000 (repr(C))
            let info = (boot_info as *const crate::drivers::vbe::UefiBootInfo).read_unaligned();
            parse_uefi(info.mem_map_addr, info.mem_map_size, info.mem_desc_size)
        } else {
            // Arranque desconocido o Multiboot 2 (no implementado aun)
            MemMap::empty()
        }
    };

    // Imprimir mapa por serial
    crate::kprintln!("[MEM] Mapa de memoria fisico ({} regiones):", map.count);
    for i in 0..map.count {
        let r = &map.regions[i];
        let end = r.start.saturating_add(r.len).saturating_sub(1);
        let kind = if r.usable { "USABLE  " } else { "RESERVED" };
        crate::kprintln!(
            "[MEM]   {:#014x} - {:#014x}  {}  ({} KiB)",
            r.start, end, kind, r.len / 1024
        );
    }
    let total_mib = map.total_usable_bytes() / (1024 * 1024);
    crate::kprintln!("[MEM] RAM utilizable total: {} MiB", total_mib);

    map
}

