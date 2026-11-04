// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Local APIC (Advanced Programmable Interrupt Controller),
//              SMP Multi-Core Startup (INIT-SIPI-SIPI) & LAPIC Preemptive Timer
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use crate::sync::SpinMutex;

pub const DEFAULT_LAPIC_BASE: u64 = 0xFEE00000;
pub const MSR_APIC_BASE: u32       = 0x0000001B;

// --- Offsets de Registros MMIO del Local APIC ---
pub const LAPIC_ID: usize        = 0x020; // Local APIC ID (bits 31:24)
pub const LAPIC_VERSION: usize   = 0x030; // Local APIC Version
pub const LAPIC_TPR: usize       = 0x080; // Task Priority Register
pub const LAPIC_APR: usize       = 0x090; // Arbitration Priority Register
pub const LAPIC_PPR: usize       = 0x0A0; // Processor Priority Register
pub const LAPIC_EOI: usize       = 0x0B0; // End of Interrupt Register
pub const LAPIC_LDR: usize       = 0x0D0; // Logical Destination Register
pub const LAPIC_DFR: usize       = 0x0E0; // Destination Format Register
pub const LAPIC_SVR: usize       = 0x0F0; // Spurious Interrupt Vector Register
pub const LAPIC_ESR: usize       = 0x280; // Error Status Register
pub const LAPIC_ICR_LOW: usize   = 0x300; // Interrupt Command Register (Bits 0..31)
pub const LAPIC_ICR_HIGH: usize  = 0x310; // Interrupt Command Register (Bits 32..63)
pub const LAPIC_LVT_TIMER: usize = 0x320; // LVT Timer Register
pub const LAPIC_LVT_LINT0: usize = 0x350; // LVT LINT0 Register
pub const LAPIC_LVT_LINT1: usize = 0x360; // LVT LINT1 Register
pub const LAPIC_LVT_ERROR: usize = 0x370; // LVT Error Register
pub const LAPIC_TIMER_INIT: usize= 0x380; // Timer Initial Count Register
pub const LAPIC_TIMER_CURR: usize= 0x390; // Timer Current Count Register
pub const LAPIC_TIMER_DIV: usize = 0x3E0; // Timer Divide Configuration Register

// --- Constantes de IPI (Inter-Processor Interrupt) ---
pub const ICR_TYPE_FIXED: u32    = 0x00000000;
pub const ICR_TYPE_INIT: u32     = 0x00000500; // INIT IPI
pub const ICR_TYPE_STARTUP: u32  = 0x00000600; // Startup IPI (SIPI)
pub const ICR_LEVEL_ASSERT: u32  = 0x00004000;
pub const ICR_LEVEL_DEASSERT: u32= 0x00000000;
pub const ICR_DEST_ALL_EXC: u32  = 0x000C0000; // All Excluding Self

#[derive(Debug, Clone, Copy)]
pub struct CoreInfo {
    pub core_id: u32,
    pub apic_id: u32,
    pub is_bsp: bool, // Bootstrap Processor (Core 0)
    pub is_online: bool,
    pub tasks_executed: u64,
}

pub struct SmpManager {
    pub lapic_base: u64,
    pub bsp_apic_id: u32,
    pub cores: Vec<CoreInfo>,
    pub smp_active: bool,
    pub timer_ticks: u64,
}

impl SmpManager {
    pub const fn new() -> Self {
        Self {
            lapic_base: DEFAULT_LAPIC_BASE,
            bsp_apic_id: 0,
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

/// Inicializa el Local APIC del procesador actual y despierta los núcleos secundarios (APs)
pub fn init_smp() {
    let mut smp = SMP.lock();

    // 1. Obtener dirección base del Local APIC desde el MSR 0x1B
    // SAFETY: `MSR_APIC_BASE` (0x1B) es un registro arquitectónico estándar x86_64 legible en Ring 0.
    let msr_apic = unsafe { read_msr(MSR_APIC_BASE) };
    let lapic_phys = msr_apic & 0xFFFFF000;
    if lapic_phys != 0 {
        smp.lapic_base = lapic_phys;
    }

    // 2. Habilitar Local APIC en el registro SVR (Vector 0xFF + Bit 8 = APIC Enable)
    // SAFETY: Acceso MMIO a los registros SVR, EOI y TPR del Local APIC cuya base física fue verificada en `MSR_APIC_BASE`.
    unsafe {
        // Enviar EOI pendiente si existiera
        smp.write_reg(LAPIC_EOI, 0);

        // Habilitar APIC: Vector espurio 0xFF con bit 8 encendido (0x1FF)
        let svr = smp.read_reg(LAPIC_SVR);
        smp.write_reg(LAPIC_SVR, svr | 0x1FF);

        // Ajustar TPR a 0 para aceptar todas las prioridades de interrupción
        smp.write_reg(LAPIC_TPR, 0);
    }

    // 3. Registrar el procesador principal (BSP)
    // SAFETY: Lectura MMIO del registro `LAPIC_ID` (offset 0x20) con el Local APIC ya habilitado.
    let bsp_id = unsafe { (smp.read_reg(LAPIC_ID) >> 24) & 0xFF };
    smp.bsp_apic_id = bsp_id;
    smp.cores.clear();
    smp.cores.push(CoreInfo {
        core_id: 0,
        apic_id: bsp_id,
        is_bsp: true,
        is_online: true,
        tasks_executed: 0,
    });

    // 4. Configurar el Temporizador Local APIC en modo Periódico
    init_lapic_timer(&smp, 100); // 100 Hz (tick cada 10ms)

    // 5. [FASE 3 DESHABILITADO] Arranque SMP multinucleo neutralizado
    // El SIPI apuntaba a 0x8000, direccion que el cargador UEFI usa para UefiBootInfo.
    // Ademas no existe trampolines de 16 bits, stacks por AP, ni confirmacion atomica.
    // Cuando se implemente correctamente debe:
    //   a) Leer la tabla MADT (ACPI) para obtener los APIC IDs reales
    //   b) Copiar el trampolines de arranque de 16 bits a 0x7000 (antes de UefiBootInfo)
    //   c) Preparar stack independiente por AP
    //   d) Esperar confirmacion atomica AP_ONLINE_COUNT antes de continuar
    // Por ahora BeaverOS funciona en modo uniprocessor (1 core BSP).
    // NO llamar wake_secondary_cores hasta que lo anterior este listo.
    // wake_secondary_cores(&mut smp);  // <-- DESHABILITADO: Fase 4-SMP pendiente

    smp.smp_active = false; // Honesto: SMP no activo hasta implementacion real
}

/// Configura el temporizador de alta resolución del Local APIC
fn init_lapic_timer(smp: &SmpManager, _freq_hz: u32) {
    // SAFETY: Escritura MMIO en los registros de configuración del temporizador del Local APIC (`DIV`, `LVT_TIMER`, `TIMER_INIT`).
    unsafe {
        // Configurar divisor de reloj: Divide por 16 (bits 3,1,0 = 0b011)
        smp.write_reg(LAPIC_TIMER_DIV, 0x03);

        // Vector 48 (0x30) en modo Periódico (Bit 17 = 1)
        let timer_vector = 48 | 0x20000;
        smp.write_reg(LAPIC_LVT_TIMER, timer_vector);

        // Contador inicial aproximado para ~100 Hz sin saturar emulacion TCG
        smp.write_reg(LAPIC_TIMER_INIT, 10_000_000);
    }
}

/// [DESHABILITADO - Fase 4-SMP pendiente]
/// Emitia la secuencia INIT-SIPI-SIPI para despertar Application Processors.
/// RIESGOS identificados por auditoria (2026-11-01):
///   - Apuntaba a 0x8000 donde el cargador UEFI escribe UefiBootInfo -> colision de memoria
///   - No existe trampolines de 16 bits en 0x8000 -> APs ejecutan datos como codigo
///   - No verificaba confirmacion atomica de AP online -> marca online sin comprobar
///   - Enumeracion fija de 3 APs (IDs 1-3) sin leer MADT/ACPI real
/// La funcion existe pero es NO-OP hasta reimplementacion correcta.
#[allow(dead_code)]
fn wake_secondary_cores(_smp: &mut SmpManager) {
    // NO-OP: Deshabilitado en Fase 3 por auditoria de seguridad.
    // Reimplementar usando:
    //   1. MADT (ACPI) para APIC IDs reales
    //   2. Trampolines 16-bit en direccion libre (< 0x7000, no 0x8000)
    //   3. Stack dedicado por AP
    //   4. AP_ONLINE_COUNT atomico con timeout de confirmacion
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

/// Trampoline de arranque secundario ejecutado por los núcleos APs al despertar
#[no_mangle]
pub extern "C" fn ap_kernel_entry() -> ! {
    // Configurar Local APIC del núcleo secundario
    let smp = SMP.lock();
    // SAFETY: Configuración de los registros MMIO `LAPIC_SVR` y `LAPIC_TPR` del núcleo secundario actual.
    unsafe {
        let svr = smp.read_reg(LAPIC_SVR);
        smp.write_reg(LAPIC_SVR, svr | 0x1FF);
        smp.write_reg(LAPIC_TPR, 0);
    }
    drop(smp);

    AP_BOOT_FLAG.store(true, Ordering::SeqCst);

    // Bucle inactivo del AP esperando asignación de tareas del planificador
    loop {
        // SAFETY: `hlt` suspende el núcleo secundario en Ring 0 hasta recibir una interrupción.
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
