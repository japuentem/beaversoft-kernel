// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: VESA / Bochs VBE Graphics Driver (Linear Framebuffer LFB)
// ============================================================================

use crate::io::{inw, outw};
use crate::sync::SpinMutex;
use crate::drivers::pci::find_vga_device;

const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
const VBE_DISPI_IOPORT_DATA: u16  = 0x01CF;

const VBE_DISPI_INDEX_ID: u16          = 0;
const VBE_DISPI_INDEX_XRES: u16        = 1;
const VBE_DISPI_INDEX_YRES: u16        = 2;
const VBE_DISPI_INDEX_BPP: u16         = 3;
const VBE_DISPI_INDEX_ENABLE: u16      = 4;
#[allow(dead_code)]
const VBE_DISPI_INDEX_BANK: u16        = 5;
#[allow(dead_code)]
const VBE_DISPI_INDEX_VIRT_WIDTH: u16  = 6;
#[allow(dead_code)]
const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 7;
#[allow(dead_code)]
const VBE_DISPI_INDEX_X_OFFSET: u16    = 8;
#[allow(dead_code)]
const VBE_DISPI_INDEX_Y_OFFSET: u16    = 9;
pub const VBE_DISPI_DISABLED: u16    = 0x00;
pub const VBE_DISPI_ENABLED: u16     = 0x01;
pub const VBE_DISPI_LFB_ENABLED: u16 = 0x40;
#[allow(dead_code)]
pub const VBE_DISPI_NOCLEARMEM: u16  = 0x80;

pub const DEFAULT_LFB_PHYSICAL_ADDR: u64 = 0xFD000000;

pub const UEFI_BOOT_INFO_ADDR: u64 = 0x9000;
pub const UEFI_BOOT_MAGIC: u32 = 0xBEA0B007;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UefiBootInfo {
    pub magic: u32,
    pub fb_base: u64,
    pub fb_size: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_pixels_per_scanline: u32,
    pub mem_map_addr: u64,
    pub mem_map_size: u64,
    pub mem_desc_size: u64,
}

impl UefiBootInfo {
    pub fn get() -> Option<&'static UefiBootInfo> {
        let ptr = UEFI_BOOT_INFO_ADDR as *const UefiBootInfo;
        unsafe {
            if (*ptr).magic == UEFI_BOOT_MAGIC {
                Some(&*ptr)
            } else {
                None
            }
        }
    }
}

pub struct VbeGraphics {
    pub is_active: bool,
    pub width: usize,
    pub height: usize,
    pub bpp: usize,
    pub lfb_address: u64,
}

impl VbeGraphics {
    pub const fn new() -> Self {
        Self {
            is_active: false,
            width: 0,
            height: 0,
            bpp: 0,
            lfb_address: DEFAULT_LFB_PHYSICAL_ADDR,
        }
    }

    fn write_reg(&self, index: u16, value: u16) {
        unsafe {
            outw(VBE_DISPI_IOPORT_INDEX, index);
            outw(VBE_DISPI_IOPORT_DATA, value);
        }
    }

    fn read_reg(&self, index: u16) -> u16 {
        unsafe {
            outw(VBE_DISPI_IOPORT_INDEX, index);
            inw(VBE_DISPI_IOPORT_DATA)
        }
    }

    /// Comprueba si la extensión gráfica VBE/Bochs o el Framebuffer UEFI está disponible
    pub fn is_available(&self) -> bool {
        if UefiBootInfo::get().is_some() {
            return true;
        }
        let id = self.read_reg(VBE_DISPI_INDEX_ID);
        id >= 0xB0C0 && id <= 0xB0C6
    }

    /// Detecta la dirección física del Linear Framebuffer (LFB) consultando UEFI o el bus PCI
    pub fn detect_lfb_address(&mut self) -> u64 {
        if let Some(info) = UefiBootInfo::get() {
            self.lfb_address = info.fb_base;
            return self.lfb_address;
        }
        if let Some(dev) = find_vga_device() {
            if dev.bar0 != 0 {
                self.lfb_address = dev.bar0 as u64;
                return self.lfb_address;
            }
        }
        self.lfb_address = DEFAULT_LFB_PHYSICAL_ADDR;
        self.lfb_address
    }

    /// Conmuta el hardware al modo gráfico de alta resolución especificado
    pub fn set_mode(&mut self, width: usize, height: usize, bpp: usize) -> Result<(), &'static str> {
        if let Some(info) = UefiBootInfo::get() {
            self.lfb_address = info.fb_base;
            self.is_active = true;
            self.width = if info.fb_pixels_per_scanline > 0 {
                info.fb_pixels_per_scanline as usize
            } else {
                info.fb_width as usize
            };
            self.height = info.fb_height as usize;
            self.bpp = 32;
            return Ok(());
        }

        if !self.is_available() {
            return Err("Dispositivo VBE / Bochs Graphics no disponible en este hardware");
        }

        self.detect_lfb_address();

        self.write_reg(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);
        self.write_reg(VBE_DISPI_INDEX_XRES, width as u16);
        self.write_reg(VBE_DISPI_INDEX_YRES, height as u16);
        self.write_reg(VBE_DISPI_INDEX_BPP, bpp as u16);
        self.write_reg(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED);

        self.is_active = true;
        self.width = width;
        self.height = height;
        self.bpp = bpp;

        Ok(())
    }

    /// Desactiva el modo gráfico y regresa a la consola de texto estándar VGA 80x25
    pub fn disable(&mut self) {
        if self.is_active {
            self.write_reg(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);
            self.is_active = false;
        }
    }

    /// Ajusta el desplazamiento Y del framebuffer para hardware page flipping
    pub fn set_y_offset(&self, y_offset: usize) {
        if UefiBootInfo::get().is_none() && self.is_active {
            self.write_reg(VBE_DISPI_INDEX_Y_OFFSET, y_offset as u16);
        }
    }

    /// Puntero directo al búfer de video lineal (RGBA / 32 bits por píxel)
    #[inline(always)]
    pub fn lfb_ptr(&self) -> *mut u32 {
        self.lfb_address as *mut u32
    }

    /// Dibuja un píxel individual de 32 bits (0x00RRGGBB) en las coordenadas (x, y)
    #[inline(always)]
    pub fn put_pixel(&self, x: usize, y: usize, color: u32) {
        if !self.is_active || x >= self.width || y >= self.height {
            return;
        }
        let ptr = self.lfb_ptr();
        unsafe {
            *ptr.add(y * self.width + x) = color;
        }
    }

    /// Rellena toda la pantalla con un color uniforme (limpieza rápida)
    pub fn clear(&self, color: u32) {
        if !self.is_active {
            return;
        }
        let ptr = self.lfb_ptr();
        let total_pixels = self.width * self.height;
        unsafe {
            for i in 0..total_pixels {
                *ptr.add(i) = color;
            }
        }
    }
}

pub static VBE: SpinMutex<VbeGraphics> = SpinMutex::new(VbeGraphics::new());
