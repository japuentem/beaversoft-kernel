// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Hello World ELF64 program running in Ring 3
// ============================================================================

#![no_std]
#![no_main]

use beaver_libc::{write, exit};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let msg = b"[HELLO.ELF] Hola Mundo desde Espacio de Usuario (Ring 3) en Beaver OS!\n";
    let _ = write(1, msg);
    exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(1);
}
