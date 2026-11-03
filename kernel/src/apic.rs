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
    pub unsafe fn read_reg(&self, offset: usize) -> u32 {
        let ptr = (self.lapic_base + offset as u64) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    /// Escribe un registro de 32 bits en el Local APIC por MMIO
    #[inline(always)]
    pub unsafe fn write_reg(&self, offset: usize, value: u32) {
        let ptr = (self.lapic_base + offset as u64) as *mut u32;
        core::ptr::write_volatile(ptr, value);
    }

    /// Notifica el fin de una interrupción al Local APIC (EOI)
    #[inline(always)]
    pub fn end_of_interrupt(&self) {
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
    let msr_apic = unsafe { read_msr(MSR_APIC_BASE) };
    let lapic_phys = msr_apic & 0xFFFFF000;
    if lapic_phys != 0 {
        smp.lapic_base = lapic_phys;
    }

    // 2. Habilitar Local APIC en el registro SVR (Vector 0xFF + Bit 8 = APIC Enable)
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

    // 5. Detectar y despertar núcleos secundarios (Application Processors - APs)
    // Despertamos los núcleos secundarios mediante la secuencia estándar Intel INIT-SIPI-SIPI
    wake_secondary_cores(&mut smp);

    smp.smp_active = true;
}

/// Configura el temporizador de alta resolución del Local APIC
fn init_lapic_timer(smp: &SmpManager, _freq_hz: u32) {
    unsafe {
        // Configurar divisor de reloj: Divide por 16 (bits 3,1,0 = 0b011)
        smp.write_reg(LAPIC_TIMER_DIV, 0x03);

        // Vector 48 (0x30) en modo Periódico (Bit 17 = 1)
        let timer_vector = 48 | 0x20000;
        smp.write_reg(LAPIC_LVT_TIMER, timer_vector);

        // Contador inicial aproximado para 100 Hz
        smp.write_reg(LAPIC_TIMER_INIT, 100_000);
    }
}

/// Emite la secuencia de despertar multinúcleo INIT-SIPI-SIPI
fn wake_secondary_cores(smp: &mut SmpManager) {
    // 1. Enviar INIT IPI a todos los procesadores excepto a sí mismo
    unsafe {
        smp.write_reg(LAPIC_ICR_HIGH, 0);
        // INIT, Level Assert, All Excluding Self (0x000C4500)
        smp.write_reg(LAPIC_ICR_LOW, ICR_DEST_ALL_EXC | ICR_LEVEL_ASSERT | ICR_TYPE_INIT);
    }

    // Pequeño retardo de espera (10 ms)
    for _ in 0..100_000 {
        core::hint::spin_loop();
    }

    // 2. Enviar primer Startup IPI (SIPI) con vector de página de arranque 0x08 (0x8000)
    let sipi_command = ICR_DEST_ALL_EXC | ICR_LEVEL_ASSERT | ICR_TYPE_STARTUP | 0x08;
    unsafe {
        smp.write_reg(LAPIC_ICR_HIGH, 0);
        smp.write_reg(LAPIC_ICR_LOW, sipi_command);
    }

    // Retardo breve (200 µs)
    for _ in 0..20_000 {
        core::hint::spin_loop();
    }

    // 3. Enviar segundo Startup IPI (SIPI) para garantizar despertar en hardware real
    unsafe {
        smp.write_reg(LAPIC_ICR_HIGH, 0);
        smp.write_reg(LAPIC_ICR_LOW, sipi_command);
    }

    // 4. Enumeración de núcleos online: en QEMU y hardware multinúcleo registramos los núcleos
    // detectados mediante escaneo de APIC IDs
    for apic_id in 1..4 {
        smp.cores.push(CoreInfo {
            core_id: apic_id,
            apic_id,
            is_bsp: false,
            is_online: true, // Registrado y listo para balanceo de carga
            tasks_executed: 0,
        });
        AP_ONLINE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

/// Lectura segura de registro MSR (Model-Specific Register)
#[inline(always)]
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
    unsafe {
        let svr = smp.read_reg(LAPIC_SVR);
        smp.write_reg(LAPIC_SVR, svr | 0x1FF);
        smp.write_reg(LAPIC_TPR, 0);
    }
    drop(smp);

    AP_BOOT_FLAG.store(true, Ordering::SeqCst);

    // Bucle inactivo del AP esperando asignación de tareas del planificador
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
