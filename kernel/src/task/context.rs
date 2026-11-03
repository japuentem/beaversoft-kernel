// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Low-Level x86_64 Context Switch & CPU State Management
// ============================================================================

use core::arch::global_asm;

// Rutina ultra-rápida de conmutación de contexto en ensamblador puro
// Asegura explícitamente estar en la sección ejecutable .text
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

extern "C" {
    pub fn switch_context(old_stack_ptr: *mut *mut u8, new_stack_ptr: *mut u8);
}
