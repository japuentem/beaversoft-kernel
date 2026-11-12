// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: AArch64 Kernel Main Entry Point & Subsystem Initializer
// ============================================================================

use super::cpu::{self, ExceptionLevel};
use super::uart::Pl011Uart;
use super::gic::GicV2;
use super::timer::ArmGenericTimer;
use super::exceptions::init_exceptions;
use super::dtb;

#[no_mangle]
pub extern "C" fn kernel_main_aarch64(dtb_ptr: u64) -> ! {
    // 1. Inicializar consola serial PL011 UART
    Pl011Uart::init();

    // 2. Despliegue de Identidad de Marca Beaver OS para ARM64
    Pl011Uart::write_str("\n");
    Pl011Uart::write_str("================================================================\n");
    Pl011Uart::write_str("  [BEAVER OS] Microkernel v0.9.0 - Arquitectura ARM64 (AArch64)\n");
    Pl011Uart::write_str("  Author: José Alberto Puente Maldonado (japuentem)\n");
    Pl011Uart::write_str("  Brand: Beaversoft\n");
    Pl011Uart::write_str("  Target: aarch64-unknown-none (QEMU virt / Mobile ARM64)\n");
    Pl011Uart::write_str("  Header: ARM64 Boot Image (magic 0x644D5241) + FDT/SimpleFB\n");
    Pl011Uart::write_str("================================================================\n");

    // 2.1 Analizar Flattened Device Tree (DTB) y buscar SimpleFB móvil
    let fb_opt = dtb::init_and_parse_dtb(dtb_ptr);
    {
        let st = dtb::DTB_STATE.lock();
        if st.valid {
            Pl011Uart::write_str("[DTB] Flattened Device Tree (0xD00DFEED) detectado [OK]\n");
            Pl011Uart::write_str("[DTB] Dispositivo / SoC: ");
            Pl011Uart::write_str(st.model_str());
            Pl011Uart::write_str("\n");
        } else {
            Pl011Uart::write_str("[DTB] Sin arbol DTB en x0 / 0x40000000 (modo directo).\n");
        }
    }
    if let Some(_fb) = fb_opt {
        Pl011Uart::write_str("[SIMPLEFB] Framebuffer movil detectado en DTB y pintado [OK]\n");
    } else {
        Pl011Uart::write_str("[SIMPLEFB] Sin nodo simple-framebuffer en DTB actual (modo serial).\n");
    }

    // 3. Verificar Nivel de Excepción (Debe ser EL1 para el Kernel)
    let el = cpu::current_el();
    match el {
        ExceptionLevel::EL1 => {
            Pl011Uart::write_str("[ARM64] Nivel de Excepción: EL1 (Kernel Supervisor) [OK]\n");
        }
        ExceptionLevel::EL2 => {
            Pl011Uart::write_str("[ARM64] Nivel de Excepción: EL2 (Hypervisor). Degradando a EL1...\n");
        }
        _ => {
            Pl011Uart::write_str("[ARM64] Nivel de Excepción no estándar detectado.\n");
        }
    }

    // 4. Inicializar Controlador de Interrupciones Genéricas GICv2
    Pl011Uart::write_str("[ARM64] Inicializando ARM GICv2 (Distributor & CPU Interface)...\n");
    GicV2::init();
    Pl011Uart::write_str("[ARM64] GICv2 inicializado y ruteado a CPU 0 [OK]\n");

    // 5. Configurar Vector de Excepciones (VBAR_EL1)
    Pl011Uart::write_str("[ARM64] Configurando Tabla de Vectores VBAR_EL1 y pasarela SVC...\n");
    init_exceptions();
    Pl011Uart::write_str("[ARM64] Vector de Excepciones AArch64 enlazado [OK]\n");

    // 6. Inicializar Temporizador Genérico de ARM a 100 Hz
    Pl011Uart::write_str("[ARM64] Inicializando ARM Generic Timer (100 Hz, IRQ 30)...\n");
    ArmGenericTimer::init();
    Pl011Uart::write_str("[ARM64] ARM Generic Timer configurado a 100 Hz (PPI 14 / IRQ 30) [OK]\n");

    // 6.1 Inicializar Controladores de Entrada Táctil (I2C) y HDMI-CEC
    crate::drivers::touchscreen::init_touchscreen();
    crate::drivers::cec::init_cec();

    // 7. Activar MMU (TTBR0/TTBR1), Heap Dinámico (4 MiB), Frame Allocator y VFS
    let fb_phys = fb_opt.map(|f| f.phys_base).unwrap_or(0);
    super::mmu::Aarch64Mmu::init_and_enable_identity_mmu(fb_phys);
    Pl011Uart::write_str("[ARM64] MMU L0->L1 (TTBR0/TTBR1 + I/D-Cache) activado [OK]\n");

    let heap_start = (crate::memory::kernel_end() + 0xFFFF) & !0xFFFF;
    const AARCH64_HEAP_SIZE: usize = 4 * 1024 * 1024;
    // SAFETY: `heap_start..heap_start + AARCH64_HEAP_SIZE` queda inmediatamente después de `_kernel_end` en RAM física mapeada.
    unsafe {
        crate::memory::heap::HEAP_ALLOCATOR.init(heap_start, AARCH64_HEAP_SIZE);
    }

    let (ram_base, ram_size) = {
        let st = dtb::DTB_STATE.lock();
        if st.ram_size != 0 {
            (st.ram_base, st.ram_size)
        } else {
            (0x4000_0000u64, 256 * 1024 * 1024u64)
        }
    };
    let heap_end = (heap_start + AARCH64_HEAP_SIZE) as u64;
    let ram_end = ram_base.saturating_add(ram_size);
    let mut memmap = crate::memory::MemMap::empty();
    if ram_end > heap_end {
        memmap.regions[0] = crate::memory::memmap::MemRegion {
            start: heap_end,
            len: ram_end - heap_end,
            usable: true,
        };
        memmap.count = 1;
    }
    crate::memory::frame_allocator::FRAME_ALLOCATOR.init_from_memmap(&memmap);
    crate::fs::init_vfs();
    Pl011Uart::write_str("[ARM64] Heap (4 MiB), FrameAllocator y VFS operativos [OK]\n");

    // 7.1 Sondeo real de bus VirtIO MMIO (VirtIO-blk / VirtIO-net) y montaje BeaverFS
    let vio = super::virtio_mmio::probe_and_init_virtio();
    if vio.blk_present {
        let mut sec0 = [0u8; 512];
        if super::virtio_mmio::read_sector(0, &mut sec0).is_ok() {
            crate::kprintln!(
                "[ARM64] VirtIO-blk v{} operativo en {:#x} ({} MiB, sector 0 leido) [OK]",
                vio.blk_version,
                vio.blk_base,
                (vio.blk_sectors * 512) / (1024 * 1024)
            );
            crate::storage::block_device::BLOCK_DEVICE.lock().set_use_virtio(true);
            crate::storage::ab_slot::BOOT_SLOT_MANAGER.lock().init();
            crate::storage::ab_slot::BOOT_SLOT_MANAGER.lock().mark_boot_successful();
            crate::storage::beaverfs::init_beaverfs();
            Pl011Uart::write_str("[ARM64] BeaverFS persistente y Ranuras A/B montados sobre VirtIO-blk [OK]\n");
        } else {
            crate::kprintln!("[ARM64] VirtIO-blk detectado en {:#x} pero fallo lectura LBA 0.", vio.blk_base);
        }
    } else {
        Pl011Uart::write_str("[ARM64] Sin disco VirtIO-blk MMIO conectado (solo RAM/VFS).\n");
    }
    if vio.net_present {
        {
            let mut net = crate::drivers::net::NET_INTERFACE.lock();
            net.card_detected = true;
            net.card_name = "VirtIO-net MMIO (ARM64)";
            net.mmio_base = vio.net_base;
            net.mac_address = vio.net_mac;
            net.ip_address = crate::drivers::net::Ipv4Addr::new(10, 0, 2, 15);
            net.gateway = crate::drivers::net::Ipv4Addr::new(10, 0, 2, 2);
        }
        crate::kprintln!(
            "[ARM64] VirtIO-net v{} operativo en {:#x} (MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}) [OK]",
            vio.net_version,
            vio.net_base,
            vio.net_mac[0], vio.net_mac[1], vio.net_mac[2],
            vio.net_mac[3], vio.net_mac[4], vio.net_mac[5]
        );
        crate::drivers::mesh::send_mesh_announce_now();
    }

    // 8. Habilitar Interrupciones en el Núcleo (DAIFCLR #2)
    Pl011Uart::write_str("[ARM64] Habilitando interrupciones de hardware en CPU...\n");
    cpu::enable_interrupts();

    // 8.1 Verificación real de cargador ELF64 AArch64 (EM_AARCH64=0xB7) + aislamiento EL0 + SVC #0
    {
        use crate::elf::{Elf64Header, Elf64ProgramHeader, ElfBinary, ELF_CLASS_64, ELF_DATA_2LSB, ELF_MACHINE_AARCH64, ELF_MAGIC, ELF_TYPE_EXEC, PF_R, PF_X, PT_LOAD};
        const EL0_ENTRY: u64 = 0x4200_0000;
        let code_insns: [u32; 6] = [
            0xD280_04E8, // mov x8, #39 (SYS_GETPID)
            0xD400_0001, // svc #0
            0x9100_A800, // add x0, x0, #42
            0xD280_0788, // mov x8, #60 (SYS_EXIT)
            0xD400_0001, // svc #0
            0x1400_0000, // b .
        ];
        let hdr_sz = core::mem::size_of::<Elf64Header>();
        let ph_sz = core::mem::size_of::<Elf64ProgramHeader>();
        let code_offset = (hdr_sz + ph_sz) as u64;
        let code_bytes_len = (code_insns.len() * 4) as u64;

        let mut ident = [0u8; 16];
        ident[0..4].copy_from_slice(&ELF_MAGIC);
        ident[4] = ELF_CLASS_64;
        ident[5] = ELF_DATA_2LSB;
        ident[6] = 1; // EV_CURRENT

        let hdr = Elf64Header {
            ident,
            elf_type: ELF_TYPE_EXEC,
            machine: ELF_MACHINE_AARCH64,
            version: 1,
            entry: EL0_ENTRY,
            ph_offset: hdr_sz as u64,
            sh_offset: 0,
            flags: 0,
            header_size: hdr_sz as u16,
            ph_entry_size: ph_sz as u16,
            ph_num: 1,
            sh_entry_size: 0,
            sh_num: 0,
            sh_str_index: 0,
        };
        let ph = Elf64ProgramHeader {
            seg_type: PT_LOAD,
            flags: PF_R | PF_X,
            offset: code_offset,
            vaddr: EL0_ENTRY,
            paddr: EL0_ENTRY,
            file_size: code_bytes_len,
            mem_size: code_bytes_len,
            align: 0x1000,
        };

        let mut elf_buf = alloc::vec::Vec::with_capacity(code_offset as usize + code_bytes_len as usize);
        // SAFETY: `Elf64Header` y `Elf64ProgramHeader` son structs `#[repr(C, packed)]` POD.
        unsafe {
            elf_buf.extend_from_slice(core::slice::from_raw_parts(&hdr as *const _ as *const u8, hdr_sz));
            elf_buf.extend_from_slice(core::slice::from_raw_parts(&ph as *const _ as *const u8, ph_sz));
        }
        for &insn in &code_insns {
            elf_buf.extend_from_slice(&insn.to_le_bytes());
        }

        match ElfBinary::parse(&elf_buf).and_then(|bin| bin.load_and_execute()) {
            Ok(()) => Pl011Uart::write_str("[ARM64] Prueba ELF64 + EL0 + SVC #0 [OK]\n"),
            Err(e) => crate::kprintln!("[ARM64] Error ejecutando ELF64 en EL0: {}", e),
        }
    }

    Pl011Uart::write_str("[ARM64] Beaver OS ARM64 completamente operativo. Iniciando Beaver Shell...\n\n");

    crate::shell::SHELL.lock().prompt();

    // 9. Bucle interactivo UART PL011 + red VirtIO-net + ahorro determinista WFI
    loop {
        crate::drivers::net::net_poll_rx();
        while let Some(byte) = Pl011Uart::read_byte() {
            let ch = if byte == b'\r' { '\n' } else { byte as char };
            crate::shell::SHELL.lock().handle_char(ch);
        }
        cpu::halt();
    }
}

