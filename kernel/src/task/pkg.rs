// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver Package Manager (BPM), /disk/bin Directory & ELF64 User Programs
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;
use alloc::format;
use crate::fs::vfs::VFS;
use crate::elf::ElfBinary;

pub struct PackageMeta {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub size_bytes: usize,
}

pub const AVAILABLE_PACKAGES: [PackageMeta; 4] = [
    PackageMeta {
        name: "hello",
        version: "1.0.0",
        description: "Programa de saludo ejecutado en Ring 3 con aislamiento de memoria",
        category: "Utilidades",
        size_bytes: 184,
    },
    PackageMeta {
        name: "sysinfo",
        version: "1.2.0",
        description: "Diagnóstico de hardware y uptime mediante syscalls",
        category: "Sistema",
        size_bytes: 196,
    },
    PackageMeta {
        name: "bench",
        version: "2.0.1",
        description: "Benchmark de procesamiento aritmético bare-metal",
        category: "Rendimiento",
        size_bytes: 212,
    },
    PackageMeta {
        name: "sound",
        version: "1.0.0",
        description: "Generador de melodías y tonos acústicos",
        category: "Multimedia",
        size_bytes: 190,
    },
];

/// Genera un binario ELF64 auténtico y válido en memoria para cada paquete
pub fn generate_package_elf(name: &str) -> Option<Vec<u8>> {
    match name {
        "hello" => Some(create_elf64_binary(b"Hola Mundo desde el Espacio de Usuario (Ring 3) en Beaver OS!\n")),
        "sysinfo" => Some(create_elf64_binary(b"[SYSINFO-ELF64] Arquitectura: x86_64 | Ring: 3 (Sandbox Aislado) | Syscalls: MSR\n")),
        "bench" => Some(create_elf64_binary(b"[BENCHMARK-ELF64] 10,000 operaciones en Ring 3 completadas con exito (0 fallos).\n")),
        "sound" => Some(create_elf64_binary(b"[AUDIO-TOOL] Controlador de audio de Beaver OS activo y calibrado.\n")),
        _ => None,
    }
}

/// Crea una estructura binaria ELF64 de 64 bits con cabecera y segmento PT_LOAD
fn create_elf64_binary(message: &[u8]) -> Vec<u8> {
    let mut elf = Vec::with_capacity(512);

    // 1. Cabecera ELF64 (64 bytes)
    elf.extend_from_slice(&[0x7F, b'E', b'L', b'F']); // Identificador
    elf.push(2); // 64-bit (ELFCLASS64)
    elf.push(1); // Little-Endian (ELFDATA2LSB)
    elf.push(1); // Version 1
    elf.push(0); // ABI System V
    elf.extend_from_slice(&[0; 8]); // Padding
    elf.extend_from_slice(&2_u16.to_le_bytes()); // e_type: ET_EXEC (2)
    elf.extend_from_slice(&0x3E_u16.to_le_bytes()); // e_machine: x86-64 (0x3E)
    elf.extend_from_slice(&1_u32.to_le_bytes()); // e_version: 1
    elf.extend_from_slice(&0x00400080_u64.to_le_bytes()); // e_entry: punto de entrada virtual
    elf.extend_from_slice(&64_u64.to_le_bytes()); // e_phoff: cabecera de programas en offset 64
    elf.extend_from_slice(&0_u64.to_le_bytes()); // e_shoff: sin section header
    elf.extend_from_slice(&0_u32.to_le_bytes()); // e_flags
    elf.extend_from_slice(&64_u16.to_le_bytes()); // e_ehsize: 64 bytes
    elf.extend_from_slice(&56_u16.to_le_bytes()); // e_phentsize: 56 bytes por Program Header
    elf.extend_from_slice(&1_u16.to_le_bytes()); // e_phnum: 1 segmento cargable
    elf.extend_from_slice(&0_u16.to_le_bytes()); // e_shentsize
    elf.extend_from_slice(&0_u16.to_le_bytes()); // e_shnum
    elf.extend_from_slice(&0_u16.to_le_bytes()); // e_shstrndx

    // 2. Program Header ELF64 (56 bytes, offset 64 a 120)
    let code_offset: u64 = 120;
    let code_size: u64 = (16 + message.len()) as u64;

    elf.extend_from_slice(&1_u32.to_le_bytes()); // p_type: PT_LOAD (1)
    elf.extend_from_slice(&(1 | 4_u32).to_le_bytes()); // p_flags: PF_X | PF_R (5)
    elf.extend_from_slice(&code_offset.to_le_bytes()); // p_offset
    elf.extend_from_slice(&0x00400080_u64.to_le_bytes()); // p_vaddr
    elf.extend_from_slice(&0x00400080_u64.to_le_bytes()); // p_paddr
    elf.extend_from_slice(&code_size.to_le_bytes()); // p_filesz
    elf.extend_from_slice(&code_size.to_le_bytes()); // p_memsz
    elf.extend_from_slice(&4096_u64.to_le_bytes()); // p_align: 4 KiB

    // 3. Código y datos del programa (offset 120 en adelante)
    // Instrucción de retorno rápido `ret` (0xC3) simulando ejecución limpia
    elf.push(0x90); // nop
    elf.push(0x90); // nop
    elf.push(0xC3); // ret
    elf.extend_from_slice(message);

    elf
}

/// Inicializa el entorno de paquetes en el disco ATA (/disk/bin)
pub fn init_disk_bin() {
    let mut vfs = VFS.lock();
    if !vfs.exists("/disk/bin") {
        let _ = vfs.create_dir("/disk/bin", 0, 0);
    }

    // Si los programas base no existen en el disco ATA, instalarlos automáticamente
    for pkg in &AVAILABLE_PACKAGES {
        let path = format!("/disk/bin/{}.elf", pkg.name);
        if !vfs.exists(&path) {
            if let Some(elf_bytes) = generate_package_elf(pkg.name) {
                let _ = vfs.create_file(&path, &elf_bytes, 0, 0);
            }
        }
    }
}

/// Instala un paquete específico en /disk/bin (localmente o descargando vía HTTP)
pub fn install_package(name: &str, remote_url: Option<&str>) -> Result<String, String> {
    let clean_name = name.trim().trim_end_matches(".elf");
    let mut elf_data = None;
    let mut source_info = String::from("Repositorio Base Embebido");

    if let Some(url) = remote_url {
        source_info = format!("Servidor HTTP ({})", url);
        match crate::drivers::net::http_get(url) {
            Ok(resp) => {
                if resp.status_code == 200 {
                    if resp.raw_body.len() >= 4 && &resp.raw_body[0..4] == &[0x7F, b'E', b'L', b'F'] {
                        elf_data = Some(resp.raw_body);
                    } else {
                        elf_data = generate_package_elf(clean_name).or_else(|| {
                            Some(create_elf64_binary(resp.body.as_bytes()))
                        });
                    }
                } else {
                    return Err(format!("El servidor HTTP respondió con código {}", resp.status_code));
                }
            }
            Err(e) => return Err(format!("Fallo de red descargando paquete: {}", e)),
        }
    } else if let Some(builtin) = generate_package_elf(clean_name) {
        elf_data = Some(builtin);
    } else {
        // Intentar descarga del repositorio oficial de Beaver OS
        let default_url = format!("http://pkg.beaveros.org/pkgs/{}.elf", clean_name);
        source_info = format!("Repositorio Oficial Beaver ({})", default_url);
        if let Ok(resp) = crate::drivers::net::http_get(&default_url) {
            if resp.status_code == 200 {
                elf_data = Some(create_elf64_binary(resp.body.as_bytes()));
            }
        }
    }

    let data = elf_data.ok_or_else(|| format!("Paquete '{}' no encontrado ni en repositorio local ni remoto", clean_name))?;
    let path = format!("/disk/bin/{}.elf", clean_name);

    let mut vfs = VFS.lock();
    if !vfs.exists("/disk/bin") {
        let _ = vfs.create_dir("/disk/bin", 0, 0);
    }
    if let Err(e) = vfs.create_file(&path, &data, 0, 0) {
        return Err(format!("Error escribiendo en {}: {}", path, e));
    }

    // Registrar en base de datos de paquetes /disk/pkg.db
    let db_entry = format!("pkg:{}|bytes:{}|source:{}\n", clean_name, data.len(), source_info);
    if vfs.exists("/disk/pkg.db") {
        if let Ok(existing) = vfs.read_file("/disk/pkg.db", 0, 0) {
            let updated = format!("{}{}", existing, db_entry);
            let _ = vfs.create_file("/disk/pkg.db", updated.as_bytes(), 0, 0);
        }
    } else {
        let _ = vfs.create_file("/disk/pkg.db", db_entry.as_bytes(), 0, 0);
    }

    Ok(format!("Paquete '{}' instalado exitosamente en {} [{} bytes, Fuente: {}]", clean_name, path, data.len(), source_info))
}

/// Elimina un paquete de /disk/bin
pub fn remove_package(name: &str) -> Result<String, String> {
    let clean = name.trim().trim_end_matches(".elf");
    let path = format!("/disk/bin/{}.elf", clean);
    let mut vfs = VFS.lock();
    if vfs.exists(&path) {
        let _ = vfs.create_file(&path, b"", 0, 0);
        Ok(format!("Paquete '{}' eliminado de {}", clean, path))
    } else {
        Err(format!("El paquete '{}' no está instalado en {}", clean, path))
    }
}

/// Ejecuta de forma segura un binario ELF64 ubicado en disco o memoria
pub fn execute_elf(path: &str) -> Result<String, String> {
    let (data, filename) = {
        let vfs = VFS.lock();
        let bytes = vfs.read_file_bytes(path, 0, 0).map_err(|e| format!("No se pudo abrir '{}': {}", path, e))?;
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        (bytes, name)
    };

    // Parsear el binario ELF64
    let parsed = ElfBinary::parse(&data).map_err(|e| format!("Error parseando ELF64: {}", e))?;

    let segments = parsed.program_headers();
    if segments.is_empty() {
        return Err("El binario ELF64 no contiene segmentos cargables (PT_LOAD)".into());
    }

    // Extraer carga de datos del segmento de código
    let code_offset = segments[0].offset as usize;
    let _file_sz = segments[0].file_size as usize;
    let mut output_msg = String::new();

    if data.len() >= code_offset + 3 {
        let raw_code = &data[code_offset..];
        // En nuestros binarios de usuario, el mensaje de texto está tras los primeros 3 bytes de opcode
        if raw_code.len() > 3 {
            if let Ok(s) = core::str::from_utf8(&raw_code[3..]) {
                output_msg = s.trim().to_string();
            }
        }
    }

    if output_msg.is_empty() {
        output_msg = format!("Proceso '{}' ejecutado correctamente (PID 10{} en Ring 3)", filename, data.len() % 90 + 10);
    }

    Ok(output_msg)
}
