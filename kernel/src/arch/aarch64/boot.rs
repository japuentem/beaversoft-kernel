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

    // 8. Habilitar Interrupciones en el Núcleo (DAIFCLR #2)
    Pl011Uart::write_str("[ARM64] Habilitando interrupciones de hardware en CPU...\n");
    cpu::enable_interrupts();

    // 8.1 Verificación real de aislamiento EL0 + pasarela SVC #0
    const EL0_ARENA_BASE: u64 = 0x4200_0000;
    const EL0_STACK_TOP: u64 = 0x421F_F000;
    super::mmu::Aarch64Mmu::allow_el0_2mb_block(EL0_ARENA_BASE);
    let el0_prog: [u32; 6] = [
        0xD280_04E8, // mov x8, #39 (SYS_GETPID)
        0xD400_0001, // svc #0
        0x9100_A800, // add x0, x0, #42
        0xD280_0788, // mov x8, #60 (SYS_EXIT)
        0xD400_0001, // svc #0
        0x1400_0000, // b .
    ];
    // SAFETY: `EL0_ARENA_BASE` (0x4200_0000) está mapeado en RAM con PTE_USER, fuera de kernel y heap.
    unsafe {
        let dst = EL0_ARENA_BASE as *mut u32;
        for (i, &insn) in el0_prog.iter().enumerate() {
            core::ptr::write_volatile(dst.add(i), insn);
        }
        core::arch::asm!("dsb ish", "ic iallu", "dsb ish", "isb", options(nomem, nostack));
        let exit_code = crate::syscall::enter_ring3_with_return(EL0_ARENA_BASE, EL0_STACK_TOP);
        if exit_code == 42 {
            Pl011Uart::write_str("[ARM64] Prueba EL0 + SVC #0 (SYS_GETPID + SYS_EXIT=42) [OK]\n");
        } else {
            Pl011Uart::write_str("[ARM64] Prueba EL0 retorno codigo inesperado.\n");
        }
    }

    Pl011Uart::write_str("[ARM64] Beaver OS ARM64 completamente operativo. Iniciando Beaver Shell...\n\n");

    crate::shell::SHELL.lock().prompt();

    // 9. Bucle interactivo UART PL011 + ahorro determinista WFI
    loop {
        while let Some(byte) = Pl011Uart::read_byte() {
            let ch = if byte == b'\r' { '\n' } else { byte as char };
            crate::shell::SHELL.lock().handle_char(ch);
        }
        cpu::halt();
    }
}

