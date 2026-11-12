// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Real Hardware Device Profile Detection (ACPI FADT, SMBIOS, DTB, Peripherals)
// ============================================================================

use crate::drivers::cec::HDMI_CEC;
use crate::drivers::touchscreen::TOUCHSCREEN;
use crate::drivers::vbe::VBE;

/// Clasificación real de dispositivo detectada a partir de tablas de firmware y periféricos físicos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Server,
    DesktopPc,
    LaptopMobile,
    SmartTv,
    SmartApplianceKiosk,
    EmbeddedArm64,
    Unknown,
}

impl DeviceClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceClass::Server => "Servidor (Rack / Headless / Enterprise)",
            DeviceClass::DesktopPc => "PC de Escritorio / Workstation",
            DeviceClass::LaptopMobile => "Laptop / Portatil / Tablet",
            DeviceClass::SmartTv => "Smart TV / Receptor HDMI-CEC",
            DeviceClass::SmartApplianceKiosk => "Electrodomestico Smart / Kiosco Tactil",
            DeviceClass::EmbeddedArm64 => "Dispositivo Embebido ARM64",
            DeviceClass::Unknown => "Dispositivo Generico Desconocido",
        }
    }
}

/// Perfil completo de hardware real detectado en tiempo de ejecución.
#[derive(Debug, Clone, Copy)]
pub struct HardwareProfile {
    pub class: DeviceClass,
    pub acpi_pm_profile: Option<u8>,
    pub smbios_chassis_type: Option<u8>,
    pub has_display: bool,
    pub has_cec: bool,
    pub has_touch: bool,
    pub cpu_cores: usize,
    pub source: &'static str,
}

/// Lee `Preferred_PM_Profile` (offset 45) de la tabla ACPI FADT (`"FACP"`).
#[cfg(target_arch = "x86_64")]
pub fn probe_fadt_pm_profile() -> Option<u8> {
    let fadt_addr = crate::apic::find_acpi_table(b"FACP")?;
    if fadt_addr < 0x1000 || fadt_addr >= 0xFFE0_0000 {
        return None;
    }
    // SAFETY: `fadt_addr` fue validada en RSDT/XSDT y verificamos su longitud antes de leer el offset 45.
    unsafe {
        let ptr = fadt_addr as *const u8;
        let total_len = core::ptr::read_unaligned(ptr.add(4) as *const u32) as usize;
        if total_len < 46 || total_len > 65536 {
            return None;
        }
        Some(*ptr.add(45))
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn probe_fadt_pm_profile() -> Option<u8> {
    None
}

#[cfg(target_arch = "x86_64")]
fn smbios_checksum_ok(ptr: *const u8, len: usize) -> bool {
    if len == 0 || len > 64 {
        return false;
    }
    let mut sum: u8 = 0;
    // SAFETY: El llamador garantiza que `ptr` apunta a `0xF0000..0xFFFFF` con `len <= 64`.
    unsafe {
        for i in 0..len {
            sum = sum.wrapping_add(*ptr.add(i));
        }
    }
    sum == 0
}

#[cfg(target_arch = "x86_64")]
fn walk_smbios_type3(table_addr: u64, table_len: usize) -> Option<u8> {
    if table_addr < 0x1000 || table_addr >= 0xFFE0_0000 || table_len < 8 || table_len > 65536 {
        return None;
    }
    // SAFETY: Recorrido acotado por `table_len` dentro de la tabla SMBIOS en memoria física.
    unsafe {
        let base = table_addr as *const u8;
        let mut offset = 0usize;
        while offset + 4 <= table_len {
            let stype = *base.add(offset);
            let slen = *base.add(offset + 1) as usize;
            if slen < 4 || offset + slen > table_len {
                break;
            }
            if stype == 3 && slen >= 6 {
                return Some(*base.add(offset + 5) & 0x7F);
            }
            if stype == 127 {
                break;
            }
            let mut str_pos = offset + slen;
            while str_pos + 1 < table_len {
                if *base.add(str_pos) == 0 && *base.add(str_pos + 1) == 0 {
                    str_pos += 2;
                    break;
                }
                str_pos += 1;
            }
            if str_pos <= offset + slen {
                break;
            }
            offset = str_pos;
        }
    }
    None
}

/// Escanea el rango BIOS/UEFI `0xF0000..0xFFFFF` buscando SMBIOS (`_SM_` o `_SM3_`) y lee Type 3 Chassis Type.
#[cfg(target_arch = "x86_64")]
pub fn probe_smbios_chassis_type() -> Option<u8> {
    let mut addr = 0x000F_0000u64;
    while addr < 0x0010_0000 {
        // SAFETY: Lectura alineada de 16 bytes en área estándar SMBIOS de memoria física baja.
        unsafe {
            let ptr = addr as *const u8;
            let sig4 = core::slice::from_raw_parts(ptr, 4);
            if sig4 == b"_SM_" {
                let ep_len = *ptr.add(5) as usize;
                if ep_len >= 0x1F && smbios_checksum_ok(ptr, ep_len) {
                    let table_len = core::ptr::read_unaligned(ptr.add(0x16) as *const u16) as usize;
                    let table_addr = core::ptr::read_unaligned(ptr.add(0x18) as *const u32) as u64;
                    if let Some(chassis) = walk_smbios_type3(table_addr, table_len) {
                        return Some(chassis);
                    }
                }
            } else {
                let sig5 = core::slice::from_raw_parts(ptr, 5);
                if sig5 == b"_SM3_" {
                    let ep_len = *ptr.add(6) as usize;
                    if ep_len >= 0x18 && smbios_checksum_ok(ptr, ep_len) {
                        let table_len = core::ptr::read_unaligned(ptr.add(0x0C) as *const u32) as usize;
                        let table_addr = core::ptr::read_unaligned(ptr.add(0x10) as *const u64);
                        if let Some(chassis) = walk_smbios_type3(table_addr, table_len) {
                            return Some(chassis);
                        }
                    }
                }
            }
        }
        addr += 16;
    }
    None
}

#[cfg(not(target_arch = "x86_64"))]
pub fn probe_smbios_chassis_type() -> Option<u8> {
    None
}

/// Detecta el perfil real del dispositivo combinando ACPI FADT, SMBIOS Type 3, DTB y periféricos físicos.
pub fn detect_device_profile() -> HardwareProfile {
    let acpi_pm_profile = probe_fadt_pm_profile();
    let smbios_chassis_type = probe_smbios_chassis_type();

    #[cfg(target_arch = "x86_64")]
    let has_display = VBE.lock().is_active;
    #[cfg(target_arch = "aarch64")]
    let has_display = {
        let vbe_on = VBE.lock().is_active;
        let dtb_fb = crate::arch::aarch64::dtb::DTB_STATE.lock().fb.present;
        vbe_on || dtb_fb
    };

    let has_cec = HDMI_CEC.lock().hw_present;
    let has_touch = TOUCHSCREEN.lock().hw_present;

    #[cfg(target_arch = "x86_64")]
    let cpu_cores = crate::apic::SMP.lock().cores.len().max(1);
    #[cfg(not(target_arch = "x86_64"))]
    let cpu_cores = 1;

    let (class, source) = if has_cec {
        (DeviceClass::SmartTv, "HDMI-CEC")
    } else if has_touch
        && (matches!(acpi_pm_profile, Some(6))
            || matches!(smbios_chassis_type, Some(0x1E | 0x22 | 0x23))
            || !cfg!(target_arch = "x86_64"))
    {
        (DeviceClass::SmartApplianceKiosk, "Touchscreen+Firmware")
    } else if matches!(acpi_pm_profile, Some(4 | 5 | 7)) {
        (DeviceClass::Server, "ACPI-FADT")
    } else if matches!(smbios_chassis_type, Some(0x11 | 0x17 | 0x19 | 0x1C | 0x1D)) {
        (DeviceClass::Server, "SMBIOS-Type3")
    } else if matches!(acpi_pm_profile, Some(2 | 8)) {
        (DeviceClass::LaptopMobile, "ACPI-FADT")
    } else if matches!(
        smbios_chassis_type,
        Some(0x08 | 0x09 | 0x0A | 0x0E | 0x1E | 0x1F | 0x20)
    ) {
        (DeviceClass::LaptopMobile, "SMBIOS-Type3")
    } else if matches!(acpi_pm_profile, Some(6)) || matches!(smbios_chassis_type, Some(0x22 | 0x23)) {
        (DeviceClass::SmartApplianceKiosk, "Firmware-Appliance")
    } else if !has_display {
        (DeviceClass::Server, "Headless-Display")
    } else if cfg!(target_arch = "aarch64") {
        (DeviceClass::EmbeddedArm64, "DTB-ARM64")
    } else if matches!(acpi_pm_profile, Some(1 | 3)) {
        (DeviceClass::DesktopPc, "ACPI-FADT")
    } else if matches!(
        smbios_chassis_type,
        Some(0x03 | 0x04 | 0x05 | 0x06 | 0x07 | 0x0D | 0x0F)
    ) {
        (DeviceClass::DesktopPc, "SMBIOS-Type3")
    } else {
        (DeviceClass::DesktopPc, "Default-Display")
    };

    HardwareProfile {
        class,
        acpi_pm_profile,
        smbios_chassis_type,
        has_display,
        has_cec,
        has_touch,
        cpu_cores,
        source,
    }
}
