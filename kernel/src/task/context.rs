// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Low-Level x86_64 Context Switch & CPU State Management
// ============================================================================

use core::arch::global_asm;

#[cfg(target_arch = "x86_64")]
global_asm!(
    r#"
    .section .text
    .global switch_context
    .type switch_context, @function
    switch_context:
        // rdi: *mut *mut u8 (donde guardar el RSP de la tarea saliente)
        // rsi: *mut u8      (el nuevo RSP de la tarea entrante)

        // 1. Guardar banderas y registros de la tarea actual en su pila
        pushfq
        push rbp
        push rbx
        push r12
        push r13
        push r14
        push r15

        // 2. Guardar el puntero de pila actual (RSP)
        mov [rdi], rsp

        // 3. Cargar el puntero de pila de la nueva tarea
        mov rsp, rsi

        // 4. Restaurar registros de la nueva tarea desde su pila
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbx
        pop rbp
        popfq

        // 5. Saltar a la instrucción siguiente de la nueva tarea
        ret
    "#
);

#[cfg(target_arch = "aarch64")]
global_asm!(
    r#"
    .section .text
    .global switch_context
    .type switch_context, %function
    switch_context:
        // x0: *mut *mut u8 (old_rsp)
        // x1: *mut u8      (new_rsp)
        stp x19, x20, [sp, #-16]!
        stp x21, x22, [sp, #-16]!
        stp x23, x24, [sp, #-16]!
        stp x25, x26, [sp, #-16]!
        stp x27, x28, [sp, #-16]!
        stp x29, x30, [sp, #-16]!

        mov x2, sp
        str x2, [x0]

        mov sp, x1

        ldp x29, x30, [sp], #16
        ldp x27, x28, [sp], #16
        ldp x25, x26, [sp], #16
        ldp x23, x24, [sp], #16
        ldp x21, x22, [sp], #16
        ldp x19, x20, [sp], #16
        ret
    "#
);

extern "C" {
    pub fn switch_context(old_stack_ptr: *mut *mut u8, new_stack_ptr: *mut u8);
}
