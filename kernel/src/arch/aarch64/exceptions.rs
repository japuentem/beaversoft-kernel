// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: AArch64 Exception Vector Table (VBAR_EL1), IRQ Router & SVC Syscall Gateway
// ============================================================================

use core::arch::asm;

#[repr(C)]
#[derive(Debug, Default)]
pub struct TrapFrame {
    pub x0: u64,
    pub x1: u64,
    pub x2: u64,
    pub x3: u64,
    pub x4: u64,
    pub x5: u64,
    pub x6: u64,
    pub x7: u64,
    pub x8: u64,
    pub x9: u64,
    pub x10: u64,
    pub x11: u64,
    pub x12: u64,
    pub x13: u64,
    pub x14: u64,
    pub x15: u64,
    pub x16: u64,
    pub x17: u64,
    pub x18: u64,
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64, // Frame Pointer
    pub x30: u64, // Link Register
    pub sp_el0: u64,
    pub elr_el1: u64, // Return PC
    pub spsr_el1: u64, // Saved Processor State
}

/// Inicializa el vector de excepciones cargando su dirección base en VBAR_EL1
pub fn init_exceptions() {
    extern "C" {
        static exception_vector_table: [u8; 0];
    }
    unsafe {
        let vbar = exception_vector_table.as_ptr() as u64;
        asm!("msr vbar_el1, {}", in(reg) vbar, options(nomem, nostack));
        asm!("isb", options(nomem, nostack));
    }
}

/// Enrutador principal de interrupciones de hardware (IRQ) para AArch64
#[no_mangle]
pub extern "C" fn aarch64_handle_irq(frame: &mut TrapFrame) {
    let irq_id = super::gic::GicV2::acknowledge_irq();

    if irq_id == 30 {
        // Interrupción del Temporizador Genérico (PPI 14)
        super::timer::ArmGenericTimer::on_irq();
    } else if irq_id == 33 {
        // Interrupción de datos entrantes por PL011 UART (SPI 1)
        if let Some(ch) = super::uart::Pl011Uart::read_byte() {
            let _ = ch;
        }
    }

    super::gic::GicV2::end_of_irq(irq_id);
}

/// Enrutador de llamadas al sistema (SVC #0) provenientes de espacio de usuario (EL0 / Ring 3)
#[no_mangle]
pub extern "C" fn aarch64_handle_sync(frame: &mut TrapFrame) {
    let esr: u64;
    unsafe {
        asm!("mrs {}, esr_el1", out(reg) esr, options(nomem, nostack));
    }

    let ec = (esr >> 26) & 0x3F; // Exception Class

    if ec == 0x15 {
        // 0x15: SVC instruction execution in AArch64 state
        let sys_no = frame.x8; // En estándar ARM64 ABI, el número de syscall viaja en x8
        let arg1 = frame.x0;
        let arg2 = frame.x1;
        let arg3 = frame.x2;

        frame.x0 = crate::syscall::syscall_dispatch(sys_no, arg1, arg2, arg3);
    } else {
        // Error de memoria o aborto de instrucción: Supervisor Anti-BSOD destruye el proceso de usuario
        super::uart::Pl011Uart::write_str("[ARM64 SUPERVISOR] Excepción sincrónica capturada en EL0/EL1.\n");
    }
}

// Tabla de vectores de excepción alineada a 2048 bytes (2^11) según especificación ARMv8-A
core::arch::global_asm!(r#"
.section .text
.align 11
.global exception_vector_table
exception_vector_table:
    // Current EL with SP0
    .align 7; b default_handler // Synchronous
    .align 7; b default_handler // IRQ
    .align 7; b default_handler // FIQ
    .align 7; b default_handler // SError

    // Current EL with SPx
    .align 7; b default_handler // Synchronous
    .align 7; b handle_irq_entry // IRQ
    .align 7; b default_handler // FIQ
    .align 7; b default_handler // SError

    // Lower EL using AArch64 (EL0 User Space)
    .align 7; b handle_sync_entry // Synchronous (SVC #0)
    .align 7; b handle_irq_entry  // IRQ
    .align 7; b default_handler   // FIQ
    .align 7; b default_handler   // SError

    // Lower EL using AArch32
    .align 7; b default_handler
    .align 7; b default_handler
    .align 7; b default_handler
    .align 7; b default_handler

default_handler:
    eret

handle_irq_entry:
    // Guardar contexto completo de registros x0..x30
    sub sp, sp, #272
    stp x0, x1, [sp, #0]
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30, [sp, #240]

    mov x0, sp
    bl aarch64_handle_irq

    // Restaurar contexto
    ldp x0, x1, [sp, #0]
    ldp x2, x3, [sp, #16]
    ldp x4, x5, [sp, #32]
    ldp x6, x7, [sp, #48]
    ldp x8, x9, [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30, [sp, #240]
    add sp, sp, #272
    eret

handle_sync_entry:
    sub sp, sp, #272
    stp x0, x1, [sp, #0]
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30, [sp, #240]

    mov x0, sp
    bl aarch64_handle_sync

    ldp x0, x1, [sp, #0]
    ldp x2, x3, [sp, #16]
    ldp x4, x5, [sp, #32]
    ldp x6, x7, [sp, #48]
    ldp x8, x9, [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30, [sp, #240]
    add sp, sp, #272
    eret
"#);
