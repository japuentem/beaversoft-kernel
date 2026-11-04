// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: IDT (Interrupt Descriptor Table) & CPU Exception / IRQ Handlers
// ============================================================================

pub mod pic;

use core::arch::asm;
use core::ptr::addr_of_mut;
use crate::gdt::DOUBLE_FAULT_IST_INDEX;
use crate::interrupts::pic::PICS;
use crate::drivers::timer;
use crate::drivers::keyboard;
use crate::vga::{VgaColor, VGA_WRITER};
use crate::kprint_color;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    pub const fn empty() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    pub fn set_handler_addr(&mut self, addr: u64, ist_index: u8) {
        self.offset_low = addr as u16;
        self.selector = 0x08; // Selector de código de kernel de 64 bits (GDT entry 1)
        self.ist = ist_index & 0x07;
        self.type_attr = 0x8E; // Presente (1), DPL 00, Interrupt Gate 64-bit (0xE)
        self.offset_mid = (addr >> 16) as u16;
        self.offset_high = (addr >> 32) as u32;
        self.reserved = 0;
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct InterruptStackFrame {
    pub instruction_pointer: u64,
    pub code_segment: u64,
    pub cpu_flags: u64,
    pub stack_pointer: u64,
    pub stack_segment: u64,
}

#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::empty(); 256];

pub fn init_idt() {
    // SAFETY: Se inicializa la tabla estática `IDT` antes de habilitar interrupciones (IF=0) y se registra en el procesador mediante `lidt`.
    unsafe {
        // --- Excepciones del CPU ---
        set_entry(0, divide_error_handler as *const () as usize as u64, 0);
        set_entry(3, breakpoint_handler as *const () as usize as u64, 0);
        set_entry(6, invalid_opcode_handler as *const () as usize as u64, 0);
        // Doble Falla (#DF) configurada con IST1 para stack dedicado a prueba de fallos
        set_entry(8, double_fault_handler as *const () as usize as u64, DOUBLE_FAULT_IST_INDEX as u8);
        set_entry(13, general_protection_fault_handler as *const () as usize as u64, 0);
        set_entry(14, page_fault_handler as *const () as usize as u64, 0);

        // --- Interrupciones de Hardware Remapeadas (IRQs) ---
        set_entry(32, timer_interrupt_handler as *const () as usize as u64, 0);    // IRQ 0: PIT Timer
        set_entry(33, keyboard_interrupt_handler as *const () as usize as u64, 0); // IRQ 1: PS/2 Keyboard

        // Handlers por defecto para todas las demás interrupciones (34..=255)
        for i in 34..=255 {
            set_entry(i, generic_irq_handler as *const () as usize as u64, 0);
        }

        // IRQ 12: Ratón PS/2 (Vector PIC2_OFFSET + 4 = 44)
        set_entry(44, mouse_interrupt_handler as *const () as usize as u64, 0);

        // Vector 48 (0x30): Temporizador Local APIC en modo Periódico (Time-Slicing)
        set_entry(48, lapic_timer_interrupt_handler as *const () as usize as u64, 0);

        // Vector 255 (0xFF): Interrupción Espuria del Local APIC
        set_entry(255, spurious_interrupt_handler as *const () as usize as u64, 0);

        let idt_raw = addr_of_mut!(IDT) as *mut IdtEntry;
        let idtr = IdtPointer {
            limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: idt_raw as u64,
        };

        asm!("lidt [{}]", in(reg) &idtr, options(readonly, nostack, preserves_flags));
    }
}

/// Carga la IDT ya inicializada por el BSP en un núcleo secundario (AP).
pub fn load_idt_on_ap() {
    // SAFETY: `IDT` ya fue poblada por el BSP antes de iniciar el arranque SMP; `lidt` solo registra el puntero en el AP actual.
    unsafe {
        let idt_raw = addr_of_mut!(IDT) as *mut IdtEntry;
        let idtr = IdtPointer {
            limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: idt_raw as u64,
        };
        asm!("lidt [{}]", in(reg) &idtr, options(readonly, nostack, preserves_flags));
    }
}

// SAFETY: El llamador debe garantizar que `index < 256` y que no hay acceso concurrente a `IDT`.
unsafe fn set_entry(index: usize, handler_addr: u64, ist: u8) {
    let idt_raw = addr_of_mut!(IDT) as *mut IdtEntry;
    (*idt_raw.add(index)).set_handler_addr(handler_addr, ist);
}

// ============================================================================
// Excepciones del Procesador
// ============================================================================

extern "x86-interrupt" fn divide_error_handler(frame: InterruptStackFrame) {
    kprint_color!(VgaColor::LightRed, VgaColor::Black, "\n[CPU EXCEPTION] #DE Divide-by-Zero Error\n{:#?}\n", frame);
    loop {
        // SAFETY: `hlt` detiene la CPU en Ring 0 tras una excepción fatal de división por cero.
        unsafe { asm!("hlt"); }
    }
}

extern "x86-interrupt" fn breakpoint_handler(frame: InterruptStackFrame) {
    kprint_color!(VgaColor::Yellow, VgaColor::Black, "\n[CPU EXCEPTION] #BP Breakpoint Triggered\n{:#?}\n", frame);
}

extern "x86-interrupt" fn invalid_opcode_handler(frame: InterruptStackFrame) {
    kprint_color!(VgaColor::LightRed, VgaColor::Black, "\n[CPU EXCEPTION] #UD Invalid Opcode Encountered\n{:#?}\n", frame);
    loop {
        // SAFETY: `hlt` detiene la CPU en Ring 0 tras encontrar un opcode inválido irrecuperable.
        unsafe { asm!("hlt"); }
    }
}

extern "x86-interrupt" fn double_fault_handler(frame: InterruptStackFrame, error_code: u64) -> ! {
    kprint_color!(
        VgaColor::LightRed, VgaColor::Black,
        "\n============================================================\n\
         [CRITICAL CPU EXCEPTION] #DF DOUBLE FAULT (IST1 Active)\n\
         Error Code: {:#x}\n\
         Frame: {:#?}\n\
         ============================================================\n",
        error_code, frame
    );
    loop {
        // SAFETY: `hlt` detiene la CPU en la pila dedicada IST1 tras una doble falla irrecuperable.
        unsafe { asm!("hlt"); }
    }
}

extern "x86-interrupt" fn general_protection_fault_handler(frame: InterruptStackFrame, error_code: u64) {
    // Detectar si el fallo vino de Ring 3 (CPL=3 en CS del frame)
    let from_user = (frame.code_segment & 0x3) == 3;

    if from_user {
        // ────────────────────────────────────────────────────────────────
        // FALLO DE USUARIO → Anti-BSOD: matar solo el proceso, no el kernel
        // ────────────────────────────────────────────────────────────────
        kprint_color!(
            VgaColor::LightRed, VgaColor::Black,
            "\n[ISOLATION] #GP desde Ring 3 | Error: {:#x} | RIP: {:#x}\n",
            error_code, frame.instruction_pointer
        );
        kprint_color!(
            VgaColor::Yellow, VgaColor::Black,
            "  → Proceso de usuario terminado. Kernel sigue activo. (Anti-BSOD)\n"
        );
        // EOI al PIC para que el timer pueda seguir funcionando
        // SAFETY: Envío de EOI al controlador PIC 8259 en Ring 0 antes de ceder la CPU tras aislar el fallo de usuario.
        unsafe { PICS.notify_end_of_interrupt(32); }
        crate::task::scheduler::mark_finished_and_yield();
        // mark_finished_and_yield llama a schedule() que hace switch_context
        // Si llegamos aquí (sin otras tareas), simplemente esperamos
        // SAFETY: `hlt` suspende la CPU si no quedan más tareas listas para planificar.
        loop { unsafe { asm!("hlt"); } }
    } else {
        // ────────────────────────────────────────────────────────────────
        // FALLO DE KERNEL → Panic inevitable (equivale a BSOD de Windows
        //                   pero solo para código kernel, jamás para usuario)
        // ────────────────────────────────────────────────────────────────
        kprint_color!(
            VgaColor::LightRed, VgaColor::Black,
            "\n[CRITICAL] #GP KERNEL General Protection Fault! Error: {:#x}\n{:#?}\n",
            error_code, frame
        );
        // SAFETY: `hlt` detiene la CPU en Ring 0 ante un fallo de protección general dentro del kernel.
        loop { unsafe { asm!("hlt"); } }
    }
}

extern "x86-interrupt" fn page_fault_handler(frame: InterruptStackFrame, error_code: u64) {
    let fault_addr: u64;
    // SAFETY: Lectura del registro de control `CR2` en Ring 0 para obtener la dirección lineal que causó el `#PF`.
    unsafe {
        asm!("mov {}, cr2", out(reg) fault_addr, options(nomem, nostack, preserves_flags));
    }

    // Bit 2 del error code de #PF = acceso de modo usuario (U/S bit)
    let from_user = (error_code & 0x4) != 0;

    if from_user {
        // ────────────────────────────────────────────────────────────────
        // PAGE FAULT DE USUARIO → Anti-BSOD: matar solo el proceso
        // ────────────────────────────────────────────────────────────────
        kprint_color!(
            VgaColor::LightRed, VgaColor::Black,
            "\n[ISOLATION] #PF desde Ring 3 | Addr: {:#x} | Error: {:#x} | RIP: {:#x}\n",
            fault_addr, error_code, frame.instruction_pointer
        );
        kprint_color!(
            VgaColor::Yellow, VgaColor::Black,
            "  → Segfault de usuario contenido. Proceso terminado. Kernel activo. (Anti-BSOD)\n"
        );
        // SAFETY: Envío de EOI al PIC 8259 en Ring 0 para mantener activas las interrupciones tras terminar el proceso infractor.
        unsafe { PICS.notify_end_of_interrupt(32); }
        crate::task::scheduler::mark_finished_and_yield();
        // SAFETY: `hlt` suspende la CPU si no hay otra tarea a la cual conmutar.
        loop { unsafe { asm!("hlt"); } }
    } else {
        // ────────────────────────────────────────────────────────────────
        // PAGE FAULT DE KERNEL → Panic (fallo real del sistema)
        // ────────────────────────────────────────────────────────────────
        kprint_color!(
            VgaColor::LightRed, VgaColor::Black,
            "\n[CRITICAL] #PF KERNEL Page Fault!\nAddr: {:#x} | Error: {:#x}\n{:#?}\n",
            fault_addr, error_code, frame
        );
        // SAFETY: `hlt` detiene la CPU en Ring 0 tras un fallo de página crítico en modo kernel.
        loop { unsafe { asm!("hlt"); } }
    }
}

// ============================================================================
// Manejadores de Interrupciones de Hardware (IRQs)
// ============================================================================

extern "x86-interrupt" fn timer_interrupt_handler(_frame: InterruptStackFrame) {
    let ticks = timer::on_timer_tick();

    // Actualizar indicador de uptime cada segundo (100 ticks @ 100Hz) en la esquina superior derecha
    // sin alterar la posición ni visibilidad del cursor de hardware
    if ticks % 100 == 0 {
        let seconds = ticks / 100;
        if let Some(mut vga) = VGA_WRITER.try_lock() {
            vga.write_str_at(0, 62, "[ UPTIME: ", VgaColor::LightCyan, VgaColor::Blue);
            
            let mut num = seconds;
            let mut buf = [b'0'; 5];
            let mut idx = 5;
            if num == 0 {
                idx -= 1;
            } else {
                while num > 0 && idx > 0 {
                    idx -= 1;
                    buf[idx] = b'0' + (num % 10) as u8;
                    num /= 10;
                }
            }
            for (offset, i) in (idx..5).enumerate() {
                vga.write_char_at(0, 72 + offset, buf[i], VgaColor::LightCyan, VgaColor::Blue);
            }
            let total_digits = 5 - idx;
            vga.write_str_at(0, 72 + total_digits, "s ]", VgaColor::LightCyan, VgaColor::Blue);
        }
    }

    // Polling de COM1 para soporte de shell interactivo por puerto serie (QEMU stdio)
    // IMPORTANTE: liberar el lock de SERIAL antes de llamar a dispatch_input_char
    static mut SERIAL_ESC: u8 = 0;
    loop {
        let maybe_b = if let Some(serial) = crate::serial::SERIAL.try_lock() {
            serial.read_byte()
        } else {
            None
        };
        let b = match maybe_b {
            Some(byte) => byte,
            None => break,
        };

        // SAFETY: `SERIAL_ESC` solo es accedida dentro de `timer_interrupt_handler`, que se ejecuta con IF=0 en modo uniprocesador.
        let esc = unsafe { SERIAL_ESC };
        if esc == 0 {
            if b == 0x1B {
                // SAFETY: Acceso exclusivo a `SERIAL_ESC` en contexto de interrupción con IF=0.
                unsafe { SERIAL_ESC = 1; }
                continue;
            }
        } else if esc == 1 {
            if b == b'[' {
                // SAFETY: Acceso exclusivo a `SERIAL_ESC` en contexto de interrupción con IF=0.
                unsafe { SERIAL_ESC = 2; }
                continue;
            } else {
                // SAFETY: Acceso exclusivo a `SERIAL_ESC` en contexto de interrupción con IF=0.
                unsafe { SERIAL_ESC = 0; }
            }
        } else if esc == 2 {
            // SAFETY: Acceso exclusivo a `SERIAL_ESC` en contexto de interrupción con IF=0.
            unsafe { SERIAL_ESC = 0; }
            if b == b'A' {
                push_input_char(crate::drivers::keyboard::KEY_UP_ARROW);
                continue;
            } else if b == b'B' {
                push_input_char(crate::drivers::keyboard::KEY_DOWN_ARROW);
                continue;
            }
        }

        let ch = match b {
            b'\r' | b'\n' => '\n',
            0x08 | 0x7F => '\x08',
            c if c >= 0x20 && c <= 0x7E => c as char,
            _ => continue,
        };
        push_input_char(ch);
    }

    // SAFETY: Notificación EOI para el vector 32 (IRQ 0 del PIT) al controlador PIC 8259.
    unsafe {
        PICS.notify_end_of_interrupt(32);
    }

    // Conmutación preemptiva cada 5 ticks (quantum de 50ms)
    if ticks % 5 == 0 {
        crate::task::scheduler::schedule_tick();
    }
}

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

extern "x86-interrupt" fn keyboard_interrupt_handler(_frame: InterruptStackFrame) {
    if let Some(ch) = keyboard::on_keyboard_interrupt() {
        push_input_char(ch);
    }

    // SAFETY: Notificación EOI para el vector 33 (IRQ 1 del teclado PS/2) al controlador PIC 8259.
    unsafe {
        PICS.notify_end_of_interrupt(33);
    }
}

extern "x86-interrupt" fn mouse_interrupt_handler(_frame: InterruptStackFrame) {
    // SAFETY: Lectura del puerto de estado PS/2 (`0x64`) en el manejador de IRQ 12 en Ring 0.
    let status = unsafe { crate::io::inb(crate::drivers::mouse::PS2_STATUS_PORT) };
    if (status & 0x01) != 0 {
        // SAFETY: Lectura del puerto de datos PS/2 (`0x60`) tras confirmar que el búfer de salida tiene datos (`status & 0x01`).
        let byte = unsafe { crate::io::inb(crate::drivers::mouse::PS2_DATA_PORT) };
        if (status & 0x20) != 0 {
            if let Some(mut mouse) = crate::drivers::mouse::MOUSE.try_lock() {
                if let Some(state) = mouse.process_byte(byte) {
                    drop(mouse);
                    let is_vbe = if let Some(vbe) = crate::drivers::vbe::VBE.try_lock() {
                        vbe.is_active
                    } else {
                        false
                    };
                    if is_vbe {
                        crate::gui::desktop::handle_mouse_event(state);
                    }
                }
            }
        }
    }

    // SAFETY: Notificación EOI para el vector 44 (IRQ 12 en PIC esclavo y maestro).
    unsafe {
        PICS.notify_end_of_interrupt(44);
    }
}

extern "x86-interrupt" fn generic_irq_handler(_frame: InterruptStackFrame) {
    // SAFETY: Envío de EOI genérico al PIC maestro en Ring 0 para evitar bloqueo de líneas IRQ no manejadas.
    unsafe {
        PICS.notify_end_of_interrupt(32);
    }
}

extern "x86-interrupt" fn lapic_timer_interrupt_handler(_frame: InterruptStackFrame) {
    if let Some(mut smp) = crate::apic::SMP.try_lock() {
        smp.timer_ticks = smp.timer_ticks.wrapping_add(1);
        smp.end_of_interrupt();
    }
}

extern "x86-interrupt" fn spurious_interrupt_handler(_frame: InterruptStackFrame) {
    // Interrupción espuria del Local APIC: no requiere EOI según arquitectura Intel x86_64
}
