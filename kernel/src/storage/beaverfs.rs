// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: BeaverFS Persistent Disk File System (Superblock, Inodes, Bitmaps)
// ============================================================================

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use crate::storage::block_device::{BLOCK_DEVICE, SECTOR_SIZE};
use crate::sync::SpinMutex;

/// Número Mágico distintivo de BeaverFS: 0xBEEF50F7
pub const BEAVERFS_MAGIC: u32 = 0xBEEF50F7;

/// LBA donde inicia la partición del sistema de archivos BeaverFS
pub const BEAVERFS_START_LBA: u32 = 256;

/// Cantidad total de sectores dedicados a BeaverFS (ej. 8192 sectores = 4 MiB)
pub const BEAVERFS_TOTAL_SECTORS: u32 = 8192;

/// Tamaño de un bloque en BeaverFS (1 sector = 512 bytes)
pub const BLOCK_SIZE: usize = SECTOR_SIZE;

/// Cantidad máxima de inodos soportados (64 inodos = 64 archivos/directorios en disco)
pub const MAX_INODES: usize = 64;

/// Inodos por sector: 512 bytes / 64 bytes por inodo = 8 inodos por sector (8 sectores para 64 inodos)
pub const INODES_PER_SECTOR: usize = 8;
pub const INODE_SECTORS: u32 = (MAX_INODES / INODES_PER_SECTOR) as u32; // 8 sectores

/// Offsets relativos dentro de la partición:
/// Sector 0: Superbloque
/// Sector 1: Bitmap de bloques (512 bytes * 8 bits = 4096 bloques direccionables)
/// Sector 2..9: Tabla de inodos (8 sectores = 64 inodos)
/// Sector 10..: Bloques de datos (Data blocks)
pub const SUPERBLOCK_REL_LBA: u32 = 0;
pub const BLOCK_BITMAP_REL_LBA: u32 = 1;
pub const INODE_TABLE_REL_LBA: u32 = 2;
pub const DATA_BLOCKS_START_REL_LBA: u32 = 10;

/// Superbloque de BeaverFS almacenado en disco (Offset 0 dentro del sector)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct Superblock {
    pub magic: u32,
    pub block_size: u32,
    pub total_blocks: u32,
    pub free_blocks: u32,
    pub total_inodes: u32,
    pub free_inodes: u32,
    pub root_inode: u32,
    pub clean_unmount: u8,
    pub reserved: [u8; 483],
}

impl Superblock {
    pub const fn new(total_blocks: u32) -> Self {
        Self {
            magic: BEAVERFS_MAGIC,
            block_size: BLOCK_SIZE as u32,
            total_blocks,
            free_blocks: total_blocks.saturating_sub(DATA_BLOCKS_START_REL_LBA),
            total_inodes: MAX_INODES as u32,
            free_inodes: MAX_INODES as u32,
            root_inode: 0,
            clean_unmount: 1,
            reserved: [0; 483],
        }
    }
}

/// Tipo de inodo en BeaverFS
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum InodeType {
    Free = 0,
    RegularFile = 1,
    Directory = 2,
}

/// Inodo en disco (Exactamente 64 bytes para alinear 8 inodos por sector de 512 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct DiskInode {
    pub inode_type: u8,          // 0: libre, 1: archivo, 2: dir (1 byte)
    pub permissions: u16,        // Permisos octales 0o755, 0o644 (2 bytes)
    pub uid: u32,                // Propietario UID (4 bytes)
    pub gid: u32,                // Grupo GID (4 bytes)
    pub size_bytes: u32,         // Tamaño exacto en bytes (4 bytes)
    pub block_count: u16,        // Bloques asignados (2 bytes)
    pub direct_blocks: [u32; 7], // Punteros directos a bloques LBA (28 bytes = 3.5 KiB)
    pub indirect_block: u32,     // Puntero indirecto simple (4 bytes = 128 bloques = 64 KiB)
    pub double_indirect: u32,    // Puntero doblemente indirecto (4 bytes = 128x128 bloques = 8 MiB)
    pub created_time: u32,       // Ticks o timestamp (4 bytes)
    pub modified_time: u32,      // Ticks o timestamp (4 bytes)
    pub reserved: [u8; 3],       // Relleno a 64 bytes exactos (3 bytes)
}

impl DiskInode {
    pub const fn empty() -> Self {
        Self {
            inode_type: 0,
            permissions: 0,
            uid: 0,
            gid: 0,
            size_bytes: 0,
            block_count: 0,
            direct_blocks: [0; 7],
            indirect_block: 0,
            double_indirect: 0,
            created_time: 0,
            modified_time: 0,
            reserved: [0; 3],
        }
    }
}

/// Entrada de directorio en bloque de datos (64 bytes exactos: 8 entradas por bloque de 512 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct DirEntry {
    pub inode_index: u32,
    pub name: [u8; 56],
    pub entry_type: u8,
    pub reserved: [u8; 3],
}

impl DirEntry {
    pub const fn empty() -> Self {
        Self {
            inode_index: 0xFFFF_FFFF,
            name: [0; 56],
            entry_type: 0,
            reserved: [0; 3],
        }
    }

    pub fn new(inode: u32, name_str: &str, entry_type: u8) -> Self {
        let mut entry = Self::empty();
        entry.inode_index = inode;
        entry.entry_type = entry_type;
        let bytes = name_str.as_bytes();
        let len = bytes.len().min(55);
        for i in 0..len {
            entry.name[i] = bytes[i];
        }
        entry
    }

    pub fn is_free(&self) -> bool {
        self.inode_index == 0xFFFF_FFFF || self.name[0] == 0
    }

    pub fn is_valid(&self) -> bool {
        self.inode_index != 0xFFFF_FFFF && self.name[0] != 0
    }

    pub fn name_as_str(&self) -> &str {
        let mut len = 0;
        while len < 56 && self.name[len] != 0 {
            len += 1;
        }
        core::str::from_utf8(&self.name[..len]).unwrap_or("")
    }
}

/// Gestor del Sistema de Archivos Persistente BeaverFS
pub struct BeaverFs {
    pub is_mounted: bool,
    pub superblock: Superblock,
}

impl BeaverFs {
    pub const fn new() -> Self {
        Self {
            is_mounted: false,
            superblock: Superblock::new(BEAVERFS_TOTAL_SECTORS),
        }
    }

    /// Formatea la partición ATA con BeaverFS
    pub fn format(&mut self) -> Result<(), &'static str> {
        let mut block_dev = BLOCK_DEVICE.lock();

        // 1. Crear y escribir el superbloque
        let sb = Superblock::new(BEAVERFS_TOTAL_SECTORS);
        let mut sb_buf = [0_u8; SECTOR_SIZE];
        // SAFETY: `sb` es una estructura `#[repr(C, packed)]` de tamaño inferior a `SECTOR_SIZE` (512 bytes) y `sb_buf` no se solapa con `sb`.
        unsafe {
            core::ptr::copy_nonoverlapping(
                &sb as *const Superblock as *const u8,
                sb_buf.as_mut_ptr(),
                core::mem::size_of::<Superblock>(),
            );
        }
        block_dev.write_sector(BEAVERFS_START_LBA + SUPERBLOCK_REL_LBA, &sb_buf)?;

        // 2. Inicializar Bitmap de Bloques (primeros bloques reservados marcados como usados)
        let mut bitmap_buf = [0_u8; SECTOR_SIZE];
        // Los primeros DATA_BLOCKS_START_REL_LBA (10 bloques: 0..9) están ocupados por metadata
        // Marcamos los primeros 10 bits como 1: 0xFF en byte 0 y 0x03 en byte 1
        bitmap_buf[0] = 0xFF;
        bitmap_buf[1] = 0x03;
        block_dev.write_sector(BEAVERFS_START_LBA + BLOCK_BITMAP_REL_LBA, &bitmap_buf)?;

        // 3. Inicializar Tabla de Inodos (todo en 0 salvo el inodo raíz 0)
        let empty_buf = [0_u8; SECTOR_SIZE];
        for sec in 0..INODE_SECTORS {
            block_dev.write_sector(BEAVERFS_START_LBA + INODE_TABLE_REL_LBA + sec, &empty_buf)?;
        }

        // Crear Inodo Raíz (Inodo 0: Directorio /)
        let mut root_inode = DiskInode::empty();
        root_inode.inode_type = InodeType::Directory as u8;
        root_inode.permissions = 0o755;
        root_inode.uid = 0;
        root_inode.gid = 0;
        root_inode.size_bytes = 0;
        root_inode.block_count = 1;
        // Asignar primer bloque de datos (LBA relativo 10) al inodo raíz
        root_inode.direct_blocks[0] = DATA_BLOCKS_START_REL_LBA;

        // Actualizar bitmap para el bloque 10 (bit 2 del byte 1)
        bitmap_buf[1] |= 0x04;
        block_dev.write_sector(BEAVERFS_START_LBA + BLOCK_BITMAP_REL_LBA, &bitmap_buf)?;

        // Guardar Inodo Raíz en el primer sector de inodos
        let mut inode_sec_buf = [0_u8; SECTOR_SIZE];
        // SAFETY: `root_inode` cabe dentro de `inode_sec_buf` (64 <= 512 bytes) y ambas regiones de memoria son independientes.
        unsafe {
            core::ptr::copy_nonoverlapping(
                &root_inode as *const DiskInode as *const u8,
                inode_sec_buf.as_mut_ptr(),
                core::mem::size_of::<DiskInode>(),
            );
        }
        block_dev.write_sector(BEAVERFS_START_LBA + INODE_TABLE_REL_LBA, &inode_sec_buf)?;

        // Limpiar el bloque de datos del directorio raíz con entradas vacías
        let mut dir_init_buf = [0_u8; SECTOR_SIZE];
        let entries_count = SECTOR_SIZE / core::mem::size_of::<DirEntry>();
        for i in 0..entries_count {
            let empty_entry = DirEntry::empty();
            // SAFETY: `i * size_of::<DirEntry>()` permanece dentro de los 512 bytes de `dir_init_buf` y no hay solapamiento con `empty_entry`.
            unsafe {
                let ptr = dir_init_buf.as_mut_ptr().add(i * core::mem::size_of::<DirEntry>());
                core::ptr::copy_nonoverlapping(&empty_entry as *const DirEntry as *const u8, ptr, core::mem::size_of::<DirEntry>());
            }
        }
        block_dev.write_sector(BEAVERFS_START_LBA + DATA_BLOCKS_START_REL_LBA, &dir_init_buf)?;

        self.superblock = sb;
        self.is_mounted = true;
        Ok(())
    }

    /// Intenta montar la partición leyendo el superbloque
    pub fn mount(&mut self) -> Result<(), &'static str> {
        let block_dev = BLOCK_DEVICE.lock();
        let mut buf = [0_u8; SECTOR_SIZE];
        block_dev.read_sector(BEAVERFS_START_LBA + SUPERBLOCK_REL_LBA, &mut buf)?;

        // SAFETY: `read_unaligned` lee de forma segura un `Superblock` (`#[repr(C, packed)]`) desde los primeros bytes de `buf`.
        let sb = unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const Superblock) };
        if sb.magic != BEAVERFS_MAGIC {
            return Err("Firma BeaverFS no detectada en disco (Superbloque no inicializado)");
        }

        self.superblock = sb;
        self.is_mounted = true;
        Ok(())
    }

    /// Lee un inodo de la tabla
    pub fn read_inode(&self, inode_idx: u32) -> Result<DiskInode, &'static str> {
        if inode_idx >= MAX_INODES as u32 {
            return Err("Índice de inodo fuera de rango");
        }

        let sec_offset = inode_idx / (INODES_PER_SECTOR as u32);
        let in_sec_idx = (inode_idx % (INODES_PER_SECTOR as u32)) as usize;

        let block_dev = BLOCK_DEVICE.lock();
        let mut buf = [0_u8; SECTOR_SIZE];
        block_dev.read_sector(BEAVERFS_START_LBA + INODE_TABLE_REL_LBA + sec_offset, &mut buf)?;

        // SAFETY: `in_sec_idx < INODES_PER_SECTOR` garantiza que el desplazamiento de 64 bytes esté dentro de `buf` (512 bytes).
        let inode = unsafe {
            let ptr = buf.as_ptr().add(in_sec_idx * core::mem::size_of::<DiskInode>()) as *const DiskInode;
            core::ptr::read_unaligned(ptr)
        };
        Ok(inode)
    }

    /// Escribe un inodo en la tabla
    pub fn write_inode(&self, inode_idx: u32, inode: &DiskInode) -> Result<(), &'static str> {
        if inode_idx >= MAX_INODES as u32 {
            return Err("Índice de inodo fuera de rango");
        }

        let sec_offset = inode_idx / (INODES_PER_SECTOR as u32);
        let in_sec_idx = (inode_idx % (INODES_PER_SECTOR as u32)) as usize;

        let mut block_dev = BLOCK_DEVICE.lock();
        let mut buf = [0_u8; SECTOR_SIZE];
        block_dev.read_sector(BEAVERFS_START_LBA + INODE_TABLE_REL_LBA + sec_offset, &mut buf)?;

        // SAFETY: `in_sec_idx < INODES_PER_SECTOR` asegura que la escritura de `DiskInode` (64 bytes) ocurra dentro de `buf` sin solapamiento.
        unsafe {
            let ptr = buf.as_mut_ptr().add(in_sec_idx * core::mem::size_of::<DiskInode>());
            core::ptr::copy_nonoverlapping(inode as *const DiskInode as *const u8, ptr, core::mem::size_of::<DiskInode>());
        }

        block_dev.write_sector(BEAVERFS_START_LBA + INODE_TABLE_REL_LBA + sec_offset, &buf)?;
        Ok(())
    }

    /// Asigna un inodo libre
    fn alloc_inode(&mut self, itype: InodeType, perms: u16, uid: u32, gid: u32) -> Result<u32, &'static str> {
        for i in 1..MAX_INODES as u32 {
            let inode = self.read_inode(i)?;
            if inode.inode_type == InodeType::Free as u8 {
                let mut new_inode = DiskInode::empty();
                new_inode.inode_type = itype as u8;
                new_inode.permissions = perms;
                new_inode.uid = uid;
                new_inode.gid = gid;
                self.write_inode(i, &new_inode)?;
                return Ok(i);
            }
        }
        Err("Tabla de inodos llena (MAX_INODES alcanzado)")
    }

    /// Asigna un bloque de datos libre desde el bitmap
    fn alloc_data_block(&mut self) -> Result<u32, &'static str> {
        let mut block_dev = BLOCK_DEVICE.lock();
        let mut bitmap = [0_u8; SECTOR_SIZE];
        block_dev.read_sector(BEAVERFS_START_LBA + BLOCK_BITMAP_REL_LBA, &mut bitmap)?;

        for byte_idx in 0..SECTOR_SIZE {
            if bitmap[byte_idx] != 0xFF {
                for bit in 0..8 {
                    if (bitmap[byte_idx] & (1 << bit)) == 0 {
                        let block_num = (byte_idx * 8 + bit) as u32;
                        if block_num < BEAVERFS_TOTAL_SECTORS {
                            bitmap[byte_idx] |= 1 << bit;
                            block_dev.write_sector(BEAVERFS_START_LBA + BLOCK_BITMAP_REL_LBA, &bitmap)?;
                            return Ok(block_num);
                        }
                    }
                }
            }
        }
        Err("No hay bloques de disco libres en BeaverFS")
    }

    /// Busca una entrada por nombre en un directorio dado por su índice de inodo
    pub fn find_entry_in_dir(&self, dir_inode_idx: u32, name: &str) -> Result<Option<(DirEntry, u32, usize)>, &'static str> {
        let dir_inode = self.read_inode(dir_inode_idx)?;
        if dir_inode.inode_type != InodeType::Directory as u8 {
            return Err("El inodo especificado no es un directorio");
        }
        let entries_count = SECTOR_SIZE / core::mem::size_of::<DirEntry>();
        let direct = dir_inode.direct_blocks;
        for block in direct {
            if block == 0 {
                continue;
            }
            let mut dir_buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + block, &mut dir_buf)?;
            for i in 0..entries_count {
                // SAFETY: `i * size_of::<DirEntry>()` permanece dentro de los 512 bytes de `dir_buf` y `read_unaligned` soporta `#[repr(C, packed)]`.
                let entry = unsafe {
                    let ptr = dir_buf.as_ptr().add(i * core::mem::size_of::<DirEntry>()) as *const DirEntry;
                    core::ptr::read_unaligned(ptr)
                };
                if entry.is_valid() && entry.name_as_str() == name {
                    return Ok(Some((entry, block, i)));
                }
            }
        }
        Ok(None)
    }

    /// Resuelve una ruta jerárquica de directorio hasta obtener su índice de inodo
    /// Ejemplo: "" o "/" -> Ok(0) (directorio raíz)
    /// Ejemplo: "docs/work" -> resuelve "docs" en inodo 0, luego "work" en inodo de docs
    pub fn resolve_dir_inode(&self, path: &str) -> Result<u32, &'static str> {
        let clean = path.trim().trim_start_matches('/').trim_end_matches('/');
        let clean = clean.strip_prefix("disk/").unwrap_or(clean);
        if clean.is_empty() {
            return Ok(0);
        }

        let mut current_inode = 0;
        for seg in clean.split('/') {
            if seg.is_empty() {
                continue;
            }
            let (entry, _, _) = self.find_entry_in_dir(current_inode, seg)?
                .ok_or("Directorio intermedio no encontrado en BeaverFS")?;
            if entry.entry_type != InodeType::Directory as u8 {
                return Err("Un componente de la ruta no es un directorio");
            }
            current_inode = entry.inode_index;
        }
        Ok(current_inode)
    }

    /// Resuelve la ruta separando el inodo del directorio padre y el nombre final del elemento
    /// Ejemplo: "docs/report.txt" -> Ok((inode_docs, "report.txt"))
    /// Ejemplo: "report.txt" -> Ok((0, "report.txt"))
    pub fn resolve_parent_dir<'a>(&self, path: &'a str) -> Result<(u32, &'a str), &'static str> {
        let clean = path.trim().trim_start_matches('/').trim_end_matches('/');
        let clean = clean.strip_prefix("disk/").unwrap_or(clean);
        if clean.is_empty() {
            return Err("Ruta vacía");
        }
        if let Some(pos) = clean.rfind('/') {
            let dir_part = &clean[..pos];
            let name = &clean[pos + 1..];
            let dir_inode = self.resolve_dir_inode(dir_part)?;
            Ok((dir_inode, name))
        } else {
            Ok((0, clean))
        }
    }

    /// Crea recursivamente los directorios intermedios necesarios si no existen
    pub fn ensure_parent_dirs(&mut self, path: &str, uid: u32, gid: u32) -> Result<(), &'static str> {
        let clean = path.trim().trim_start_matches('/').trim_end_matches('/');
        let clean = clean.strip_prefix("disk/").unwrap_or(clean);
        if let Some(pos) = clean.rfind('/') {
            let dir_part = &clean[..pos];
            let mut current = String::new();
            for seg in dir_part.split('/') {
                if seg.is_empty() { continue; }
                if !current.is_empty() {
                    current.push('/');
                }
                current.push_str(seg);
                if !self.is_dir(&current) {
                    self.create_dir(&current, uid, gid)?;
                }
            }
        }
        Ok(())
    }

    /// Añade una entrada (archivo o subdirectorio) a un directorio dado por su inodo
    pub fn add_entry_to_dir(&mut self, dir_inode_idx: u32, entry: DirEntry) -> Result<(), &'static str> {
        let mut dir_inode = self.read_inode(dir_inode_idx)?;
        if dir_inode.inode_type != InodeType::Directory as u8 {
            return Err("El inodo destino no es un directorio");
        }
        let entries_count = SECTOR_SIZE / core::mem::size_of::<DirEntry>();

        // 1. Buscar slot libre en los bloques asignados existentes
        let direct = dir_inode.direct_blocks;
        for block in direct {
            if block == 0 {
                continue;
            }
            let mut buf = [0_u8; SECTOR_SIZE];
            let mut bd = BLOCK_DEVICE.lock();
            bd.read_sector(BEAVERFS_START_LBA + block, &mut buf)?;
            for i in 0..entries_count {
                let existing = unsafe {
                    let ptr = buf.as_ptr().add(i * core::mem::size_of::<DirEntry>()) as *const DirEntry;
                    core::ptr::read_unaligned(ptr)
                };
                if existing.is_free() {
                    unsafe {
                        let ptr = buf.as_mut_ptr().add(i * core::mem::size_of::<DirEntry>());
                        core::ptr::copy_nonoverlapping(&entry as *const DirEntry as *const u8, ptr, core::mem::size_of::<DirEntry>());
                    }
                    bd.write_sector(BEAVERFS_START_LBA + block, &buf)?;
                    return Ok(());
                }
            }
        }

        // 2. Si no hay slot libre, asignar un nuevo bloque directo al directorio
        for b in 0..7 {
            if dir_inode.direct_blocks[b] == 0 {
                let new_block = self.alloc_data_block()?;
                let mut init_buf = [0_u8; SECTOR_SIZE];
                let empty_entry = DirEntry::empty();
                for i in 0..entries_count {
                    unsafe {
                        let ptr = init_buf.as_mut_ptr().add(i * core::mem::size_of::<DirEntry>());
                        core::ptr::copy_nonoverlapping(&empty_entry as *const DirEntry as *const u8, ptr, core::mem::size_of::<DirEntry>());
                    }
                }
                // Insertar la nueva entrada en el primer slot
                unsafe {
                    core::ptr::copy_nonoverlapping(&entry as *const DirEntry as *const u8, init_buf.as_mut_ptr(), core::mem::size_of::<DirEntry>());
                }
                BLOCK_DEVICE.lock().write_sector(BEAVERFS_START_LBA + new_block, &init_buf)?;
                dir_inode.direct_blocks[b] = new_block;
                dir_inode.block_count += 1;
                self.write_inode(dir_inode_idx, &dir_inode)?;
                return Ok(());
            }
        }

        Err("Directorio lleno (límite de entradas por directorio alcanzado)")
    }

    /// Busca un archivo o subdirectorio por su ruta jerárquica
    fn find_file_inode(&self, path: &str) -> Result<Option<(u32, u32, usize)>, &'static str> {
        let (parent_inode_idx, name) = match self.resolve_parent_dir(path) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        if let Some((entry, block, slot)) = self.find_entry_in_dir(parent_inode_idx, name)? {
            Ok(Some((entry.inode_index, block, slot)))
        } else {
            Ok(None)
        }
    }

    /// Crea un subdirectorio persistente en BeaverFS (ej: "docs" o "docs/proyectos")
    pub fn create_dir(&mut self, path: &str, uid: u32, gid: u32) -> Result<(), &'static str> {
        if !self.is_mounted {
            return Err("BeaverFS no montado.");
        }
        let (parent_inode_idx, dir_name) = self.resolve_parent_dir(path)?;
        if dir_name.is_empty() || dir_name.len() > 55 {
            return Err("Nombre de directorio no válido (máximo 55 caracteres)");
        }

        // Comprobar que no exista ya un elemento con ese nombre
        if self.find_entry_in_dir(parent_inode_idx, dir_name)?.is_some() {
            return Err("Ya existe un archivo o directorio con ese nombre en BeaverFS");
        }

        // Asignar inodo de tipo Directorio
        let new_inode_idx = self.alloc_inode(InodeType::Directory, 0o755, uid, gid)?;

        // Asignar primer bloque de datos para las entradas del subdirectorio
        let new_block = match self.alloc_data_block() {
            Ok(b) => b,
            Err(e) => {
                let mut empty_inode = DiskInode::empty();
                empty_inode.inode_type = InodeType::Free as u8;
                let _ = self.write_inode(new_inode_idx, &empty_inode);
                return Err(e);
            }
        };

        let mut init_buf = [0_u8; SECTOR_SIZE];
        let empty_entry = DirEntry::empty();
        let entries_count = SECTOR_SIZE / core::mem::size_of::<DirEntry>();
        for i in 0..entries_count {
            unsafe {
                let ptr = init_buf.as_mut_ptr().add(i * core::mem::size_of::<DirEntry>());
                core::ptr::copy_nonoverlapping(&empty_entry as *const DirEntry as *const u8, ptr, core::mem::size_of::<DirEntry>());
            }
        }
        BLOCK_DEVICE.lock().write_sector(BEAVERFS_START_LBA + new_block, &init_buf)?;

        // Guardar inodo del nuevo directorio
        let mut new_dir_inode = self.read_inode(new_inode_idx)?;
        new_dir_inode.direct_blocks[0] = new_block;
        new_dir_inode.block_count = 1;
        new_dir_inode.size_bytes = 0;
        self.write_inode(new_inode_idx, &new_dir_inode)?;

        // Agregar la entrada al directorio padre
        let entry = DirEntry::new(new_inode_idx, dir_name, InodeType::Directory as u8);
        self.add_entry_to_dir(parent_inode_idx, entry)?;

        Ok(())
    }

    /// Comprueba si una ruta corresponde a un directorio válido
    pub fn is_dir(&self, path: &str) -> bool {
        if !self.is_mounted {
            return false;
        }
        match self.resolve_dir_inode(path) {
            Ok(idx) => {
                if let Ok(inode) = self.read_inode(idx) {
                    inode.inode_type == InodeType::Directory as u8
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    /// Comprueba si un archivo o directorio existe en BeaverFS
    pub fn exists(&self, path: &str) -> bool {
        if !self.is_mounted {
            return false;
        }
        let clean = path.trim().trim_start_matches('/').trim_end_matches('/');
        if clean.is_empty() {
            return true;
        }
        if self.resolve_dir_inode(clean).is_ok() {
            return true;
        }
        if let Ok((parent, name)) = self.resolve_parent_dir(clean) {
            if let Ok(Some(_)) = self.find_entry_in_dir(parent, name) {
                return true;
            }
        }
        false
    }

    /// Lista los archivos y subdirectorios dentro de una ruta de BeaverFS
    pub fn list_dir(&self, path: &str) -> Result<Vec<(String, bool, usize, String)>, &'static str> {
        if !self.is_mounted {
            return Err("BeaverFS no montado.");
        }
        let dir_inode_idx = self.resolve_dir_inode(path)?;
        let dir_inode = self.read_inode(dir_inode_idx)?;
        if dir_inode.inode_type != InodeType::Directory as u8 {
            return Err("La ruta especificada no es un directorio");
        }

        let entries_count = SECTOR_SIZE / core::mem::size_of::<DirEntry>();
        let mut list = Vec::new();
        let direct = dir_inode.direct_blocks;

        for block in direct {
            if block == 0 {
                continue;
            }
            let mut buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + block, &mut buf)?;
            for i in 0..entries_count {
                let entry = unsafe {
                    let ptr = buf.as_ptr().add(i * core::mem::size_of::<DirEntry>()) as *const DirEntry;
                    core::ptr::read_unaligned(ptr)
                };
                if entry.is_valid() {
                    let is_dir = entry.entry_type == InodeType::Directory as u8;
                    let target_inode = self.read_inode(entry.inode_index).unwrap_or(DiskInode::empty());
                    let size = target_inode.size_bytes as usize;
                    let perms = target_inode.permissions;
                    let prefix = if is_dir { 'd' } else { '-' };
                    let r_u = if (perms & 0o400) != 0 { 'r' } else { '-' };
                    let w_u = if (perms & 0o200) != 0 { 'w' } else { '-' };
                    let x_u = if (perms & 0o100) != 0 { 'x' } else { '-' };
                    let r_g = if (perms & 0o040) != 0 { 'r' } else { '-' };
                    let w_g = if (perms & 0o020) != 0 { 'w' } else { '-' };
                    let x_g = if (perms & 0o010) != 0 { 'x' } else { '-' };
                    let r_o = if (perms & 0o004) != 0 { 'r' } else { '-' };
                    let w_o = if (perms & 0o002) != 0 { 'w' } else { '-' };
                    let x_o = if (perms & 0o001) != 0 { 'x' } else { '-' };
                    let mode_str = alloc::format!("{}{}{}{}{}{}{}{}{}{}", prefix, r_u, w_u, x_u, r_g, w_g, x_g, r_o, w_o, x_o);
                    list.push((String::from(entry.name_as_str()), is_dir, size, mode_str));
                }
            }
        }
        Ok(list)
    }

    /// Elimina un subdirectorio si está completamente vacío
    pub fn remove_dir(&mut self, path: &str) -> Result<(), &'static str> {
        if !self.is_mounted {
            return Err("BeaverFS no montado.");
        }
        let (parent_inode_idx, dir_name) = self.resolve_parent_dir(path)?;
        let (entry, target_block, slot) = self.find_entry_in_dir(parent_inode_idx, dir_name)?
            .ok_or("Directorio no encontrado en BeaverFS")?;
        if entry.entry_type != InodeType::Directory as u8 {
            return Err("La ruta especificada no es un directorio");
        }
        let dir_inode_idx = entry.inode_index;
        let dir_inode = self.read_inode(dir_inode_idx)?;

        // Verificar que el directorio esté vacío
        let entries_count = SECTOR_SIZE / core::mem::size_of::<DirEntry>();
        let direct = dir_inode.direct_blocks;
        for block in direct {
            if block == 0 {
                continue;
            }
            let mut buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + block, &mut buf)?;
            for i in 0..entries_count {
                let ent = unsafe {
                    let ptr = buf.as_ptr().add(i * core::mem::size_of::<DirEntry>()) as *const DirEntry;
                    core::ptr::read_unaligned(ptr)
                };
                if ent.is_valid() {
                    return Err("El directorio no está vacío");
                }
            }
        }

        // 1. Liberar inodo
        let mut free_inode = dir_inode;
        free_inode.inode_type = InodeType::Free as u8;
        self.write_inode(dir_inode_idx, &free_inode)?;

        // 2. Liberar bloques del directorio en el bitmap
        let mut bitmap = [0_u8; SECTOR_SIZE];
        let mut bd = BLOCK_DEVICE.lock();
        bd.read_sector(BEAVERFS_START_LBA + BLOCK_BITMAP_REL_LBA, &mut bitmap)?;
        let direct = dir_inode.direct_blocks;
        for blk in direct {
            if blk >= DATA_BLOCKS_START_REL_LBA && blk < BEAVERFS_TOTAL_SECTORS {
                let byte_idx = (blk / 8) as usize;
                let bit_idx = (blk % 8) as u8;
                if byte_idx < SECTOR_SIZE {
                    bitmap[byte_idx] &= !(1 << bit_idx);
                }
            }
        }
        bd.write_sector(BEAVERFS_START_LBA + BLOCK_BITMAP_REL_LBA, &bitmap)?;

        // 3. Limpiar entrada en el bloque del directorio padre
        let mut dir_buf = [0_u8; SECTOR_SIZE];
        bd.read_sector(BEAVERFS_START_LBA + target_block, &mut dir_buf)?;
        let empty_entry = DirEntry::empty();
        unsafe {
            let ptr = dir_buf.as_mut_ptr().add(slot * core::mem::size_of::<DirEntry>());
            core::ptr::copy_nonoverlapping(&empty_entry as *const DirEntry as *const u8, ptr, core::mem::size_of::<DirEntry>());
        }
        bd.write_sector(BEAVERFS_START_LBA + target_block, &dir_buf)?;

        Ok(())
    }

    /// Escribe un archivo persistente en disco (creándolo o sobrescribiéndolo con soporte multi-bloque)
    pub fn write_file(&mut self, path: &str, content: &[u8], uid: u32, gid: u32) -> Result<(), &'static str> {
        if !self.is_mounted {
            return Err("BeaverFS no montado. Ejecuta 'format-disk' o 'mount-disk' primero.");
        }
        self.ensure_parent_dirs(path, uid, gid)?;
        let (parent_inode_idx, name) = self.resolve_parent_dir(path)?;
        if name.is_empty() || name.len() > 55 {
            return Err("Nombre de archivo inválido (máximo 55 caracteres)");
        }

        let existing_entry = self.find_entry_in_dir(parent_inode_idx, name)?;
        let inode_idx = match existing_entry {
            Some((entry, _, _)) => {
                if entry.entry_type == InodeType::Directory as u8 {
                    return Err("La ruta especificada es un directorio, no un archivo");
                }
                entry.inode_index
            }
            None => {
                let new_idx = self.alloc_inode(InodeType::RegularFile, 0o644, uid, gid)?;
                let new_entry = DirEntry::new(new_idx, name, InodeType::RegularFile as u8);
                self.add_entry_to_dir(parent_inode_idx, new_entry)?;
                new_idx
            }
        };

        // 2. Asignar bloques de datos para el contenido del archivo
        let needed_blocks = if content.is_empty() { 1 } else { (content.len() + SECTOR_SIZE - 1) / SECTOR_SIZE };
        const MAX_FILE_BLOCKS: usize = 7 + 128 + (128 * 128); // 16,519 bloques = 8.06 MiB
        if needed_blocks > MAX_FILE_BLOCKS {
            return Err("Archivo excede el tamaño máximo soportado en BeaverFS (8 MiB)");
        }

        let mut data_blocks = Vec::with_capacity(needed_blocks);
        for _ in 0..needed_blocks {
            data_blocks.push(self.alloc_data_block()?);
        }

        for (i, &blk) in data_blocks.iter().enumerate() {
            let start = i * SECTOR_SIZE;
            let end = (start + SECTOR_SIZE).min(content.len());
            let mut data_buf = [0_u8; SECTOR_SIZE];
            if start < content.len() {
                data_buf[..end - start].copy_from_slice(&content[start..end]);
            }
            BLOCK_DEVICE.lock().write_sector(BEAVERFS_START_LBA + blk, &data_buf)?;
        }

        // 3. Actualizar inodo con el tamaño y punteros a bloques
        let mut file_inode = self.read_inode(inode_idx)?;
        file_inode.size_bytes = content.len() as u32;
        file_inode.block_count = needed_blocks as u16;

        let direct_count = needed_blocks.min(7);
        for i in 0..direct_count {
            file_inode.direct_blocks[i] = data_blocks[i];
        }
        for i in direct_count..7 {
            file_inode.direct_blocks[i] = 0;
        }

        // Bloque indirecto simple (hasta 128 bloques = 64 KiB)
        if needed_blocks > 7 {
            let ind_blk = self.alloc_data_block()?;
            let mut ind_buf = [0_u8; SECTOR_SIZE];
            let ind_limit = needed_blocks.min(7 + 128);
            for (idx, &blk) in data_blocks[7..ind_limit].iter().enumerate() {
                let bytes = blk.to_le_bytes();
                ind_buf[idx * 4..idx * 4 + 4].copy_from_slice(&bytes);
            }
            BLOCK_DEVICE.lock().write_sector(BEAVERFS_START_LBA + ind_blk, &ind_buf)?;
            file_inode.indirect_block = ind_blk;
        } else {
            file_inode.indirect_block = 0;
        }

        // Bloque doblemente indirecto (hasta 128 x 128 bloques = 8 MiB)
        if needed_blocks > 7 + 128 {
            let dind_blk = self.alloc_data_block()?;
            let mut dind_buf = [0_u8; SECTOR_SIZE];
            let remaining = &data_blocks[7 + 128..];
            let num_tables = (remaining.len() + 127) / 128;

            for t in 0..num_tables {
                let table_blk = self.alloc_data_block()?;
                dind_buf[t * 4..t * 4 + 4].copy_from_slice(&table_blk.to_le_bytes());

                let start_idx = t * 128;
                let end_idx = (start_idx + 128).min(remaining.len());
                let mut table_buf = [0_u8; SECTOR_SIZE];
                for (entry, &blk) in remaining[start_idx..end_idx].iter().enumerate() {
                    table_buf[entry * 4..entry * 4 + 4].copy_from_slice(&blk.to_le_bytes());
                }
                BLOCK_DEVICE.lock().write_sector(BEAVERFS_START_LBA + table_blk, &table_buf)?;
            }
            BLOCK_DEVICE.lock().write_sector(BEAVERFS_START_LBA + dind_blk, &dind_buf)?;
            file_inode.double_indirect = dind_blk;
        } else {
            file_inode.double_indirect = 0;
        }

        self.write_inode(inode_idx, &file_inode)?;
        Ok(())
    }

    /// Lee un archivo persistente desde el disco ATA (soporta archivos multi-bloque)
    pub fn read_file(&self, name: &str) -> Result<Vec<u8>, &'static str> {
        if !self.is_mounted {
            return Err("BeaverFS no montado.");
        }

        let (inode_idx, _, _) = self.find_file_inode(name)?.ok_or("Archivo no encontrado en BeaverFS")?;
        let inode = self.read_inode(inode_idx)?;

        let mut blocks = Vec::new();
        let direct = inode.direct_blocks;
        for blk in direct {
            if blk != 0 {
                blocks.push(blk);
            }
        }

        let ind_blk = inode.indirect_block;
        if ind_blk != 0 {
            let mut ind_buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + ind_blk, &mut ind_buf)?;
            for chunk in ind_buf.chunks_exact(4) {
                let blk = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                if blk != 0 && blk != 0xFFFF_FFFF {
                    blocks.push(blk);
                }
            }
        }

        let dind_blk = inode.double_indirect;
        if dind_blk != 0 {
            let mut dind_buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + dind_blk, &mut dind_buf)?;
            for chunk in dind_buf.chunks_exact(4) {
                let table_blk = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                if table_blk != 0 && table_blk != 0xFFFF_FFFF {
                    let mut table_buf = [0_u8; SECTOR_SIZE];
                    BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + table_blk, &mut table_buf)?;
                    for entry_chunk in table_buf.chunks_exact(4) {
                        let blk = u32::from_le_bytes([entry_chunk[0], entry_chunk[1], entry_chunk[2], entry_chunk[3]]);
                        if blk != 0 && blk != 0xFFFF_FFFF {
                            blocks.push(blk);
                        }
                    }
                }
            }
        }

        let mut out = Vec::with_capacity(inode.size_bytes as usize);
        let mut remaining = inode.size_bytes as usize;
        for &blk in &blocks {
            if remaining == 0 {
                break;
            }
            let mut data_buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + blk, &mut data_buf)?;
            let to_read = remaining.min(SECTOR_SIZE);
            out.extend_from_slice(&data_buf[..to_read]);
            remaining -= to_read;
        }

        Ok(out)
    }

    /// Lista los archivos registrados en el directorio raíz de BeaverFS
    pub fn list_files(&self) -> Result<Vec<(String, u32, u16)>, &'static str> {
        if !self.is_mounted {
            return Err("BeaverFS no montado.");
        }

        let root_inode = self.read_inode(0)?;
        let mut list = Vec::new();
        let entries_count = SECTOR_SIZE / core::mem::size_of::<DirEntry>();
        let root_blocks = root_inode.direct_blocks;

        for root_block in root_blocks {
            if root_block == 0 {
                continue;
            }
            let mut dir_buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + root_block, &mut dir_buf)?;

            for i in 0..entries_count {
                // SAFETY: `i * size_of::<DirEntry>()` permanece dentro de los 512 bytes de `dir_buf` y se lee sin requerir alineación estricta.
                let entry = unsafe {
                    let ptr = dir_buf.as_ptr().add(i * core::mem::size_of::<DirEntry>()) as *const DirEntry;
                    core::ptr::read_unaligned(ptr)
                };
                if entry.is_valid() {
                    let name = String::from(entry.name_as_str());
                    if let Ok(inode) = self.read_inode(entry.inode_index) {
                        list.push((name, inode.size_bytes, inode.permissions));
                    }
                }
            }
        }

        Ok(list)
    }

    /// Elimina un archivo liberando su inodo, bloques en el bitmap y entrada de directorio
    pub fn remove_file(&mut self, name: &str) -> Result<(), &'static str> {
        if !self.is_mounted {
            return Err("BeaverFS no montado.");
        }

        let (in_idx, target_block, slot) = self.find_file_inode(name)?.ok_or("Archivo no encontrado en BeaverFS")?;

        // 1. Liberar inodo y sus bloques
        let inode = self.read_inode(in_idx)?;
        let mut blocks_to_free = Vec::new();
        let direct = inode.direct_blocks;
        for blk in direct {
            if blk != 0 {
                blocks_to_free.push(blk);
            }
        }
        let ind_blk = inode.indirect_block;
        if ind_blk != 0 {
            blocks_to_free.push(ind_blk);
            let mut ind_buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + ind_blk, &mut ind_buf)?;
            for chunk in ind_buf.chunks_exact(4) {
                let blk = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                if blk != 0 && blk != 0xFFFF_FFFF {
                    blocks_to_free.push(blk);
                }
            }
        }

        let dind_blk = inode.double_indirect;
        if dind_blk != 0 {
            blocks_to_free.push(dind_blk);
            let mut dind_buf = [0_u8; SECTOR_SIZE];
            BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + dind_blk, &mut dind_buf)?;
            for chunk in dind_buf.chunks_exact(4) {
                let table_blk = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                if table_blk != 0 && table_blk != 0xFFFF_FFFF {
                    blocks_to_free.push(table_blk);
                    let mut table_buf = [0_u8; SECTOR_SIZE];
                    BLOCK_DEVICE.lock().read_sector(BEAVERFS_START_LBA + table_blk, &mut table_buf)?;
                    for entry_chunk in table_buf.chunks_exact(4) {
                        let blk = u32::from_le_bytes([entry_chunk[0], entry_chunk[1], entry_chunk[2], entry_chunk[3]]);
                        if blk != 0 && blk != 0xFFFF_FFFF {
                            blocks_to_free.push(blk);
                        }
                    }
                }
            }
        }

        // Marcar inodo libre
        let mut free_inode = inode;
        free_inode.inode_type = InodeType::Free as u8;
        self.write_inode(in_idx, &free_inode)?;

        // 2. Liberar bloques de datos en el bitmap
        let mut bitmap = [0_u8; SECTOR_SIZE];
        let mut bd = BLOCK_DEVICE.lock();
        bd.read_sector(BEAVERFS_START_LBA + BLOCK_BITMAP_REL_LBA, &mut bitmap)?;
        for blk in blocks_to_free {
            if blk >= DATA_BLOCKS_START_REL_LBA && blk < BEAVERFS_TOTAL_SECTORS {
                let byte_idx = (blk / 8) as usize;
                let bit_idx = (blk % 8) as u8;
                if byte_idx < SECTOR_SIZE {
                    bitmap[byte_idx] &= !(1 << bit_idx);
                }
            }
        }
        bd.write_sector(BEAVERFS_START_LBA + BLOCK_BITMAP_REL_LBA, &bitmap)?;

        // 3. Limpiar entrada de directorio en el bloque específico
        let mut dir_buf = [0_u8; SECTOR_SIZE];
        bd.read_sector(BEAVERFS_START_LBA + target_block, &mut dir_buf)?;
        let empty_entry = DirEntry::empty();
        // SAFETY: `slot < entries_count` garantiza que la escritura de `empty_entry` (64 bytes) ocurra dentro de `dir_buf`.
        unsafe {
            let ptr = dir_buf.as_mut_ptr().add(slot * core::mem::size_of::<DirEntry>());
            core::ptr::copy_nonoverlapping(&empty_entry as *const DirEntry as *const u8, ptr, core::mem::size_of::<DirEntry>());
        }
        bd.write_sector(BEAVERFS_START_LBA + target_block, &dir_buf)?;

        Ok(())
    }
}

pub static BEAVER_FS: SpinMutex<BeaverFs> = SpinMutex::new(BeaverFs::new());

pub fn init_beaverfs() {
    let mut fs = BEAVER_FS.lock();
    match fs.mount() {
        Ok(()) => {
            crate::kprintln!("[FS] BeaverFS montado exitosamente desde disco ATA (Magic: 0x{:08X})", BEAVERFS_MAGIC);
        }
        Err(_) => {
            // Si el disco aún no está formateado, se formatea automáticamente en el primer arranque
            if fs.format().is_ok() {
                crate::kprintln!("[FS] BeaverFS formateado e inicializado en disco ATA (Partición LBA {})", BEAVERFS_START_LBA);
            }
        }
    }
}
