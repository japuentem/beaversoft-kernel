// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: VirtIO MMIO Bus Discovery & VirtIO-blk Driver for AArch64 (QEMU virt)
// ============================================================================

use core::ptr::{read_volatile, write_volatile};
use crate::sync::SpinMutex;

const VIRTIO_MMIO_BASE: u64 = 0x0A00_0000;
const VIRTIO_MMIO_STRIDE: u64 = 0x200;
const VIRTIO_MMIO_SLOTS: usize = 32;
const VIRTIO_MAGIC: u32 = 0x7472_6976; // "virt"

const REG_MAGIC: usize = 0x000;
const REG_VERSION: usize = 0x004;
const REG_DEVICE_ID: usize = 0x008;
const REG_VENDOR_ID: usize = 0x00C;
const REG_DRIVER_FEATURES: usize = 0x020;
const REG_GUEST_PAGE_SIZE: usize = 0x028;
const REG_QUEUE_SEL: usize = 0x030;
const REG_QUEUE_NUM_MAX: usize = 0x034;
const REG_QUEUE_NUM: usize = 0x038;
const REG_QUEUE_ALIGN: usize = 0x03C;
const REG_QUEUE_PFN: usize = 0x040;
const REG_QUEUE_READY: usize = 0x044;
const REG_QUEUE_NOTIFY: usize = 0x050;
const REG_INT_STATUS: usize = 0x060;
const REG_INT_ACK: usize = 0x064;
const REG_STATUS: usize = 0x070;
const REG_QUEUE_DESC_LOW: usize = 0x080;
const REG_QUEUE_DESC_HIGH: usize = 0x084;
const REG_QUEUE_DRIVER_LOW: usize = 0x090;
const REG_QUEUE_DRIVER_HIGH: usize = 0x094;
const REG_QUEUE_DEVICE_LOW: usize = 0x0A0;
const REG_QUEUE_DEVICE_HIGH: usize = 0x0A4;
const REG_CONFIG: usize = 0x100;

const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER: u32 = 2;
const STATUS_DRIVER_OK: u32 = 4;
const STATUS_FEATURES_OK: u32 = 8;

const VRING_DESC_F_NEXT: u16 = 1;
const VRING_DESC_F_WRITE: u16 = 2;

const QUEUE_SIZE: usize = 16;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct VringDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct VringAvail {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE],
    used_event: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct VringUsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct VringUsed {
    flags: u16,
    idx: u16,
    ring: [VringUsedElem; QUEUE_SIZE],
    avail_event: u16,
}

/// Estructura contigua alineada a 4 KiB compatible con VirtIO MMIO v1 (legacy) y v2 (modern).
#[repr(C, align(4096))]
struct VirtQueueMem {
    desc: [VringDesc; QUEUE_SIZE], // 16 * 16 = 256 bytes
    avail: VringAvail,             // 4 + 32 + 2 = 38 bytes
    _pad: [u8; 4096 - 256 - 38],   // Alinea `used` exactamente a +4096 bytes
    used: VringUsed,
}

static mut BLK_QUEUE: VirtQueueMem = VirtQueueMem {
    desc: [VringDesc { addr: 0, len: 0, flags: 0, next: 0 }; QUEUE_SIZE],
    avail: VringAvail { flags: 0, idx: 0, ring: [0; QUEUE_SIZE], used_event: 0 },
    _pad: [0; 4096 - 256 - 38],
    used: VringUsed { flags: 0, idx: 0, ring: [VringUsedElem { id: 0, len: 0 }; QUEUE_SIZE], avail_event: 0 },
};

#[repr(C)]
struct VirtioBlkReq {
    req_type: u32,
    reserved: u32,
    sector: u64,
}

static mut BLK_REQ: VirtioBlkReq = VirtioBlkReq { req_type: 0, reserved: 0, sector: 0 };
static mut BLK_STATUS: u8 = 0xFF;

#[derive(Debug, Clone, Copy)]
pub struct VirtioDiscoverySummary {
    pub blk_present: bool,
    pub blk_base: u64,
    pub blk_version: u32,
    pub blk_sectors: u64,
    pub net_present: bool,
    pub net_base: u64,
    pub total_devices: usize,
}

struct VirtioBlkState {
    initialized: bool,
    base: u64,
    version: u32,
    sectors: u64,
    last_used_idx: u16,
    avail_idx: u16,
    net_present: bool,
    net_base: u64,
    total_devices: usize,
}

static VIRTIO_BLK: SpinMutex<VirtioBlkState> = SpinMutex::new(VirtioBlkState {
    initialized: false,
    base: 0,
    version: 0,
    sectors: 0,
    last_used_idx: 0,
    avail_idx: 0,
    net_present: false,
    net_base: 0,
    total_devices: 0,
});

#[inline]
unsafe fn mmio_read32(base: u64, offset: usize) -> u32 {
    read_volatile((base + offset as u64) as *const u32)
}

#[inline]
unsafe fn mmio_write32(base: u64, offset: usize, val: u32) {
    write_volatile((base + offset as u64) as *mut u32, val);
}

#[inline]
fn memory_barrier() {
    unsafe {
        core::arch::asm!("dsb sy", options(nostack));
    }
}

/// Escanea los 32 slots VirtIO MMIO en QEMU `virt` e inicializa VirtIO-blk real si está presente.
pub fn probe_and_init_virtio() -> VirtioDiscoverySummary {
    let mut state = VIRTIO_BLK.lock();
    state.initialized = false;
    state.base = 0;
    state.sectors = 0;
    state.net_present = false;
    state.net_base = 0;
    state.total_devices = 0;

    for slot in 0..VIRTIO_MMIO_SLOTS {
        let base = VIRTIO_MMIO_BASE + (slot as u64) * VIRTIO_MMIO_STRIDE;
        // SAFETY: `0x0A00_0000..0x0A00_4000` es la ventana MMIO VirtIO estándar en QEMU `virt`, mapeada como Device-nGnRnE.
        unsafe {
            let magic = mmio_read32(base, REG_MAGIC);
            if magic != VIRTIO_MAGIC {
                continue;
            }
            let version = mmio_read32(base, REG_VERSION);
            let dev_id = mmio_read32(base, REG_DEVICE_ID);
            if dev_id == 0 {
                continue;
            }
            state.total_devices += 1;

            if dev_id == 1 && !state.net_present {
                state.net_present = true;
                state.net_base = base;
            } else if dev_id == 2 && !state.initialized {
                if init_virtio_blk_device(base, version, &mut state) {
                    state.initialized = true;
                    state.base = base;
                    state.version = version;
                }
            }
        }
    }

    VirtioDiscoverySummary {
        blk_present: state.initialized,
        blk_base: state.base,
        blk_version: state.version,
        blk_sectors: state.sectors,
        net_present: state.net_present,
        net_base: state.net_base,
        total_devices: state.total_devices,
    }
}

unsafe fn init_virtio_blk_device(base: u64, version: u32, state: &mut VirtioBlkState) -> bool {
    // 1. Reset
    mmio_write32(base, REG_STATUS, 0);
    memory_barrier();

    // 2. ACKNOWLEDGE | DRIVER
    mmio_write32(base, REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    memory_barrier();

    if version == 2 {
        mmio_write32(base, REG_DRIVER_FEATURES, 0);
        mmio_write32(base, REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK);
        memory_barrier();
        if (mmio_read32(base, REG_STATUS) & STATUS_FEATURES_OK) == 0 {
            return false;
        }
    } else if version == 1 {
        mmio_write32(base, REG_DRIVER_FEATURES, 0);
        mmio_write32(base, REG_GUEST_PAGE_SIZE, 4096);
    } else {
        return false;
    }

    // 3. Configurar VirtQueue 0
    mmio_write32(base, REG_QUEUE_SEL, 0);
    let max_q = mmio_read32(base, REG_QUEUE_NUM_MAX);
    if max_q < QUEUE_SIZE as u32 {
        return false;
    }
    mmio_write32(base, REG_QUEUE_NUM, QUEUE_SIZE as u32);

    let q_base = core::ptr::addr_of!(BLK_QUEUE) as u64;
    let desc_addr = core::ptr::addr_of!(BLK_QUEUE.desc) as u64;
    let avail_addr = core::ptr::addr_of!(BLK_QUEUE.avail) as u64;
    let used_addr = core::ptr::addr_of!(BLK_QUEUE.used) as u64;

    if version == 2 {
        mmio_write32(base, REG_QUEUE_DESC_LOW, desc_addr as u32);
        mmio_write32(base, REG_QUEUE_DESC_HIGH, (desc_addr >> 32) as u32);
        mmio_write32(base, REG_QUEUE_DRIVER_LOW, avail_addr as u32);
        mmio_write32(base, REG_QUEUE_DRIVER_HIGH, (avail_addr >> 32) as u32);
        mmio_write32(base, REG_QUEUE_DEVICE_LOW, used_addr as u32);
        mmio_write32(base, REG_QUEUE_DEVICE_HIGH, (used_addr >> 32) as u32);
        mmio_write32(base, REG_QUEUE_READY, 1);
    } else {
        mmio_write32(base, REG_QUEUE_ALIGN, 4096);
        mmio_write32(base, REG_QUEUE_PFN, (q_base >> 12) as u32);
    }

    // 4. DRIVER_OK
    let cur_status = mmio_read32(base, REG_STATUS);
    mmio_write32(base, REG_STATUS, cur_status | STATUS_DRIVER_OK);
    memory_barrier();

    // 5. Leer capacidad real en sectores de 512B desde Config (0x100..0x107)
    let cap_lo = mmio_read32(base, REG_CONFIG) as u64;
    let cap_hi = mmio_read32(base, REG_CONFIG + 4) as u64;
    state.sectors = cap_lo | (cap_hi << 32);
    state.avail_idx = 0;
    state.last_used_idx = 0;
    let _ = mmio_read32(base, REG_VENDOR_ID);
    true
}

/// Lee un sector real de 512 bytes del disco VirtIO-blk en ARM64.
pub fn read_sector(lba: u64, buf: &mut [u8; 512]) -> Result<(), &'static str> {
    submit_blk_request(false, lba, buf.as_mut_ptr() as u64)
}

/// Escribe un sector real de 512 bytes en el disco VirtIO-blk en ARM64.
pub fn write_sector(lba: u64, buf: &[u8; 512]) -> Result<(), &'static str> {
    submit_blk_request(true, lba, buf.as_ptr() as u64)
}

fn submit_blk_request(is_write: bool, lba: u64, buf_ptr: u64) -> Result<(), &'static str> {
    let mut state = VIRTIO_BLK.lock();
    if !state.initialized {
        return Err("VirtIO-blk no inicializado");
    }
    if lba >= state.sectors {
        return Err("LBA fuera de rango en VirtIO-blk");
    }

    // SAFETY: Exclusividad garantizada por `VIRTIO_BLK` SpinMutex; los descriptores apuntan a buffers estáticos/stack válidos durante el poll síncrono.
    unsafe {
        BLK_REQ.req_type = if is_write { 1 } else { 0 }; // 1 = VIRTIO_BLK_T_OUT, 0 = VIRTIO_BLK_T_IN
        BLK_REQ.reserved = 0;
        BLK_REQ.sector = lba;
        BLK_STATUS = 0xFF;

        BLK_QUEUE.desc[0] = VringDesc {
            addr: core::ptr::addr_of!(BLK_REQ) as u64,
            len: 16,
            flags: VRING_DESC_F_NEXT,
            next: 1,
        };
        BLK_QUEUE.desc[1] = VringDesc {
            addr: buf_ptr,
            len: 512,
            flags: if is_write { VRING_DESC_F_NEXT } else { VRING_DESC_F_NEXT | VRING_DESC_F_WRITE },
            next: 2,
        };
        BLK_QUEUE.desc[2] = VringDesc {
            addr: core::ptr::addr_of!(BLK_STATUS) as u64,
            len: 1,
            flags: VRING_DESC_F_WRITE,
            next: 0,
        };

        let slot = (state.avail_idx as usize) % QUEUE_SIZE;
        BLK_QUEUE.avail.ring[slot] = 0;
        memory_barrier();

        state.avail_idx = state.avail_idx.wrapping_add(1);
        BLK_QUEUE.avail.idx = state.avail_idx;
        memory_barrier();

        mmio_write32(state.base, REG_QUEUE_NOTIFY, 0);

        let mut timeout = 2_000_000u32;
        while read_volatile(core::ptr::addr_of!(BLK_QUEUE.used.idx)) == state.last_used_idx {
            memory_barrier();
            timeout -= 1;
            if timeout == 0 {
                return Err("Timeout esperando respuesta VirtIO-blk");
            }
        }
        state.last_used_idx = read_volatile(core::ptr::addr_of!(BLK_QUEUE.used.idx));

        let isr = mmio_read32(state.base, REG_INT_STATUS);
        if isr != 0 {
            mmio_write32(state.base, REG_INT_ACK, isr);
        }

        if read_volatile(core::ptr::addr_of!(BLK_STATUS)) != 0 {
            return Err("Error de E/S reportado por VirtIO-blk");
        }
    }

    Ok(())
}

