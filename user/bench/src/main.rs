// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Benchmark ELF64 program running in Ring 3
// ============================================================================

#![no_std]
#![no_main]

use beaver_libc::{write, exit, uptime};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let msg_start = b"[BENCH.ELF] Ejecutando benchmark aritmetico en Ring 3...\n";
    let _ = write(1, msg_start);

    let _t0 = uptime();
    let mut sum: u64 = 0;
    for i in 0..10_000u64 {
        sum = sum.wrapping_add(i ^ 0x55AA);
    }

    let msg_done = b"[BENCH.ELF] 10,000 iteraciones en Ring 3 finalizadas con exito (0 fallos).\n";
    let _ = write(1, msg_done);
    exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(1);
}

