// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver OS Ring 3 Native Interactive Shell
// ============================================================================

#![no_std]
#![no_main]

use beaver_libc::{read, write, uptime, getpid, exit};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let banner = b"Beaver OS Native Shell (Ring 3)\nEscribe 'help' o 'exit'\n";
    let _ = write(1, banner);

    let prompt = b"beaver-sh (Ring 3)$ ";
    let mut buf = [0u8; 128];

    loop {
        let _ = write(1, prompt);
        match read(0, &mut buf) {
            Ok(n) if n > 0 => {
                let cmd = &buf[..n];
                if cmd.starts_with(b"exit") {
                    exit(0);
                } else if cmd.starts_with(b"help") {
                    let _ = write(1, b"Comandos: help, uptime, whoami, exit\n");
                } else if cmd.starts_with(b"whoami") {
                    let pid = getpid();
                    let mut msg = [0u8; 40];
                    let p = b"ring3-user (PID: ";
                    let mut idx = 0;
                    for &b in p { msg[idx] = b; idx += 1; }
                    msg[idx] = b'0' + ((pid % 10) as u8); idx += 1;
                    msg[idx] = b')'; idx += 1;
                    msg[idx] = b'\n'; idx += 1;
                    let _ = write(1, &msg[..idx]);
                } else if cmd.starts_with(b"uptime") {
                    let up = uptime();
                    let mut msg = [0u8; 48];
                    let p = b"Kernel Uptime: ";
                    let mut idx = 0;
                    for &b in p { msg[idx] = b; idx += 1; }
                    msg[idx] = b'0' + ((up % 10) as u8); idx += 1;
                    for &b in b" s\n" { msg[idx] = b; idx += 1; }
                    let _ = write(1, &msg[..idx]);
                }
            }
            _ => {}
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(1);
}
