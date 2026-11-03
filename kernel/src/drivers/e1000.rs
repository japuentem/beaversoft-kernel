// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Intel E1000 / 82540EM Gigabit Ethernet Driver — DMA Real
//              Implementa transmision (TX) y recepcion (RX) por anillos de
//              descriptores DMA sin ninguna emulacion de loopback.
//              Prerequisito: allocate_contiguous(n) del bitmap frame allocator.
// ============================================================================

// ============================================================================
// Registros MMIO del E1000 (offsets en bytes desde BAR0)
// ============================================================================
pub const E1000_REG_CTRL:    u32 = 0x0000; // Device Control
pub const E1000_REG_STATUS:  u32 = 0x0008; // Device Status
pub const E1000_REG_EERD:    u32 = 0x0014; // EEPROM Read
pub const E1000_REG_ICR:     u32 = 0x00C0; // Interrupt Cause Read
pub const E1000_REG_IMS:     u32 = 0x00D0; // Interrupt Mask Set
pub const E1000_REG_IMC:     u32 = 0x00D8; // Interrupt Mask Clear

// Recepcion
pub const E1000_REG_RCTL:    u32 = 0x0100; // Receive Control
pub const E1000_REG_RDBAL:   u32 = 0x2800; // RX Desc Base Addr Low
pub const E1000_REG_RDBAH:   u32 = 0x2804; // RX Desc Base Addr High
pub const E1000_REG_RDLEN:   u32 = 0x2808; // RX Desc Ring Length
pub const E1000_REG_RDH:     u32 = 0x2810; // RX Desc Head
pub const E1000_REG_RDT:     u32 = 0x2818; // RX Desc Tail

// Transmision
pub const E1000_REG_TCTL:    u32 = 0x0400; // Transmit Control
pub const E1000_REG_TIPG:    u32 = 0x0410; // TX Inter-Packet Gap
pub const E1000_REG_TDBAL:   u32 = 0x3800; // TX Desc Base Addr Low
pub const E1000_REG_TDBAH:   u32 = 0x3804; // TX Desc Base Addr High
pub const E1000_REG_TDLEN:   u32 = 0x3808; // TX Desc Ring Length
pub const E1000_REG_TDH:     u32 = 0x3810; // TX Desc Head
pub const E1000_REG_TDT:     u32 = 0x3818; // TX Desc Tail

// Filtro MAC
pub const E1000_REG_RAL:     u32 = 0x5400; // Receive Address Low
pub const E1000_REG_RAH:     u32 = 0x5404; // Receive Address High

// ============================================================================
// Bits de control
// ============================================================================
pub const E1000_CTRL_RST:    u32 = 1 << 26; // Reset completo del dispositivo
pub const E1000_CTRL_SLU:    u32 = 1 << 6;  // Set Link Up
pub const E1000_CTRL_ASDE:   u32 = 1 << 5;  // Auto-Speed Detection Enable

pub const E1000_RCTL_EN:     u32 = 1 << 1;  // Receiver Enable
pub const E1000_RCTL_SBP:    u32 = 1 << 2;  // Store Bad Packets
pub const E1000_RCTL_UPE:    u32 = 1 << 3;  // Unicast Promiscuous Enable
pub const E1000_RCTL_MPE:    u32 = 1 << 4;  // Multicast Promiscuous Enable
pub const E1000_RCTL_BAM:    u32 = 1 << 15; // Broadcast Accept Mode
pub const E1000_RCTL_SECRC:  u32 = 1 << 26; // Strip Ethernet CRC
pub const E1000_RCTL_BSIZE:  u32 = 0 << 16; // Buffer size: 2048 bytes (BSIZE=00, BSEX=0)

pub const E1000_TCTL_EN:     u32 = 1 << 1;  // Transmitter Enable
pub const E1000_TCTL_PSP:    u32 = 1 << 3;  // Pad Short Packets
pub const E1000_TCTL_CT:     u32 = 0x10 << 4;  // Collision Threshold
pub const E1000_TCTL_COLD:   u32 = 0x40 << 12; // Collision Distance

// Bits de estado de descriptores
pub const E1000_TXD_CMD_EOP: u8 = 1 << 0; // End Of Packet
pub const E1000_TXD_CMD_IFCS:u8 = 1 << 1; // Insert FCS (CRC)
pub const E1000_TXD_CMD_RS:  u8 = 1 << 3; // Report Status
pub const E1000_TXD_STAT_DD: u8 = 1 << 0; // Descriptor Done

pub const E1000_RXD_STAT_DD: u8 = 1 << 0; // Descriptor Done
pub const E1000_RXD_STAT_EOP:u8 = 1 << 1; // End Of Packet

// ============================================================================
// Anillos de descriptores (32 entradas cada uno)
// ============================================================================
pub const NUM_RX_DESC: usize = 32;
pub const NUM_TX_DESC: usize = 32;
pub const RX_BUF_SIZE: usize = 2048;
pub const TX_BUF_SIZE: usize = 2048;

// Descripcion de recepcion E1000 (16 bytes, alineado a 16)
#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct RxDesc {
    pub addr:   u64, // Buffer address (fisico)
    pub length: u16,
    pub csum:   u16,
    pub status: u8,
    pub errors: u8,
    pub special:u16,
}

impl RxDesc {
    pub const fn zero() -> Self {
        Self { addr: 0, length: 0, csum: 0, status: 0, errors: 0, special: 0 }
    }
}

// Descripcion de transmision E1000 (16 bytes, alineado a 16)
#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct TxDesc {
    pub addr:    u64, // Buffer address (fisico)
    pub length:  u16,
    pub cso:     u8,
    pub cmd:     u8,
    pub status:  u8,
    pub css:     u8,
    pub special: u16,
}

impl TxDesc {
    pub const fn zero() -> Self {
        Self { addr: 0, length: 0, cso: 0, cmd: 0, status: 0, css: 0, special: 0 }
    }
}

// ============================================================================
// Estado del driver
// ============================================================================
pub struct E1000Driver {
    pub mmio_base:  u64,       // Base MMIO del dispositivo
    pub initialized:bool,

    // Anillo RX
    pub rx_descs:   *mut RxDesc,
    pub rx_bufs:    [*mut u8; NUM_RX_DESC],
    pub rx_tail:    usize,

    // Anillo TX
    pub tx_descs:   *mut TxDesc,
    pub tx_bufs:    [*mut u8; NUM_TX_DESC],
    pub tx_tail:    usize,
}

// SAFETY: El driver se protege externamente con SpinMutex; los punteros son
// estables (marcos DMA fijos) y solo accedemos desde un contexto a la vez.
unsafe impl Send for E1000Driver {}
unsafe impl Sync for E1000Driver {}

impl E1000Driver {
    pub const fn uninit() -> Self {
        Self {
            mmio_base: 0,
            initialized: false,
            rx_descs: core::ptr::null_mut(),
            rx_bufs: [core::ptr::null_mut(); NUM_RX_DESC],
            rx_tail: 0,
            tx_descs: core::ptr::null_mut(),
            tx_bufs: [core::ptr::null_mut(); NUM_TX_DESC],
            tx_tail: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Lectura / escritura de registros MMIO
    // -----------------------------------------------------------------------

    /// Lee un registro de 32 bits del E1000.
    #[inline]
    pub unsafe fn read_reg(&self, offset: u32) -> u32 {
        // SAFETY: mmio_base apunta a BAR0 del E1000 mapeado identidad en 4 GiB.
        let ptr = (self.mmio_base + offset as u64) as *const u32;
        ptr.read_volatile()
    }

    /// Escribe un registro de 32 bits en el E1000.
    #[inline]
    pub unsafe fn write_reg(&self, offset: u32, val: u32) {
        // SAFETY: idem.
        let ptr = (self.mmio_base + offset as u64) as *mut u32;
        ptr.write_volatile(val);
    }

    // -----------------------------------------------------------------------
    // Inicializacion del hardware
    // -----------------------------------------------------------------------

    /// Inicializa el E1000 con los anillos DMA. Llama solo una vez con MMIO valido.
    /// Devuelve true si tuvo exito.
    pub fn init(&mut self, mmio_base: u64) -> bool {
        use crate::memory::frame_allocator::FRAME_ALLOCATOR;

        self.mmio_base = mmio_base;
        self.initialized = false;

        // 1. Reset del dispositivo
        unsafe {
            self.write_reg(E1000_REG_CTRL, self.read_reg(E1000_REG_CTRL) | E1000_CTRL_RST);
        }
        // Esperar reset (el bit se borra solo)
        for _ in 0..100_000 {
            core::hint::spin_loop();
        }
        unsafe {
            // Desactivar todas las interrupciones — trabajamos en modo polling
            self.write_reg(E1000_REG_IMC, 0xFFFF_FFFF);
            // Set Link Up + Auto-Speed Detection
            self.write_reg(E1000_REG_CTRL,
                self.read_reg(E1000_REG_CTRL) | E1000_CTRL_SLU | E1000_CTRL_ASDE);
            // Limpiar Multicast Table Array (128 registros en 0x5200..0x5400)
            for i in 0..128u32 {
                self.write_reg(0x5200 + i * 4, 0);
            }
        }

        // 2. Configurar anillo RX
        // Necesitamos: 32 descriptores (32*16=512 B) + 32 buffers (32*2048=64 KiB)
        // Asignamos los descriptores en marcos contiguos
        let rx_desc_frames = 1; // 512 bytes caben en 1 marco de 4 KiB
        let rx_desc_addr = match FRAME_ALLOCATOR.allocate_contiguous(rx_desc_frames) {
            Some(a) => a,
            None => return false,
        };
        self.rx_descs = rx_desc_addr as *mut RxDesc;

        // Inicializar descriptores RX y asignar buffers
        for i in 0..NUM_RX_DESC {
            let buf_addr = match FRAME_ALLOCATOR.allocate_frame() {
                Some(a) => a,
                None => return false,
            };
            self.rx_bufs[i] = buf_addr as *mut u8;
            unsafe {
                let desc_ptr = self.rx_descs.add(i);
                desc_ptr.write_volatile(RxDesc {
                    addr: buf_addr as u64,
                    length: 0,
                    csum: 0,
                    status: 0,
                    errors: 0,
                    special: 0,
                });
            }
        }
        self.rx_tail = NUM_RX_DESC - 1;

        unsafe {
            self.write_reg(E1000_REG_RDBAL, rx_desc_addr as u32);
            self.write_reg(E1000_REG_RDBAH, (rx_desc_addr >> 32) as u32);
            self.write_reg(E1000_REG_RDLEN, (NUM_RX_DESC * 16) as u32);
            self.write_reg(E1000_REG_RDH, 0);
            self.write_reg(E1000_REG_RDT, self.rx_tail as u32);
            // RCTL: habilitar | unicast/multicast/broadcast | strip CRC | buffer 2048
            self.write_reg(E1000_REG_RCTL,
                E1000_RCTL_EN | E1000_RCTL_UPE | E1000_RCTL_MPE | E1000_RCTL_BAM | E1000_RCTL_SECRC | E1000_RCTL_BSIZE);
        }

        // 3. Configurar anillo TX
        let tx_desc_frames = 1;
        let tx_desc_addr = match FRAME_ALLOCATOR.allocate_contiguous(tx_desc_frames) {
            Some(a) => a,
            None => return false,
        };
        self.tx_descs = tx_desc_addr as *mut TxDesc;

        for i in 0..NUM_TX_DESC {
            let buf_addr = match FRAME_ALLOCATOR.allocate_frame() {
                Some(a) => a,
                None => return false,
            };
            self.tx_bufs[i] = buf_addr as *mut u8;
            unsafe {
                let desc_ptr = self.tx_descs.add(i);
                desc_ptr.write_volatile(TxDesc {
                    addr: buf_addr as u64,
                    length: 0,
                    cso: 0,
                    cmd: 0,
                    status: E1000_TXD_STAT_DD, // Marcar como libre inicialmente
                    css: 0,
                    special: 0,
                });
            }
        }
        self.tx_tail = 0;

        unsafe {
            self.write_reg(E1000_REG_TDBAL, tx_desc_addr as u32);
            self.write_reg(E1000_REG_TDBAH, (tx_desc_addr >> 32) as u32);
            self.write_reg(E1000_REG_TDLEN, (NUM_TX_DESC * 16) as u32);
            self.write_reg(E1000_REG_TDH, 0);
            self.write_reg(E1000_REG_TDT, 0);
            // TCTL: Enable | Pad Short Packets | CT=16 | COLD=64
            self.write_reg(E1000_REG_TCTL,
                E1000_TCTL_EN | E1000_TCTL_PSP | E1000_TCTL_CT | E1000_TCTL_COLD);
            // TIPG: valor estandar para 802.3 gigabit
            self.write_reg(E1000_REG_TIPG, 0x0060_200A);
        }

        self.initialized = true;
        true
    }

    // -----------------------------------------------------------------------
    // Transmision
    // -----------------------------------------------------------------------

    /// Envia una trama Ethernet por DMA real.
    /// Devuelve true si la trama fue encolada y confirmada por el hardware.
    /// Nunca bloquea indefinidamente (limite de iteraciones).
    pub fn send(&mut self, frame: &[u8]) -> bool {
        if !self.initialized || frame.is_empty() || frame.len() > TX_BUF_SIZE {
            return false;
        }

        let tail = self.tx_tail;
        let desc_ptr = unsafe { self.tx_descs.add(tail) };

        // Verificar que el descriptor esta libre (DD=1 significa que el HW termino)
        let cur_status = unsafe { core::ptr::addr_of!((*desc_ptr).status).read_volatile() };
        if cur_status & E1000_TXD_STAT_DD == 0 {
            // Buffer ocupado — la cola TX esta llena; descartar este paquete
            return false;
        }

        // Copiar trama al buffer DMA
        unsafe {
            core::ptr::copy_nonoverlapping(frame.as_ptr(), self.tx_bufs[tail], frame.len());
            core::ptr::addr_of_mut!((*desc_ptr).length).write_volatile(frame.len() as u16);
            core::ptr::addr_of_mut!((*desc_ptr).cmd).write_volatile(
                E1000_TXD_CMD_EOP | E1000_TXD_CMD_IFCS | E1000_TXD_CMD_RS
            );
            core::ptr::addr_of_mut!((*desc_ptr).status).write_volatile(0);
        }
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        // Avanzar TX tail para notificar al hardware
        self.tx_tail = (tail + 1) % NUM_TX_DESC;
        unsafe {
            self.write_reg(E1000_REG_TDT, self.tx_tail as u32);
        }

        // Esperar confirmacion DD con lectura volatile (limite de iteraciones)
        for _ in 0..50_000 {
            core::hint::spin_loop();
            let st = unsafe { core::ptr::addr_of!((*desc_ptr).status).read_volatile() };
            if st & E1000_TXD_STAT_DD != 0 {
                return true;
            }
        }

        // Timeout: el hardware no confirmo en tiempo. Reportar pero continuar.
        false
    }

    // -----------------------------------------------------------------------
    // Recepcion (polling)
    // -----------------------------------------------------------------------

    /// Sondea el anillo RX y llama al callback para cada trama recibida.
    /// Llama desde el bucle principal o desde el tick del PIT (no desde IRQ).
    /// `callback(frame: &[u8])` recibe la trama completa.
    pub fn poll_rx<F>(&mut self, mut callback: F)
    where
        F: FnMut(&[u8]),
    {
        if !self.initialized { return; }

        for _ in 0..NUM_RX_DESC {
            let next = (self.rx_tail + 1) % NUM_RX_DESC;
            let desc_ptr = unsafe { self.rx_descs.add(next) };
            let status = unsafe { core::ptr::addr_of!((*desc_ptr).status).read_volatile() };
            if status & E1000_RXD_STAT_DD == 0 {
                break; // El hardware aun no escribio en el siguiente descriptor
            }

            let length = unsafe { core::ptr::addr_of!((*desc_ptr).length).read_volatile() };
            if status & E1000_RXD_STAT_EOP != 0 && length > 0 {
                let len = (length as usize).min(RX_BUF_SIZE);
                let buf = unsafe {
                    core::slice::from_raw_parts(self.rx_bufs[next], len)
                };
                callback(buf);
            }

            // Reciclar descriptor: limpiar status/length con write_volatile y devolver al HW
            unsafe {
                core::ptr::addr_of_mut!((*desc_ptr).status).write_volatile(0);
                core::ptr::addr_of_mut!((*desc_ptr).length).write_volatile(0);
            }
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

            self.rx_tail = next;
            unsafe {
                self.write_reg(E1000_REG_RDT, self.rx_tail as u32);
            }
        }
    }
}

use crate::sync::SpinMutex;

pub static E1000: SpinMutex<E1000Driver> = SpinMutex::new(E1000Driver::uninit());

