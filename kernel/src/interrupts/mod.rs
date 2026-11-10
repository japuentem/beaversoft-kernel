// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Multi-Architecture Interrupt Subsystem & Input Event Queue
// ============================================================================

pub mod pic;

#[cfg(target_arch = "x86_64")]
pub mod x86_64_idt;
#[cfg(target_arch = "x86_64")]
pub use x86_64_idt::*;

#[cfg(target_arch = "aarch64")]
pub fn init_idt() {}

#[cfg(target_arch = "aarch64")]
pub fn load_idt_on_ap() {}

// ============================================================================
// Cola de Entrada de Teclado y Terminal (Agnóstica de Arquitectura)
// ============================================================================

struct InputQueue {
    buf: [char; 256],
    head: usize,
    tail: usize,
}

impl InputQueue {
    const fn new() -> Self {
        Self {
            buf: ['\0'; 256],
            head: 0,
            tail: 0,
        }
    }

    fn push(&mut self, ch: char) {
        let next = (self.tail + 1) % 256;
        if next != self.head {
            self.buf[self.tail] = ch;
            self.tail = next;
        }
    }

    fn pop(&mut self) -> Option<char> {
        if self.head == self.tail {
            None
        } else {
            let ch = self.buf[self.head];
            self.head = (self.head + 1) % 256;
            Some(ch)
        }
    }

    fn is_empty(&self) -> bool {
        self.head == self.tail
    }
}

static INPUT_QUEUE: crate::sync::SpinMutex<InputQueue> = crate::sync::SpinMutex::new(InputQueue::new());

pub fn push_input_char(ch: char) {
    if let Some(mut q) = INPUT_QUEUE.try_lock() {
        q.push(ch);
    }
}

pub fn pop_input_char() -> Option<char> {
    if let Some(mut q) = INPUT_QUEUE.try_lock() {
        q.pop()
    } else {
        None
    }
}

pub fn has_pending_input() -> bool {
    if let Some(q) = INPUT_QUEUE.try_lock() {
        !q.is_empty()
    } else {
        false
    }
}

pub fn process_pending_input() {
    loop {
        let maybe_ch = if let Some(mut q) = INPUT_QUEUE.try_lock() {
            q.pop()
        } else {
            None
        };
        match maybe_ch {
            Some(ch) => dispatch_input_char(ch),
            None => break,
        }
    }
}

pub fn dispatch_input_char(ch: char) {
    let is_vbe = if let Some(vbe) = crate::drivers::vbe::VBE.try_lock() {
        vbe.is_active
    } else {
        false
    };

    if is_vbe {
        crate::gui::desktop::handle_gui_char(ch);
    } else {
        crate::shell::SHELL.lock().handle_char(ch);
    }
}
