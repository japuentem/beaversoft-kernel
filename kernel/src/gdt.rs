// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: GDT (Global Descriptor Table) & TSS (Task State Segment) with IST
//              Phase 6: Ring 3 user-mode segments + syscall/sysret support
// ============================================================================

#![cfg_attr(target_arch = "aarch64", allow(dead_code, unused_imports))]

#[cfg(target_arch = "x86_64")]
use core::arch::asm;
use core::mem::size_of;
use core::ptr::addr_of_mut;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 1;

// Selectores de segmento de la GDT (Phase 6 layout compatible con SYSRETQ):
//   0x00 – Null
//   0x08 – Kernel Code  (Ring 0, DPL=0)
//   0x10 – Kernel Data  (Ring 0, DPL=0)
//   0x18 – User Data    (Ring 3, DPL=3, STAR[63:48] + 8)
//   0x20 – User Code    (Ring 3, DPL=3, STAR[63:48] + 16)
//   0x28 – TSS low  (16-byte descriptor, 2 slots)
//   0x30 – TSS high
pub const SELECTOR_KERNEL_CODE: u16 = 0x08;
pub const SELECTOR_KERNEL_DATA: u16 = 0x10;
pub const SELECTOR_USER_DATA:   u16 = 0x18 | 3; // RPL=3 (0x1B)
pub const SELECTOR_USER_CODE:   u16 = 0x20 | 3; // RPL=3 (0x23)
pub const SELECTOR_TSS:         u16 = 0x28;

#[repr(C, packed)]
pub struct TaskStateSegment {
    reserved_1: u32,
    pub rsp: [u64; 3],
    reserved_2: u64,
    pub ist: [u64; 7],
    reserved_3: u64,
    reserved_4: u16,
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            reserved_1: 0,
            rsp: [0; 3],
            reserved_2: 0,
            ist: [0; 7],
            reserved_3: 0,
            reserved_4: 0,
            iomap_base: size_of::<TaskStateSegment>() as u16,
        }
    }
}

// Pila de interrupción dedicada (IST1) para manejar Doble Falla (#DF) de forma infalible
const IST_STACK_SIZE: usize = 4096 * 4; // 16 KB
static mut DOUBLE_FAULT_STACK: [u8; IST_STACK_SIZE] = [0; IST_STACK_SIZE];

// Pila de Ring 0 usada al transitar de Ring 3 → Ring 0 por interrupciones de hardware
const RING0_STACK_SIZE: usize = 4096 * 4; // 16 KB
static mut RING0_STACK: [u8; RING0_STACK_SIZE] = [0; RING0_STACK_SIZE];

static mut TSS: TaskStateSegment = TaskStateSegment::new();

// GDT de 7 entradas (Phase 6):
//   [0]  Null  [1] KernelCode  [2] KernelData
//   [3]  UserCode(Ring3)  [4] UserData(Ring3)
//   [5..6] TSS (16 bytes = 2 slots de 64 bits)
static mut GDT: [u64; 7] = [0; 7];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[cfg(target_arch = "x86_64")]
pub fn init_gdt() {
    // SAFETY: Se inicializan las estructuras estáticas `TSS` y `GDT` durante el arranque mononúcleo con IF=0 y se cargan con `lgdt`/`ltr`.
    unsafe {
        // 1. Configurar la pila del IST1 en el TSS (para Double Fault)
        let df_stack_end = addr_of_mut!(DOUBLE_FAULT_STACK) as u64 + IST_STACK_SIZE as u64;
        (*addr_of_mut!(TSS)).ist[DOUBLE_FAULT_IST_INDEX as usize - 1] = df_stack_end;

        // 2. Configurar RSP0 en el TSS = pila Ring 0 para transiciones Ring3→Ring0
        //    El procesador carga este puntero automáticamente al procesar una
        //    interrupción/excepción desde Ring 3
        let ring0_stack_end = addr_of_mut!(RING0_STACK) as u64 + RING0_STACK_SIZE as u64;
        (*addr_of_mut!(TSS)).rsp[0] = ring0_stack_end;

        // 3. Llenar entradas de la GDT extendida
        let gdt_ptr_raw = addr_of_mut!(GDT) as *mut u64;

        // Null descriptor
        *gdt_ptr_raw.add(0) = 0x0000000000000000;

        // Kernel Code 64-bit (Ring 0): P=1, DPL=00, S=1, Type=0xA (Exec/Read), L=1
        *gdt_ptr_raw.add(1) = 0x00209A0000000000;

        // Kernel Data 64-bit (Ring 0): P=1, DPL=00, S=1, Type=0x2 (Read/Write)
        *gdt_ptr_raw.add(2) = 0x0000920000000000;

        // User Data 64-bit (Ring 3, slot 3 = 0x18): P=1, DPL=11, S=1, Type=0x2 (Read/Write)
        *gdt_ptr_raw.add(3) = 0x0000F20000000000;

        // User Code 64-bit (Ring 3, slot 4 = 0x20): P=1, DPL=11, S=1, Type=0xA (Exec/Read), L=1
        *gdt_ptr_raw.add(4) = 0x0020FA0000000000;

        // 4. Crear Descriptor TSS de 64 bits (16 bytes = 2 entradas de 64 bits) en slots 5..6
        let tss_base  = addr_of_mut!(TSS) as u64;
        let tss_limit = (size_of::<TaskStateSegment>() - 1) as u64;

        let mut tss_low: u64 = 0;
        tss_low |= tss_limit & 0xFFFF;
        tss_low |= (tss_base & 0x00FF_FFFF) << 16;
        tss_low |= 0x89_u64 << 40; // P=1, DPL=0, Type=0x9 (64-bit TSS Available)
        tss_low |= ((tss_limit >> 16) & 0x0F) << 48;
        tss_low |= ((tss_base >> 24) & 0xFF) << 56;

        let tss_high: u64 = (tss_base >> 32) & 0xFFFF_FFFF;

        *gdt_ptr_raw.add(5) = tss_low;  // 0x28: TSS low
        *gdt_ptr_raw.add(6) = tss_high; // 0x30: TSS high

        // 5. Cargar la GDT extendida con LGDT
        let gdt_ptr = DescriptorTablePointer {
            limit: (size_of::<[u64; 7]>() - 1) as u16,
            base: gdt_ptr_raw as u64,
        };
        asm!("lgdt [{}]", in(reg) &gdt_ptr, options(readonly, nostack, preserves_flags));

        // 6. Recargar registros de segmento: CS = 0x08, DS/ES/SS/FS/GS = 0x10
        asm!(
            "push 0x08",
            "lea {tmp}, [2f + rip]",
            "push {tmp}",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            "mov fs, ax",
            "mov gs, ax",
            tmp = out(reg) _,
            options(preserves_flags)
        );

        // 7. Cargar el Task Register con el nuevo selector de TSS (0x28)
        let tss_selector: u16 = SELECTOR_TSS;
        asm!("ltr cx", in("cx") tss_selector, options(nostack, preserves_flags));
    }
}

/// Actualiza el RSP0 del TSS con la pila de kernel de la tarea actual.
/// Debe llamarse antes de entrar a Ring 3 para cada tarea.
#[cfg(target_arch = "x86_64")]
pub fn set_tss_rsp0(stack_top: u64) {
    // SAFETY: Actualización atómica en modo uniprocesador del campo `rsp[0]` de la estructura estática `TSS`.
    unsafe {
        (*addr_of_mut!(TSS)).rsp[0] = stack_top;
    }
}

/// Carga la GDT principal del kernel en un núcleo secundario (AP) tras completar el trampolín de 16->64 bits.
#[cfg(target_arch = "x86_64")]
pub fn load_gdt_on_ap() {
    // SAFETY: `GDT` ya fue inicializada por el BSP antes de enviar SIPI; el AP solo carga `GDTR` y selectores de Ring 0.
    unsafe {
        let gdt_ptr_raw = addr_of_mut!(GDT) as *mut u64;
        let gdt_ptr = DescriptorTablePointer {
            limit: (size_of::<[u64; 7]>() - 1) as u16,
            base: gdt_ptr_raw as u64,
        };
        asm!("lgdt [{}]", in(reg) &gdt_ptr, options(readonly, nostack, preserves_flags));
    }
}

#[cfg(target_arch = "aarch64")]
pub fn init_gdt() {}

#[cfg(target_arch = "aarch64")]
pub fn set_tss_rsp0(_stack_top: u64) {}

#[cfg(target_arch = "aarch64")]
pub fn load_gdt_on_ap() {}


