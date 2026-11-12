// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Hello World ELF64 program running in Ring 3
// ============================================================================

#![no_std]
#![no_main]

use beaver_libc::{write, exit, malloc, free};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let msg = b"[HELLO.ELF] Hola Mundo desde Espacio de Usuario (Ring 3) en Beaver OS!\n";
    let _ = write(1, msg);

    // Asignación dinámica en Heap de usuario mediante SYS_BRK + malloc
    let ptr = malloc(64);
    if !ptr.is_null() {
        let heap_msg = b"  [USER-HEAP] Asignacion dinamica con malloc() en Ring 3 [OK]\n";
        unsafe {
            core::ptr::copy_nonoverlapping(heap_msg.as_ptr(), ptr, heap_msg.len());
            let _ = write(1, core::slice::from_raw_parts(ptr, heap_msg.len()));
        }
        free(ptr);
    }

    exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(1);
}

