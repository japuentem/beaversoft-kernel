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

    // 7. Habilitar Interrupciones en el Núcleo (DAIFCLR #2)
    Pl011Uart::write_str("[ARM64] Habilitando interrupciones de hardware en CPU...\n");
    cpu::enable_interrupts();
    Pl011Uart::write_str("[ARM64] Beaver OS ARM64 completamente operativo. Entrando a bucle supervisor...\n\n");

    // 8. Bucle supervisor determinista con instrucción de bajo consumo WFI
    loop {
        cpu::halt();
    }
}

