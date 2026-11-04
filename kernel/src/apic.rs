// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Local APIC (Advanced Programmable Interrupt Controller),
//              ACPI MADT Multi-Core Discovery, 16-bit Real-Mode Trampoline at 0x8000
//              (INIT-SIPI-SIPI) & LAPIC Preemptive Timer
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{fence, AtomicBool, AtomicU32, Ordering};
use crate::io::io_wait;
use crate::memory::frame_allocator::{FRAME_ALLOCATOR, PAGE_SIZE};
use crate::memory::paging;
use crate::sync::SpinMutex;

pub const DEFAULT_LAPIC_BASE: u64 = 0xFEE00000;
pub const MSR_APIC_BASE: u32      = 0x0000001B;

/// Dirección física del trampolín SMP de 16 bits (Vector SIPI 0x08 -> 0x8000).
/// Nota: `UefiBootInfo` fue reubicado en `0x9000`, dejando `0x8000..0x8FFF` exclusivo para SMP.
pub const SMP_TRAMPOLINE_ADDR: usize = 0x8000;
pub const SMP_SIPI_VECTOR: u32       = (SMP_TRAMPOLINE_ADDR / 0x1000) as u32; // 0x08

// Direcciones de buzón (mailbox) dentro de la página física 0x8000
const TRAMPOLINE_GDT_ADDR: usize     = 0x8100;
const TRAMPOLINE_GDTR_ADDR: usize    = 0x8120;
const TRAMPOLINE_PML4_ADDR: usize    = 0x8F00; // u32: dirección física de PML4 (< 4 GiB)
const TRAMPOLINE_STACK_ADDR: usize   = 0x8F08; // u64: cima de pila de 16 KiB del AP
const TRAMPOLINE_ENTRY_ADDR: usize   = 0x8F10; // u64: puntero a `ap_kernel_entry`
const TRAMPOLINE_ACK_ADDR: usize     = 0x8F18; // u32: confirmación escrita por el AP (1 = online)
const TRAMPOLINE_APIC_ID_ADDR: usize = 0x8F20; // u32: APIC ID leído por el propio AP

// --- Offsets de Registros MMIO del Local APIC ---
pub const LAPIC_ID: usize         = 0x020; // Local APIC ID (bits 31:24)
pub const LAPIC_VERSION: usize    = 0x030; // Local APIC Version
pub const LAPIC_TPR: usize        = 0x080; // Task Priority Register
pub const LAPIC_APR: usize        = 0x090; // Arbitration Priority Register
pub const LAPIC_PPR: usize        = 0x0A0; // Processor Priority Register
pub const LAPIC_EOI: usize        = 0x0B0; // End of Interrupt Register
pub const LAPIC_LDR: usize        = 0x0D0; // Logical Destination Register
pub const LAPIC_DFR: usize        = 0x0E0; // Destination Format Register
pub const LAPIC_SVR: usize        = 0x0F0; // Spurious Interrupt Vector Register
pub const LAPIC_ESR: usize        = 0x280; // Error Status Register
pub const LAPIC_ICR_LOW: usize    = 0x300; // Interrupt Command Register (Bits 0..31)
pub const LAPIC_ICR_HIGH: usize   = 0x310; // Interrupt Command Register (Bits 32..63)
pub const LAPIC_LVT_TIMER: usize  = 0x320; // LVT Timer Register
pub const LAPIC_LVT_LINT0: usize  = 0x350; // LVT LINT0 Register
pub const LAPIC_LVT_LINT1: usize  = 0x360; // LVT LINT1 Register
pub const LAPIC_LVT_ERROR: usize  = 0x370; // LVT Error Register
pub const LAPIC_TIMER_INIT: usize = 0x380; // Timer Initial Count Register
pub const LAPIC_TIMER_CURR: usize = 0x390; // Timer Current Count Register
pub const LAPIC_TIMER_DIV: usize  = 0x3E0; // Timer Divide Configuration Register

// --- Constantes de IPI (Inter-Processor Interrupt) ---
pub const ICR_TYPE_FIXED: u32     = 0x00000000;
pub const ICR_TYPE_INIT: u32      = 0x00000500; // INIT IPI
pub const ICR_TYPE_STARTUP: u32   = 0x00000600; // Startup IPI (SIPI)
pub const ICR_LEVEL_ASSERT: u32   = 0x00004000;
pub const ICR_LEVEL_DEASSERT: u32 = 0x00000000;
pub const ICR_DEST_ALL_EXC: u32   = 0x000C0000; // All Excluding Self

#[derive(Debug, Clone, Copy)]
pub struct CoreInfo {
    pub core_id: u32,
    pub apic_id: u32,
    pub is_bsp: bool, // Bootstrap Processor (Core 0)
    pub is_online: bool,
    pub stack_top: u64,
    pub tasks_executed: u64,
}

pub struct SmpManager {
    pub lapic_base: u64,
    pub bsp_apic_id: u32,
    pub madt_addr: u64,
    pub madt_detected: bool,
    pub trampoline_addr: u64,
    pub cores: Vec<CoreInfo>,
    pub smp_active: bool,
    pub timer_ticks: u64,
}

impl SmpManager {
    pub const fn new() -> Self {
        Self {
            lapic_base: DEFAULT_LAPIC_BASE,
            bsp_apic_id: 0,
            madt_addr: 0,
            madt_detected: false,
            trampoline_addr: SMP_TRAMPOLINE_ADDR as u64,
            cores: Vec::new(),
            smp_active: false,
            timer_ticks: 0,
        }
    }

    /// Lee un registro de 32 bits del Local APIC por MMIO
    #[inline(always)]
    // SAFETY: El llamador garantiza que `self.lapic_base + offset` apunta a un registro MMIO válido del Local APIC.
    pub unsafe fn read_reg(&self, offset: usize) -> u32 {
        let ptr = (self.lapic_base + offset as u64) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    /// Escribe un registro de 32 bits en el Local APIC por MMIO
    #[inline(always)]
    // SAFETY: El llamador garantiza que `self.lapic_base + offset` apunta a un registro MMIO válido del Local APIC.
    pub unsafe fn write_reg(&self, offset: usize, value: u32) {
        let ptr = (self.lapic_base + offset as u64) as *mut u32;
        core::ptr::write_volatile(ptr, value);
    }

    /// Notifica el fin de una interrupción al Local APIC (EOI)
    #[inline(always)]
    pub fn end_of_interrupt(&self) {
        // SAFETY: El registro `LAPIC_EOI` (offset 0xB0) es seguro de escribir en MMIO para acusar recibo de interrupción.
        unsafe {
            self.write_reg(LAPIC_EOI, 0);
        }
    }
}

pub static SMP: SpinMutex<SmpManager> = SpinMutex::new(SmpManager::new());
pub static AP_BOOT_FLAG: AtomicBool = AtomicBool::new(false);
pub static AP_ONLINE_COUNT: AtomicU32 = AtomicU32::new(1); // BSP ya está online
static LAPIC_READY: AtomicBool = AtomicBool::new(false);

/// Devuelve el APIC ID del procesador que ejecuta actualmente (0 si el Local APIC aún no está activo).
#[inline(always)]
pub fn current_apic_id() -> u32 {
    if !LAPIC_READY.load(Ordering::Relaxed) {
        return 0;
    }
    // SAFETY: `DEFAULT_LAPIC_BASE + LAPIC_ID` (0xFEE00020) está mapeado en MMIO y `LAPIC_READY` confirma que el Local APIC fue inicializado.
    unsafe {
        let val = core::ptr::read_volatile((DEFAULT_LAPIC_BASE + LAPIC_ID as u64) as *const u32);
        (val >> 24) & 0xFF
    }
}

// ============================================================================
// Descubrimiento ACPI RSDP -> RSDT/XSDT -> MADT ("APIC")
// ============================================================================

/// Verifica si en la dirección física `addr` existe una estructura RSDP ACPI válida (`"RSD PTR "`).
fn is_valid_rsdp(addr: u64) -> bool {
    // SAFETY: Lectura de 20 bytes en regiones BIOS/ACPI mapeadas por identidad.
    unsafe {
        let ptr = addr as *const u8;
        let sig = core::slice::from_raw_parts(ptr, 8);
        if sig != b"RSD PTR " {
            return false;
        }
        let revision = *ptr.add(15);
        if revision > 2 {
            return false;
        }
        let mut sum: u8 = 0;
        for i in 0..20 {
            sum = sum.wrapping_add(*ptr.add(i));
        }
        if sum != 0 {
            return false;
        }
        // Verificar que rsdt_addr apunte a una dirección razonable (< 4 GiB)
        let rsdt_addr = core::ptr::read_unaligned(ptr.add(16) as *const u32);
        rsdt_addr >= 0x1000 && rsdt_addr < 0xFFE0_0000
    }
}

/// Busca el puntero RSDP en áreas estándar BIOS (EBDA y 0xE0000..0x100000) y en regiones ACPI de UEFI (OVMF).
fn find_rsdp() -> Option<u64> {
    // 1. Escanear EBDA (0x0008_0000 .. 0x000A_0000) y ROM BIOS (0x000E_0000 .. 0x0010_0000)
    let legacy_ranges: [(u64, u64); 2] = [
        (0x0008_0000, 0x000A_0000),
        (0x000E_0000, 0x0010_0000),
    ];
    for (start, end) in legacy_ranges {
        let mut addr = start;
        while addr < end {
            if is_valid_rsdp(addr) {
                return Some(addr);
            }
            addr += 16;
        }
    }

    // 2. Escanear regiones altas donde OVMF UEFI coloca `ACPIReclaimMemory` (en máquinas de 128..512 MiB)
    let uefi_ranges: [(u64, u64); 2] = [
        (0x1E00_0000, 0x2000_0000), // Top de 512 MiB en OVMF
        (0x0700_0000, 0x0800_0000), // Top de 128 MiB en OVMF
    ];
    let magic_u64 = u64::from_le_bytes(*b"RSD PTR ");
    for (start, end) in uefi_ranges {
        let mut addr = start;
        while addr < end {
            // SAFETY: Comparación rápida de 8 bytes alineada a 16 bytes en memoria física mapeada por identidad.
            let candidate = unsafe { *(addr as *const u64) };
            if candidate == magic_u64 && is_valid_rsdp(addr) {
                return Some(addr);
            }
            addr += 16;
        }
    }

    None
}

/// Busca la tabla `"APIC"` (MADT) dentro de una RSDT (32 bits) o XSDT (64 bits).
fn find_madt_in_sdt(sdt_addr: u64, is_xsdt: bool) -> Option<u64> {
    if sdt_addr < 0x1000 || sdt_addr >= 0xFFE0_0000 {
        return None;
    }
    // SAFETY: Lectura de cabecera ACPI SDT en `sdt_addr`.
    unsafe {
        let ptr = sdt_addr as *const u8;
        let sig = core::slice::from_raw_parts(ptr, 4);
        if (is_xsdt && sig != b"XSDT") || (!is_xsdt && sig != b"RSDT") {
            return None;
        }
        let total_len = core::ptr::read_unaligned(ptr.add(4) as *const u32) as usize;
        if total_len < 36 || total_len > 65536 {
            return None;
        }
        let entries_bytes = total_len - 36;
        let stride = if is_xsdt { 8 } else { 4 };
        let count = entries_bytes / stride;

        for i in 0..count {
            let entry_ptr = ptr.add(36 + i * stride);
            let table_addr = if is_xsdt {
                core::ptr::read_unaligned(entry_ptr as *const u64)
            } else {
                core::ptr::read_unaligned(entry_ptr as *const u32) as u64
            };
            if table_addr >= 0x1000 && table_addr < 0xFFE0_0000 {
                let t_sig = core::slice::from_raw_parts(table_addr as *const u8, 4);
                if t_sig == b"APIC" {
                    return Some(table_addr);
                }
            }
        }
    }
    None
}

/// Parsea la tabla ACPI MADT (`"APIC"`) y devuelve `(madt_addr, lapic_phys_base, lista_de_apic_ids)`.
pub fn discover_madt_cores() -> Option<(u64, u64, Vec<u32>)> {
    let rsdp_addr = find_rsdp()?;
    // SAFETY: `rsdp_addr` ya fue validado por `is_valid_rsdp`.
    let (revision, rsdt_addr, xsdt_addr) = unsafe {
        let ptr = rsdp_addr as *const u8;
        let rev = *ptr.add(15);
        let rsdt = core::ptr::read_unaligned(ptr.add(16) as *const u32) as u64;
        let xsdt = if rev >= 2 {
            core::ptr::read_unaligned(ptr.add(24) as *const u64)
        } else {
            0
        };
        (rev, rsdt, xsdt)
    };

    let madt_addr = if revision >= 2 && xsdt_addr != 0 {
        find_madt_in_sdt(xsdt_addr, true).or_else(|| find_madt_in_sdt(rsdt_addr, false))?
    } else {
        find_madt_in_sdt(rsdt_addr, false)?
    };

    let mut apic_ids = Vec::new();
    // SAFETY: `madt_addr` apunta a una tabla ACPI `"APIC"` verificada en memoria física.
    let lapic_base = unsafe {
        let ptr = madt_addr as *const u8;
        let total_len = core::ptr::read_unaligned(ptr.add(4) as *const u32) as usize;
        let lapic_phys = core::ptr::read_unaligned(ptr.add(36) as *const u32) as u64;

        let mut offset = 44usize;
        while offset + 2 <= total_len.min(4096) {
            let entry_type = *ptr.add(offset);
            let record_len = *ptr.add(offset + 1) as usize;
            if record_len < 2 || offset + record_len > total_len {
                break;
            }
            // Tipo 0: Processor Local APIC
            if entry_type == 0 && record_len >= 8 {
                let apic_id = *ptr.add(offset + 3) as u32;
                let flags = core::ptr::read_unaligned(ptr.add(offset + 4) as *const u32);
                // Bit 0 = Processor Enabled, Bit 1 = Online Capable
                if (flags & 0x03) != 0 && !apic_ids.contains(&apic_id) {
                    apic_ids.push(apic_id);
                }
            }
            offset += record_len;
        }
        lapic_phys
    };

    Some((madt_addr, lapic_base, apic_ids))
}

// ============================================================================
// Trampolín SMP de 16 bits -> 64 bits en 0x8000 (Vector SIPI 0x08)
// ============================================================================

/// Código máquina del trampolín en `0x8000`:
/// - `0x8000..0x8047`: Entrada en Modo Real de 16 bits (`CS=0x0800, IP=0x0000`), carga `GDTR` en `[0x8120]`,
///   activa `CR4.PAE`, carga `CR3` desde `[0x8F00]`, activa `EFER.LME`, activa `CR0.PG | CR0.PE` y hace
///   `ljmpl 0x0008:0x00008048` directamente a Modo Largo de 64 bits.
/// - `0x8048..0x806D`: Código de 64 bits que configura `DS/ES/FS/GS/SS = 0x10`, carga `RSP` desde `[0x8F08]`,
///   carga `RAX` desde `[0x8F10]` (`ap_kernel_entry`) y ejecuta `call rax`.
const AP_TRAMPOLINE_CODE: [u8; 108] = [
    // --- [0x8000] 16-bit Real Mode Entry ---
    0xFA,                                           // 0x00: cli
    0xFC,                                           // 0x01: cld
    0x31, 0xC0,                                     // 0x02: xor ax, ax
    0x8E, 0xD8,                                     // 0x04: mov ds, ax (DS = 0x0000)
    0x8E, 0xC0,                                     // 0x06: mov es, ax
    0x8E, 0xD0,                                     // 0x08: mov ss, ax
    0x0F, 0x01, 0x16, 0x20, 0x81,                   // 0x0A: lgdt [0x8120]
    0x0F, 0x20, 0xE0,                               // 0x0F: mov eax, cr4
    0x66, 0x83, 0xC8, 0x20,                         // 0x12: or eax, 0x20 (PAE)
    0x0F, 0x22, 0xE0,                               // 0x16: mov cr4, eax
    0x66, 0xA1, 0x00, 0x8F,                         // 0x19: mov eax, dword ptr [0x8F00] (PML4 phys)
    0x0F, 0x22, 0xD8,                               // 0x1D: mov cr3, eax
    0x66, 0xB9, 0x80, 0x00, 0x00, 0xC0,             // 0x20: mov ecx, 0xC0000080 (IA32_EFER)
    0x0F, 0x32,                                     // 0x26: rdmsr
    0x66, 0x0D, 0x00, 0x01, 0x00, 0x00,             // 0x28: or eax, 0x00000100 (LME)
    0x0F, 0x30,                                     // 0x2E: wrmsr
    0x0F, 0x20, 0xC0,                               // 0x30: mov eax, cr0
    0x66, 0x0D, 0x01, 0x00, 0x00, 0x80,             // 0x33: or eax, 0x80000001 (PG | PE)
    0x0F, 0x22, 0xC0,                               // 0x39: mov cr0, eax
    0x66, 0xEA, 0x48, 0x80, 0x00, 0x00, 0x08, 0x00, // 0x3C: ljmpl 0x0008:0x00008048
    0x90, 0x90, 0x90, 0x90,                         // 0x44..0x47: nop padding

    // --- [0x8048] 64-bit Long Mode Entry ---
    0x66, 0xB8, 0x10, 0x00,                         // 0x48: mov ax, 0x0010
    0x8E, 0xD8,                                     // 0x4C: mov ds, ax
    0x8E, 0xC0,                                     // 0x4E: mov es, ax
    0x8E, 0xE0,                                     // 0x50: mov fs, ax
    0x8E, 0xE8,                                     // 0x52: mov gs, ax
    0x8E, 0xD0,                                     // 0x54: mov ss, ax
    0x48, 0x8B, 0x24, 0x25, 0x08, 0x8F, 0x00, 0x00, // 0x56: mov rsp, qword ptr [0x8F08]
    0x48, 0x8B, 0x04, 0x25, 0x10, 0x8F, 0x00, 0x00, // 0x5E: mov rax, qword ptr [0x8F10]
    0xFF, 0xD0,                                     // 0x66: call rax
    0xFA,                                           // 0x68: cli
    0xF4,                                           // 0x69: hlt
    0xEB, 0xFD,                                     // 0x6A: jmp $-1
];

/// Instala el trampolín de 16 bits, la GDT temporal y los punteros del kernel en la página `0x8000`.
fn install_trampoline(pml4_phys: u32) {
    // SAFETY: La página física `0x8000..0x8FFF` está reservada en `frame_allocator.rs` y libre de `UefiBootInfo` (movido a `0x9000`).
    unsafe {
        // 1. Limpiar la página completa 0x8000..0x8FFF
        core::ptr::write_bytes(SMP_TRAMPOLINE_ADDR as *mut u8, 0, PAGE_SIZE);

        // 2. Copiar código máquina 16->64 bits en 0x8000
        core::ptr::copy_nonoverlapping(
            AP_TRAMPOLINE_CODE.as_ptr(),
            SMP_TRAMPOLINE_ADDR as *mut u8,
            AP_TRAMPOLINE_CODE.len(),
        );

        // 3. Instalar GDT temporal de 64 bits en 0x8100 (Null, Code64 0x08, Data64 0x10)
        let gdt_ptr = TRAMPOLINE_GDT_ADDR as *mut u64;
        core::ptr::write_volatile(gdt_ptr.add(0), 0x0000_0000_0000_0000);
        core::ptr::write_volatile(gdt_ptr.add(1), 0x0020_9A00_0000_0000);
        core::ptr::write_volatile(gdt_ptr.add(2), 0x0000_9200_0000_0000);

        // 4. Instalar descriptor GDTR de 6 bytes en 0x8120 (limit = 23, base = 0x8100)
        core::ptr::write_unaligned(TRAMPOLINE_GDTR_ADDR as *mut u16, 23u16);
        core::ptr::write_unaligned((TRAMPOLINE_GDTR_ADDR + 2) as *mut u32, TRAMPOLINE_GDT_ADDR as u32);

        // 5. Escribir dirección física de PML4 y punto de entrada Rust (`ap_kernel_entry`)
        core::ptr::write_volatile(TRAMPOLINE_PML4_ADDR as *mut u32, pml4_phys);
        core::ptr::write_volatile(
            TRAMPOLINE_ENTRY_ADDR as *mut u64,
            ap_kernel_entry as *const () as usize as u64,
        );
    }
    fence(Ordering::SeqCst);
}

#[inline(always)]
fn delay_io_cycles(iterations: usize) {
    for _ in 0..iterations {
        // SAFETY: `io_wait()` escribe en el puerto de diagnóstico 0x80 (~1 µs por operación).
        unsafe {
            io_wait();
        }
    }
}

/// Espera a que el bit Delivery Status (bit 12) de `LAPIC_ICR_LOW` se limpie.
fn wait_icr_idle(smp: &SmpManager) {
    for _ in 0..100_000 {
        // SAFETY: Lectura MMIO de `LAPIC_ICR_LOW`.
        let icr = unsafe { smp.read_reg(LAPIC_ICR_LOW) };
        if (icr & (1 << 12)) == 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Inicializa el Local APIC del procesador principal (BSP), descubre todos los núcleos en la tabla
/// ACPI MADT y los despierta mediante el trampolín de 16 bits en `0x8000` (`INIT-SIPI-SIPI`).
pub fn init_smp() {
    // 1. Descubrir topología real en la tabla ACPI MADT ("APIC")
    let madt_info = discover_madt_cores();

    let mut smp = SMP.lock();

    // 2. Obtener dirección base del Local APIC desde el MSR 0x1B (o MADT)
    // SAFETY: `MSR_APIC_BASE` (0x1B) es un registro arquitectónico estándar x86_64 legible en Ring 0.
    let msr_apic = unsafe { read_msr(MSR_APIC_BASE) };
    let lapic_phys = msr_apic & 0xFFFF_F000;
    if lapic_phys != 0 {
        smp.lapic_base = lapic_phys;
    } else if let Some((_, madt_lapic, _)) = &madt_info {
        if *madt_lapic != 0 {
            smp.lapic_base = *madt_lapic;
        }
    }

    // 3. Habilitar Local APIC del BSP en el registro SVR (Vector 0xFF + Bit 8 = APIC Enable)
    // SAFETY: Acceso MMIO a los registros SVR, EOI y TPR del Local APIC cuya base física fue verificada.
    unsafe {
        smp.write_reg(LAPIC_EOI, 0);
        let svr = smp.read_reg(LAPIC_SVR);
        smp.write_reg(LAPIC_SVR, svr | 0x1FF);
        smp.write_reg(LAPIC_TPR, 0);
    }
    LAPIC_READY.store(true, Ordering::Release);

    // 4. Registrar el procesador principal (BSP)
    // SAFETY: Lectura MMIO del registro `LAPIC_ID` (offset 0x20) con el Local APIC ya habilitado.
    let bsp_id = unsafe { (smp.read_reg(LAPIC_ID) >> 24) & 0xFF };
    smp.bsp_apic_id = bsp_id;
    smp.cores.clear();
    smp.cores.push(CoreInfo {
        core_id: 0,
        apic_id: bsp_id,
        is_bsp: true,
        is_online: true,
        stack_top: 0,
        tasks_executed: 0,
    });

    // 5. Configurar el Temporizador Local APIC en modo Periódico en el BSP
    init_lapic_timer(&smp, 100);

    // 6.Si la tabla ACPI MADT reportó procesadores secundarios (APs), despertarlos uno por uno
    if let Some((madt_addr, _, apic_ids)) = madt_info {
        smp.madt_detected = true;
        smp.madt_addr = madt_addr;
        wake_secondary_cores(&mut smp, &apic_ids);
    } else {
        smp.madt_detected = false;
        smp.smp_active = false;
    }
}

/// Configura el temporizador de alta resolución del Local APIC
fn init_lapic_timer(smp: &SmpManager, _freq_hz: u32) {
    // SAFETY: Escritura MMIO en los registros de configuración del temporizador del Local APIC (`DIV`, `LVT_TIMER`, `TIMER_INIT`).
    unsafe {
        smp.write_reg(LAPIC_TIMER_DIV, 0x03);
        let timer_vector = 48 | 0x20000;
        smp.write_reg(LAPIC_LVT_TIMER, timer_vector);
        smp.write_reg(LAPIC_TIMER_INIT, 10_000_000);
    }
}

/// Despierta los Application Processors (APs) descubiertos en la tabla ACPI MADT:
/// 1. Instala el trampolín de 16 bits en `0x8000` (vector `0x08`).
/// 2. Asigna una pila física independiente de 16 KiB por cada AP.
/// 3. Envía la secuencia `INIT` -> espera -> `SIPI` (y `2º SIPI` si es necesario).
/// 4. Verifica confirmación atómica real en `AP_BOOT_FLAG` antes de marcar el núcleo `is_online = true`.
fn wake_secondary_cores(smp: &mut SmpManager, madt_apic_ids: &[u32]) {
    let pml4_phys = paging::kernel_cr3() as u32;
    install_trampoline(pml4_phys);

    let bsp_id = smp.bsp_apic_id;
    let mut next_core_idx = 1u32;
    let mut online_aps = 0u32;

    for &apic_id in madt_apic_ids {
        if apic_id == bsp_id {
            continue;
        }

        // Reservar 4 marcos contiguos (16 KiB) para la pila privada de este AP
        let stack_base = match FRAME_ALLOCATOR.allocate_contiguous(4) {
            Some(addr) => addr,
            None => break,
        };
        let stack_top = (stack_base + 4 * PAGE_SIZE) as u64;

        AP_BOOT_FLAG.store(false, Ordering::SeqCst);
        // SAFETY: Actualización del buzón del trampolín en `0x8F08` y `0x8F18` antes de enviar `INIT-SIPI`.
        unsafe {
            core::ptr::write_volatile(TRAMPOLINE_STACK_ADDR as *mut u64, stack_top);
            core::ptr::write_volatile(TRAMPOLINE_ACK_ADDR as *mut u32, 0);
        }
        fence(Ordering::SeqCst);

        // 1. Enviar INIT IPI (Assert) al APIC ID destino
        // SAFETY: Escritura MMIO en `LAPIC_ICR_HIGH` y `LAPIC_ICR_LOW` para el APIC destino.
        unsafe {
            smp.write_reg(LAPIC_ESR, 0);
            smp.write_reg(LAPIC_ICR_HIGH, apic_id << 24);
            smp.write_reg(LAPIC_ICR_LOW, ICR_TYPE_INIT | ICR_LEVEL_ASSERT);
        }
        wait_icr_idle(smp);
        delay_io_cycles(2_000);

        // 2. Enviar primer Startup IPI (SIPI) con vector 0x08 (dirección física 0x8000)
        // SAFETY: Envío de comando SIPI al registro `LAPIC_ICR` por MMIO.
        unsafe {
            smp.write_reg(LAPIC_ESR, 0);
            smp.write_reg(LAPIC_ICR_HIGH, apic_id << 24);
            smp.write_reg(LAPIC_ICR_LOW, ICR_TYPE_STARTUP | SMP_SIPI_VECTOR);
        }
        wait_icr_idle(smp);

        // Esperar confirmación atómica tras el primer SIPI
        let mut awake = false;
        for _ in 0..80_000 {
            if AP_BOOT_FLAG.load(Ordering::Acquire) {
                awake = true;
                break;
            }
            core::hint::spin_loop();
        }

        // 3. Si aún no despertó, enviar segundo SIPI según especificación Intel MP
        if !awake {
            delay_io_cycles(500);
            // SAFETY: Segundo envío de `SIPI` al APIC destino si el primero aún no despertó al AP.
            unsafe {
                smp.write_reg(LAPIC_ESR, 0);
                smp.write_reg(LAPIC_ICR_HIGH, apic_id << 24);
                smp.write_reg(LAPIC_ICR_LOW, ICR_TYPE_STARTUP | SMP_SIPI_VECTOR);
            }
            wait_icr_idle(smp);

            for _ in 0..250_000 {
                if AP_BOOT_FLAG.load(Ordering::Acquire) {
                    awake = true;
                    break;
                }
                core::hint::spin_loop();
            }
        }

        if awake {
            online_aps += 1;
        }

        smp.cores.push(CoreInfo {
            core_id: next_core_idx,
            apic_id,
            is_bsp: false,
            is_online: awake,
            stack_top,
            tasks_executed: 0,
        });
        next_core_idx += 1;
    }

    smp.smp_active = online_aps > 0;
}

/// Lectura segura de registro MSR (Model-Specific Register)
#[inline(always)]
// SAFETY: El llamador debe garantizar que `msr` es una dirección MSR válida en la arquitectura actual y se ejecuta en Ring 0.
pub unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nomem, nostack)
    );
    ((high as u64) << 32) | (low as u64)
}

/// Escritura segura en registro MSR
#[inline(always)]
// SAFETY: El llamador debe garantizar que `msr` y `value` son válidos para la arquitectura y que se ejecuta en Ring 0.
pub unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nomem, nostack)
    );
}

/// Punto de entrada de 64 bits ejecutado por cada Application Processor (AP) tras salir del trampolín en `0x8000`.
/// Nota importante: NO adquiere `SMP.lock()` porque el BSP mantiene `SMP.lock()` mientras espera `AP_BOOT_FLAG`.
#[no_mangle]
pub extern "C" fn ap_kernel_entry() -> ! {
    // 1. Cargar la GDT e IDT definitivas del kernel en este núcleo secundario
    crate::gdt::load_gdt_on_ap();
    crate::interrupts::load_idt_on_ap();

    // 2. Habilitar el Local APIC propio del núcleo secundario directamente por MMIO
    // SAFETY: Cada núcleo tiene su propio Local APIC mapeado en `DEFAULT_LAPIC_BASE` (0xFEE00000).
    let apic_id = unsafe {
        let svr_ptr = (DEFAULT_LAPIC_BASE + LAPIC_SVR as u64) as *mut u32;
        let tpr_ptr = (DEFAULT_LAPIC_BASE + LAPIC_TPR as u64) as *mut u32;
        let eoi_ptr = (DEFAULT_LAPIC_BASE + LAPIC_EOI as u64) as *mut u32;
        let id_ptr  = (DEFAULT_LAPIC_BASE + LAPIC_ID as u64) as *const u32;

        let svr = core::ptr::read_volatile(svr_ptr);
        core::ptr::write_volatile(svr_ptr, svr | 0x1FF);
        core::ptr::write_volatile(tpr_ptr, 0);
        core::ptr::write_volatile(eoi_ptr, 0);

        let id = (core::ptr::read_volatile(id_ptr) >> 24) & 0xFF;
        core::ptr::write_volatile(TRAMPOLINE_APIC_ID_ADDR as *mut u32, id);
        core::ptr::write_volatile(TRAMPOLINE_ACK_ADDR as *mut u32, 1);
        id
    };
    let _ = apic_id;

    // 3. Notificar atómicamente al BSP que este AP ya está online en modo de 64 bits
    AP_ONLINE_COUNT.fetch_add(1, Ordering::SeqCst);
    AP_BOOT_FLAG.store(true, Ordering::Release);

    // 4. Bucle de reposo del AP con baja latencia
    loop {
        // SAFETY: `hlt` suspende el núcleo secundario en Ring 0 hasta recibir una interrupción.
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
