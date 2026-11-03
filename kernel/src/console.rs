// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Unified Kernel Console Macros (VGA + Serial Output)
// ============================================================================

use core::fmt::{self, Write};
use crate::vga::{VgaColor, VGA_WRITER};
use crate::serial::SERIAL;

pub fn _kprint(args: fmt::Arguments) {
    let _ = VGA_WRITER.lock().write_fmt(args);
    let _ = SERIAL.lock().write_fmt(args);

    if let Some(vbe) = crate::drivers::vbe::VBE.try_lock() {
        if vbe.is_active {
            drop(vbe);
            let s = alloc::fmt::format(args);
            crate::gui::desktop::gui_terminal_write_str(&s);
        }
    }
}

pub fn _kprint_colored(fg: VgaColor, bg: VgaColor, args: fmt::Arguments) {
    {
        let mut vga = VGA_WRITER.lock();
        vga.set_color(fg, bg);
        let _ = vga.write_fmt(args);
    }
    let _ = SERIAL.lock().write_fmt(args);

    if let Some(vbe) = crate::drivers::vbe::VBE.try_lock() {
        if vbe.is_active {
            drop(vbe);
            let s = alloc::fmt::format(args);
            crate::gui::desktop::gui_terminal_write_str(&s);
        }
    }
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::console::_kprint(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("\n"));
    ($($arg:tt)*) => ({
        $crate::console::_kprint(format_args!($($arg)*));
        $crate::kprint!("\n");
    });
}

#[macro_export]
macro_rules! kprint_color {
    ($fg:expr, $bg:expr, $($arg:tt)*) => {
        $crate::console::_kprint_colored($fg, $bg, format_args!($($arg)*))
    };
}
