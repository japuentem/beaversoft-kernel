// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: ELF64 Executable Binary Parser, Memory Mapper & Ring 3 Loader
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use crate::memory::frame_allocator::{FRAME_ALLOCATOR, PAGE_SIZE};
use crate::usermode::jump_to_ring3;
use crate::vga::VgaColor;
use crate::{kprintln, kprint_color};

/// Número mágico de cabecera ELF: 0x7F, 'E', 'L', 'F'
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

pub const ELF_CLASS_64: u8 = 2;
pub const ELF_DATA_2LSB: u8 = 1; // Little-Endian
pub const ELF_TYPE_EXEC: u16 = 2; // Archivo ejecutable
pub const ELF_MACHINE_X86_64: u16 = 0x3E; // AMD x86-64

pub const PT_LOAD: u32 = 1; // Segmento cargable en memoria

pub const PF_X: u32 = 1; // Ejecución
pub const PF_W: u32 = 2; // Escritura
pub const PF_R: u32 = 4; // Lectura

/// Cabecera ELF de 64 bits (64 bytes exactos)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct Elf64Header {
    pub ident: [u8; 16],
    pub elf_type: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: u64,
    pub ph_offset: u64,
    pub sh_offset: u64,
    pub flags: u32,
    pub header_size: u16,
    pub ph_entry_size: u16,
    pub ph_num: u16,
    pub sh_entry_size: u16,
    pub sh_num: u16,
    pub sh_str_index: u16,
}

/// Cabecera de Segmento de Programa (Program Header) ELF64 (56 bytes exactos)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct Elf64ProgramHeader {
    pub seg_type: u32,
    pub flags: u32,
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub align: u64,
}

/// Representación parseada de un binario ELF listo para ser ejecutado
pub struct ElfBinary<'a> {
    pub data: &'a [u8],
    pub header: Elf64Header,
}

impl<'a> ElfBinary<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, &'static str> {
        if data.len() < core::mem::size_of::<Elf64Header>() {
            return Err("El archivo es demasiado pequeño para ser un binario ELF");
        }

        // SAFETY: Se verificó previamente que `data.len() >= size_of::<Elf64Header>()` y `read_unaligned` no exige alineación.
        let header = unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Elf64Header) };

        if &header.ident[0..4] != &ELF_MAGIC {
            return Err("Firma mágica inválida (No es un binario ELF)");
        }
        if header.ident[4] != ELF_CLASS_64 {
            return Err("Formato no soportado (Solo ELF64 de 64 bits)");
        }
        if header.ident[5] != ELF_DATA_2LSB {
            return Err("Orden de bytes no soportado (Se requiere Little-Endian)");
        }
        if header.machine != ELF_MACHINE_X86_64 {
            return Err("Arquitectura incompatible (Se requiere x86_64)");
        }

        Ok(Self { data, header })
    }

    /// Itera sobre las cabeceras de programas (segmentos cargables PT_LOAD)
    pub fn program_headers(&self) -> Vec<Elf64ProgramHeader> {
        let mut list = Vec::new();
        let ph_offset = self.header.ph_offset as usize;
        let ph_size = self.header.ph_entry_size as usize;
        let count = self.header.ph_num as usize;

        for i in 0..count {
            let offset = ph_offset + i * ph_size;
            if offset + ph_size <= self.data.len() {
                // SAFETY: `offset + ph_size <= self.data.len()` garantiza que la lectura no alineada de `Elf64ProgramHeader` está dentro del slice.
                let ph = unsafe {
                    core::ptr::read_unaligned(self.data.as_ptr().add(offset) as *const Elf64ProgramHeader)
                };
                list.push(ph);
            }
        }
        list
    }

    /// Carga el binario en memoria física dedicada y salta a Ring 3
    pub fn load_and_execute(&self) -> Result<(), &'static str> {
        let entry_rip = self.header.entry;
        let ph_count = self.header.ph_num;

        kprintln!("==================== CARGADOR DE BINARIOS ELF64 ====================");
        kprint_color!(VgaColor::LightCyan, VgaColor::Black, "  * Punto de Entrada (RIP): {:#010x}\n", entry_rip);
        kprint_color!(VgaColor::LightCyan, VgaColor::Black, "  * Cantidad Segmentos   : {}\n", ph_count);

        let segments = self.program_headers();
        let mut loaded_count = 0;

        for ph in segments {
            let seg_type = ph.seg_type;
            let flags = ph.flags;
            let file_offset = ph.offset as usize;
            let file_sz = ph.file_size as usize;
            let mem_sz = ph.mem_size as usize;
            let target_vaddr = ph.vaddr;

            if seg_type == PT_LOAD {
                kprintln!("  -> Segmento PT_LOAD en {:#010x} | File: {} B | Mem: {} B | Flags: 0x{:x}",
                    target_vaddr, file_sz, mem_sz, flags);

                if file_offset + file_sz > self.data.len() {
                    return Err("El segmento excede los límites del archivo ELF");
                }

                // Copiar el contenido del segmento en la dirección destino
                // SAFETY: Se validó `file_offset + file_sz <= self.data.len()` y `target_vaddr` apunta a la región de usuario mapeada para el ejecutable.
                unsafe {
                    let dest = target_vaddr as *mut u8;
                    core::ptr::copy_nonoverlapping(self.data.as_ptr().add(file_offset), dest, file_sz);
                    if mem_sz > file_sz {
                        core::ptr::write_bytes(dest.add(file_sz), 0, mem_sz - file_sz);
                    }
                }
                loaded_count += 1;
            }
        }

        if loaded_count == 0 {
            return Err("El archivo ELF no contiene segmentos cargables PT_LOAD");
        }

        // Asignar página de pila privada de usuario de 4 KiB en Ring 3
        let stack_frame = FRAME_ALLOCATOR.allocate_frame().ok_or("No hay marcos libres para la pila de usuario")?;
        let user_stack_top = (stack_frame + PAGE_SIZE) as u64;

        kprint_color!(
            VgaColor::LightGreen, VgaColor::Black,
            "[OK] Segmentos mapeados con éxito. Transfiriendo control a Ring 3...\n"
        );
        kprint_color!(
            VgaColor::Yellow, VgaColor::Black,
            "  Pila de Usuario Ring 3: {:#010x}\n\
             ====================================================================\n\n",
            user_stack_top
        );

        // Salto a Ring 3 mediante IRETQ
        jump_to_ring3(entry_rip, user_stack_top);
    }
}

/// Genera un binario ELF64 mínimo de prueba sintético en memoria
/// Programa que invoca `SYS_WRITE` ("Hello from Ring 3 ELF!") y `SYS_EXIT` (0)
pub fn generate_sample_elf_binary() -> Vec<u8> {
    let mut elf = Vec::new();

    // 1. Cabecera ELF64 (64 bytes)
    let header = Elf64Header {
        ident: [0x7F, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        elf_type: ELF_TYPE_EXEC,
        machine: ELF_MACHINE_X86_64,
        version: 1,
        entry: 0x400000 + 0x1000, // Punto de entrada en 0x401000
        ph_offset: 64,             // Inmediatamente tras la cabecera ELF
        sh_offset: 0,
        flags: 0,
        header_size: 64,
        ph_entry_size: 56,
        ph_num: 1,                 // 1 segmento PT_LOAD
        sh_entry_size: 0,
        sh_num: 0,
        sh_str_index: 0,
    };

    // SAFETY: `Elf64Header` es `#[repr(C, packed)]` de exactamente 64 bytes inicializados y alineación de `u8` es 1.
    let header_bytes = unsafe {
        core::slice::from_raw_parts(&header as *const Elf64Header as *const u8, 64)
    };
    elf.extend_from_slice(header_bytes);

    // 2. Program Header (56 bytes)
    let ph = Elf64ProgramHeader {
        seg_type: PT_LOAD,
        flags: PF_R | PF_W | PF_X,
        offset: 0x1000,            // Offset en archivo
        vaddr: 0x400000 + 0x1000,  // Dirección virtual destino (0x401000)
        paddr: 0x400000 + 0x1000,
        file_size: 32,             // 32 bytes de código máquina
        mem_size: 32,
        align: 0x1000,
    };

    // SAFETY: `Elf64ProgramHeader` es `#[repr(C, packed)]` de exactamente 56 bytes inicializados y alineación de `u8` es 1.
    let ph_bytes = unsafe {
        core::slice::from_raw_parts(&ph as *const Elf64ProgramHeader as *const u8, 56)
    };
    elf.extend_from_slice(ph_bytes);

    // Rellenar hasta el offset 0x1000 (4096 bytes)
    while elf.len() < 0x1000 {
        elf.push(0);
    }

    // 3. Código Máquina x86_64 de Ring 3:
    // mov rax, 0x01   (SYS_WRITE)
    // mov rdi, 0x00   (Mensaje 0)
    // syscall
    // mov rax, 0x03   (SYS_EXIT)
    // mov rdi, 0x00   (Código 0)
    // syscall
    let code: [u8; 38] = [
        0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1
        0x48, 0xC7, 0xC7, 0x00, 0x00, 0x00, 0x00, // mov rdi, 0
        0x0F, 0x05,                               // syscall
        0x48, 0xC7, 0xC0, 0x03, 0x00, 0x00, 0x00, // mov rax, 3
        0x48, 0xC7, 0xC7, 0x00, 0x00, 0x00, 0x00, // mov rdi, 0
        0x0F, 0x05,                               // syscall
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90,       // nop padding
    ];

    elf.extend_from_slice(&code);
    elf
}
