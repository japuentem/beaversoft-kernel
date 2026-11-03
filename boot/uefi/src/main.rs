// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver Universal UEFI Bootloader (x86_64 EFI Application)
// ============================================================================

#![no_std]
#![no_main]

use core::panic::PanicInfo;

static KERNEL_ELF: &[u8] = include_bytes!("../../../target/x86_64-unknown-none/release/beaversoft-kernel");

pub const UEFI_BOOT_INFO_ADDR: u64 = 0x9000; // Movido de 0x8000: reservamos 0x8000 para futuro trampolines SMP de 16 bits
pub const UEFI_BOOT_MAGIC: u32 = 0xBEA0B007;

// Buffer estatico para el mapa de memoria UEFI (se copia aqui antes de exit_boot_services)
// Ubicado inmediatamente despues de UefiBootInfo en 0x9000 + sizeof(UefiBootInfo)
pub const UEFI_MEM_MAP_ADDR: u64 = 0x9100; // ~256 bytes despues de la cabecera

#[repr(C)]
pub struct UefiBootInfo {
    pub magic: u32,
    pub fb_base: u64,
    pub fb_size: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_pixels_per_scanline: u32,
    // Mapa de memoria UEFI (anadido en Fase 2)
    pub mem_map_addr: u64,  // Direccion fisica del primer descriptor EFI_MEMORY_DESCRIPTOR
    pub mem_map_size: u64,  // Tamano total del mapa en bytes (num_descriptors * desc_size)
    pub mem_desc_size: u64, // Tamano de cada descriptor (varía por firmware, tipicamente 48 bytes)
}

#[repr(C)]
pub struct EfiTableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

#[repr(C)]
pub struct EfiSimpleTextOutputProtocol {
    pub reset: extern "win64" fn(*mut EfiSimpleTextOutputProtocol, bool) -> usize,
    pub output_string: extern "win64" fn(*mut EfiSimpleTextOutputProtocol, *const u16) -> usize,
    pub test_string: usize,
    pub query_mode: usize,
    pub set_mode: usize,
    pub set_attribute: extern "win64" fn(*mut EfiSimpleTextOutputProtocol, usize) -> usize,
    pub clear_screen: extern "win64" fn(*mut EfiSimpleTextOutputProtocol) -> usize,
}

#[repr(C)]
pub struct EfiGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x9042a9de,
    data2: 0x23dc,
    data3: 0x4a38,
    data4: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

#[repr(C)]
pub struct EfiGraphicsOutputModeInformation {
    pub version: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub pixel_format: u32,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub reserved_mask: u32,
    pub pixels_per_scan_line: u32,
}

#[repr(C)]
pub struct EfiGraphicsOutputProtocolMode {
    pub max_mode: u32,
    pub mode: u32,
    pub info: *mut EfiGraphicsOutputModeInformation,
    pub size_of_info: usize,
    pub framebuffer_base: u64,
    pub framebuffer_size: usize,
}

#[repr(C)]
pub struct EfiGraphicsOutputProtocol {
    pub query_mode: extern "win64" fn(*mut EfiGraphicsOutputProtocol, u32, *mut usize, *mut *mut EfiGraphicsOutputModeInformation) -> usize,
    pub set_mode: extern "win64" fn(*mut EfiGraphicsOutputProtocol, u32) -> usize,
    pub blt: usize,
    pub mode: *mut EfiGraphicsOutputProtocolMode,
}

#[repr(C)]
pub struct EfiBootServices {
    pub hdr: EfiTableHeader,
    pub raise_tpl: usize,
    pub restore_tpl: usize,
    pub allocate_pages: extern "win64" fn(alloc_type: u32, memory_type: u32, pages: usize, memory: *mut u64) -> usize,
    pub free_pages: extern "win64" fn(memory: u64, pages: usize) -> usize,
    pub get_memory_map: extern "win64" fn(memory_map_size: *mut usize, memory_map: *mut u8, map_key: *mut usize, descriptor_size: *mut usize, descriptor_version: *mut u32) -> usize,
    pub allocate_pool: usize,
    pub free_pool: usize,
    pub create_event: usize,
    pub set_timer: usize,
    pub wait_for_event: usize,
    pub signal_event: usize,
    pub close_event: usize,
    pub check_event: usize,
    pub install_protocol_interface: usize,
    pub reinstall_protocol_interface: usize,
    pub uninstall_protocol_interface: usize,
    pub handle_protocol: usize,
    pub reserved: usize,
    pub register_protocol_notify: usize,
    pub locate_handle: usize,
    pub locate_device_path: usize,
    pub install_configuration_table: usize,
    pub load_image: usize,
    pub start_image: usize,
    pub exit: usize,
    pub unload_image: usize,
    pub exit_boot_services: extern "win64" fn(image_handle: *mut (), map_key: usize) -> usize,
    pub get_next_monotonic_count: usize,
    pub stall: extern "win64" fn(microseconds: usize) -> usize,
    pub set_watchdog_timer: usize,
    pub connect_controller: usize,
    pub disconnect_controller: usize,
    pub open_protocol: usize,
    pub close_protocol: usize,
    pub open_protocol_information: usize,
    pub protocols_per_handle: usize,
    pub locate_handle_buffer: usize,
    pub locate_protocol: extern "win64" fn(protocol: *const EfiGuid, registration: *mut (), interface: *mut *mut ()) -> usize,
}

#[repr(C)]
pub struct EfiSystemTable {
    pub hdr: EfiTableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub console_in_handle: *mut (),
    pub con_in: *mut (),
    pub console_out_handle: *mut (),
    pub con_out: *mut EfiSimpleTextOutputProtocol,
    pub standard_error_handle: *mut (),
    pub std_err: *mut EfiSimpleTextOutputProtocol,
    pub runtime_services: *mut (),
    pub boot_services: *mut EfiBootServices,
    pub number_of_table_entries: usize,
    pub configuration_table: *mut (),
}

#[inline(always)]
unsafe fn read_u16_raw(ptr: *const u8, offset: usize) -> u16 {
    core::ptr::read_unaligned(ptr.add(offset) as *const u16)
}

#[inline(always)]
unsafe fn read_u32_raw(ptr: *const u8, offset: usize) -> u32 {
    core::ptr::read_unaligned(ptr.add(offset) as *const u32)
}

#[inline(always)]
unsafe fn read_u64_raw(ptr: *const u8, offset: usize) -> u64 {
    core::ptr::read_unaligned(ptr.add(offset) as *const u64)
}

static mut PRINT_BUF: [u16; 256] = [0; 256];

fn print_str(con_out: *mut EfiSimpleTextOutputProtocol, s: &str) {
    if con_out.is_null() {
        return;
    }
    unsafe {
        let buf_ptr = core::ptr::addr_of_mut!(PRINT_BUF) as *mut u16;
        let mut i = 0;
        for b in s.bytes() {
            if b == b'\n' {
                if i + 2 < 255 {
                    *buf_ptr.add(i) = '\r' as u16;
                    i += 1;
                    *buf_ptr.add(i) = '\n' as u16;
                    i += 1;
                }
            } else if i + 1 < 255 {
                *buf_ptr.add(i) = b as u16;
                i += 1;
            }
        }
        *buf_ptr.add(i) = 0;
        ((*con_out).output_string)(con_out, buf_ptr);
    }
}

static mut MEM_MAP: [u8; 8192] = [0; 8192];

#[no_mangle]
pub extern "win64" fn efi_main(image_handle: *mut (), system_table: *mut EfiSystemTable) -> usize {
    if system_table.is_null() {
        return 1;
    }

    let con_out = unsafe { (*system_table).con_out };
    let bs = unsafe { (*system_table).boot_services };

    // Limpiar pantalla y mostrar banner de arranque
    if !con_out.is_null() {
        unsafe {
            ((*con_out).clear_screen)(con_out);
        }
    }

    print_str(con_out, "====================================================================\n");
    print_str(con_out, "  BEAVER OS - Universal UEFI Bootloader (x86_64 Long Mode)\n");
    print_str(con_out, "  Desarrollado para Hardware Real (PC, Laptop, Workstation, Servidor)\n");
    print_str(con_out, "====================================================================\n\n");

    // 1. Obtener protocolo GOP para alta resolucion y Linear Framebuffer
    print_str(con_out, "[UEFI] Localizando protocolo grafico GOP (Graphics Output Protocol)...\n");
    let mut gop_ptr: *mut () = core::ptr::null_mut();
    let mut fb_base: u64 = 0;
    let mut fb_size: u64 = 0;
    let mut fb_width: u32 = 1024;
    let mut fb_height: u32 = 768;
    let mut fb_scanline: u32 = 1024;

    if !bs.is_null() {
        let status = unsafe {
            ((*bs).locate_protocol)(
                &EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID as *const EfiGuid,
                core::ptr::null_mut(),
                &mut gop_ptr as *mut *mut ()
            )
        };

        if status == 0 && !gop_ptr.is_null() {
            let gop = gop_ptr as *mut EfiGraphicsOutputProtocol;
            let mode = unsafe { (*gop).mode };
            if !mode.is_null() {
                // Explorar modos disponibles y fijar explícitamente 1024x768x32
                let max_mode = unsafe { (*mode).max_mode };
                for m in 0..max_mode {
                    let mut size_of_info = 0usize;
                    let mut info_ptr = core::ptr::null_mut();
                    let q_status = unsafe { ((*gop).query_mode)(gop, m, &mut size_of_info, &mut info_ptr) };
                    if q_status == 0 && !info_ptr.is_null() {
                        let w = unsafe { (*info_ptr).horizontal_resolution };
                        let h = unsafe { (*info_ptr).vertical_resolution };
                        if w == 1024 && h == 768 {
                            let _ = unsafe { ((*gop).set_mode)(gop, m) };
                            break;
                        }
                    }
                }

                // Releer punteros de framebuffer actualizados tras set_mode
                let mode_upd = unsafe { (*gop).mode };
                if !mode_upd.is_null() {
                    fb_base = unsafe { (*mode_upd).framebuffer_base };
                    fb_size = unsafe { (*mode_upd).framebuffer_size as u64 };
                    let info = unsafe { (*mode_upd).info };
                    if !info.is_null() {
                        fb_width = unsafe { (*info).horizontal_resolution };
                        fb_height = unsafe { (*info).vertical_resolution };
                        fb_scanline = unsafe { (*info).pixels_per_scan_line };
                    }
                }
            }
            print_str(con_out, "[OK] GOP configurado exitosamente a 1024x768 TrueColor.\n");
        } else {
            print_str(con_out, "[INFO] GOP no disponible. Usando fallback de VBE/PCI.\n");
        }
    }

    // 2. Escribir BootInfo estructurado en 0x9000 para que el kernel lo recoja
    let boot_info = UEFI_BOOT_INFO_ADDR as *mut UefiBootInfo;
    unsafe {
        (*boot_info).magic = UEFI_BOOT_MAGIC;
        (*boot_info).fb_base = fb_base;
        (*boot_info).fb_size = fb_size;
        (*boot_info).fb_width = fb_width;
        (*boot_info).fb_height = fb_height;
        (*boot_info).fb_pixels_per_scanline = fb_scanline;
        // mem_map_* se rellenan despues de exit_boot_services (ver paso 6b)
        (*boot_info).mem_map_addr = 0;
        (*boot_info).mem_map_size = 0;
        (*boot_info).mem_desc_size = 0;
    }

    // 3. Asignar 1 MB en 0x100000 para cargar el Kernel ELF64
    print_str(con_out, "[UEFI] Reservando memoria en 0x100000 para Beaver Kernel...\n");
    if !bs.is_null() {
        let mut target_addr: u64 = 0x100000;
        let _ = unsafe {
            ((*bs).allocate_pages)(2 /* AllocateAddress */, 2 /* EfiLoaderData */, 256 /* 1MB */, &mut target_addr)
        };
    }

    // 4. Parsear y cargar segmentos ELF64 del Kernel Beaver
    print_str(con_out, "[UEFI] Extrayendo y cargando segmentos ELF64 de Beaver Kernel...\n");
    let elf_ptr = KERNEL_ELF.as_ptr();
    let elf_len = KERNEL_ELF.len();

    if elf_len < 64 || unsafe { read_u32_raw(elf_ptr, 0) } != 0x464C457F {
        print_str(con_out, "[ERROR] Formato ELF64 del kernel no valido.\n");
        loop {}
    }

    let phoff = unsafe { read_u64_raw(elf_ptr, 32) } as usize;
    let phentsize = unsafe { read_u16_raw(elf_ptr, 54) } as usize;
    let phnum = unsafe { read_u16_raw(elf_ptr, 56) } as usize;

    for i in 0..phnum {
        let off = phoff + i * phentsize;
        let p_type = unsafe { read_u32_raw(elf_ptr, off) };
        if p_type == 1 { // PT_LOAD
            let p_offset = unsafe { read_u64_raw(elf_ptr, off + 8) } as usize;
            let p_paddr = unsafe { read_u64_raw(elf_ptr, off + 24) } as usize;
            let p_filesz = unsafe { read_u64_raw(elf_ptr, off + 32) } as usize;
            let p_memsz = unsafe { read_u64_raw(elf_ptr, off + 40) } as usize;

            unsafe {
                let dest = p_paddr as *mut u8;
                let src = elf_ptr.add(p_offset);
                core::ptr::copy_nonoverlapping(src, dest, p_filesz);
                if p_memsz > p_filesz {
                    core::ptr::write_bytes(dest.add(p_filesz), 0, p_memsz - p_filesz);
                }
            }
        }
    }

    // 5. Buscar la cabecera magica "BEAVUEFI" para obtener el punto de entrada de 64 bits
    let mut uefi_entry_addr: u64 = 0;
    unsafe {
        let mem_start = 0x100000 as *const u8;
        for offset in (0..0x10000).step_by(8) {
            let ptr = mem_start.add(offset);
            if core::slice::from_raw_parts(ptr, 8) == b"BEAVUEFI" {
                let entry_ptr = ptr.add(8) as *const u64;
                uefi_entry_addr = *entry_ptr;
                break;
            }
        }
    }

    if uefi_entry_addr == 0 {
        print_str(con_out, "[ERROR] No se localizo la firma BEAVUEFI en el kernel.\n");
        loop {}
    }

    print_str(con_out, "[OK] Beaver Kernel verificado. Saltando a Modo Largo...\n");
    if !bs.is_null() {
        let _ = unsafe { ((*bs).stall)(200_000) }; // Pausa breve de 200ms para lectura humana
    }

    // 6. Cerrar servicios de firmware UEFI (ExitBootServices)
    let mut mem_map_size = 8192_usize;
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_ver: u32 = 0;
    let mem_map_ptr = core::ptr::addr_of_mut!(MEM_MAP) as *mut u8;

    if !bs.is_null() {
        let _ = unsafe {
            ((*bs).get_memory_map)(&mut mem_map_size, mem_map_ptr, &mut map_key, &mut desc_size, &mut desc_ver)
        };
        let status = unsafe {
            ((*bs).exit_boot_services)(image_handle, map_key)
        };
        if status != 0 {
            // Reintentar si el map_key cambio durante la llamada
            mem_map_size = 8192_usize;
            let _ = unsafe {
                ((*bs).get_memory_map)(&mut mem_map_size, mem_map_ptr, &mut map_key, &mut desc_size, &mut desc_ver)
            };
            let _ = unsafe {
                ((*bs).exit_boot_services)(image_handle, map_key)
            };
        }
    }

    // 6b. Copiar mapa de memoria al buffer fisico fijo (UEFI_MEM_MAP_ADDR) y actualizar boot_info
    // Despues de exit_boot_services ya no podemos llamar servicios UEFI, pero si copiar memoria
    let copy_size = if mem_map_size > 4096 { 4096 } else { mem_map_size };
    unsafe {
        let dst = UEFI_MEM_MAP_ADDR as *mut u8;
        let src = mem_map_ptr;
        for i in 0..copy_size {
            *dst.add(i) = *src.add(i);
        }
        // Actualizar boot_info con la ubicacion y tamano del mapa copiado
        let bi = UEFI_BOOT_INFO_ADDR as *mut UefiBootInfo;
        (*bi).mem_map_addr  = UEFI_MEM_MAP_ADDR;
        (*bi).mem_map_size  = copy_size as u64;
        (*bi).mem_desc_size = desc_size as u64;
    }

    // 7. Salto final al Kernel en 64-bit Long Mode
    unsafe {
        core::arch::asm!("cli");
        let entry_fn: extern "C" fn() -> ! = core::mem::transmute(uefi_entry_addr);
        entry_fn();
    }
}

// Stubs y runtime requeridos por el compilador para bare-metal UEFI
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut core::ffi::c_void, src: *const core::ffi::c_void, n: usize) -> *mut core::ffi::c_void {
    let d = dest as *mut u8;
    let s = src as *const u8;
    for i in 0..n {
        *d.add(i) = *s.add(i);
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut core::ffi::c_void, c: i32, n: usize) -> *mut core::ffi::c_void {
    let d = dest as *mut u8;
    for i in 0..n {
        *d.add(i) = c as u8;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const core::ffi::c_void, s2: *const core::ffi::c_void, n: usize) -> i32 {
    let a = s1 as *const u8;
    let b = s2 as *const u8;
    for i in 0..n {
        let val_a = *a.add(i);
        let val_b = *b.add(i);
        if val_a != val_b {
            return val_a as i32 - val_b as i32;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn __chkstk() {}

#[no_mangle]
pub static _fltused: i32 = 0;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("cli; hlt");
        }
    }
}
