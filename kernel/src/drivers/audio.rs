// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Bare-Metal Audio Subsystem (Intel HDA, AC'97 DMA, PC Speaker & WAV Parser)
//              Stereo 16-bit PCM streaming, RIFF WAVE decoder & Beaver Chime Synthesizer
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use crate::io::{inb, outb, outw, outl};
use crate::drivers::pci::{pci_read_config_u32, pci_write_config_u32};
use crate::sync::SpinLock;
use crate::memory::AUDIO_DMA_BUFFER_ADDR;

// --- Puertos del Altavoz PC Speaker (PIT Canal 2) ---
const TIMER_CHANNEL_2: u16 = 0x42;
const TIMER_COMMAND: u16   = 0x43;
const SPEAKER_PORT: u16    = 0x61;
const PIT_BASE_FREQUENCY: u32 = 1193180;

// --- Registros Intel High Definition Audio (HDA MMIO) ---
const HDA_REG_GCAP: usize     = 0x00; // Global Capabilities (u16)
const HDA_REG_GCTL: usize     = 0x08; // Global Control (u32)
#[allow(dead_code)]
const HDA_REG_WAKEEN: usize   = 0x0C; // Wake Enable (u16)
#[allow(dead_code)]
const HDA_REG_STATESTS: usize = 0x0E; // State Change Status (u16)
#[allow(dead_code)]
const HDA_REG_INTCTL: usize   = 0x20; // Interrupt Control (u32)
#[allow(dead_code)]
const HDA_REG_CORBLBASE: usize= 0x40; // CORB Lower Base Address
#[allow(dead_code)]
const HDA_REG_CORBUBASE: usize= 0x44; // CORB Upper Base Address
#[allow(dead_code)]
const HDA_REG_CORBWP: usize   = 0x48; // CORB Write Pointer
#[allow(dead_code)]
const HDA_REG_CORBRP: usize   = 0x4A; // CORB Read Pointer
#[allow(dead_code)]
const HDA_REG_CORBCTL: usize  = 0x4C; // CORB Control
#[allow(dead_code)]
const HDA_REG_CORBSTS: usize  = 0x4D; // CORB Status
#[allow(dead_code)]
const HDA_REG_CORBSIZE: usize = 0x4E; // CORB Size

// --- Offsets de Stream Descriptor HDA (relativos al stream) ---
const HDA_SD_CTL: usize   = 0x00; // Control (u32: bits 23:20 Stream Tag, bit 1 RUN, bit 0 SRST)
#[allow(dead_code)]
const HDA_SD_STS: usize   = 0x03; // Status (u8: bit 2 BCIS, bit 5 FIFORDY)
#[allow(dead_code)]
const HDA_SD_LPIB: usize  = 0x04; // Link Position in Buffer (u32)
const HDA_SD_CBL: usize   = 0x08; // Cyclic Buffer Length (u32)
const HDA_SD_LVI: usize   = 0x0C; // Last Valid Index (u16)
#[allow(dead_code)]
const HDA_SD_FIFOS: usize = 0x0E; // FIFO Size (u16)
const HDA_SD_FMT: usize   = 0x12; // Stream Format (u16)
const HDA_SD_BDPL: usize  = 0x18; // BDL Pointer Lower (u32)
const HDA_SD_BDPU: usize  = 0x1C; // BDL Pointer Upper (u32)

// --- Registros AC'97 Bus Mastering (I/O Space) ---
const AC97_PO_BDBAR: u16 = 0x10; // PCM Output Buffer Descriptor Base Address
#[allow(dead_code)]
const AC97_PO_CIV: u16   = 0x14; // PCM Output Current Index Value
const AC97_PO_LVI: u16   = 0x15; // PCM Output Last Valid Index
#[allow(dead_code)]
const AC97_PO_SR: u16    = 0x16; // PCM Output Status Register
const AC97_PO_CR: u16    = 0x1B; // PCM Output Control Register

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioControllerType {
    PcSpeaker,
    IntelHda,
    Ac97,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioDeviceInfo {
    pub controller_type: AudioControllerType,
    pub pci_audio_found: bool,
    pub pci_vendor_id: u16,
    pub pci_device_id: u16,
    pub pci_device_name: &'static str,
    pub mmio_base: u64,
    pub nambar_io: u16,
    pub nabmbar_io: u16,
    pub num_output_streams: u8,
    pub num_input_streams: u8,
    pub dma_buffer_addr: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub total_pcm_bytes_played: u64,
    pub total_beeps_played: u64,
}

pub static AUDIO_STATE: SpinLock<AudioDeviceInfo> = SpinLock::new(AudioDeviceInfo {
    controller_type: AudioControllerType::PcSpeaker,
    pci_audio_found: false,
    pci_vendor_id: 0,
    pci_device_id: 0,
    pci_device_name: "PC Speaker PIT 8254",
    mmio_base: 0,
    nambar_io: 0,
    nabmbar_io: 0,
    num_output_streams: 0,
    num_input_streams: 0,
    dma_buffer_addr: AUDIO_DMA_BUFFER_ADDR as u64,
    sample_rate: 44100,
    channels: 2,
    bits_per_sample: 16,
    total_pcm_bytes_played: 0,
    total_beeps_played: 0,
});

/// Entrada de la Lista de Descriptores de Búfer (BDL) de Intel HDA
#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct HdaBdlEntry {
    pub addr_low: u32,
    pub addr_high: u32,
    pub length: u32,
    pub flags: u32, // bit 0 = Interrupt on Completion (IOC)
}

/// Entrada de Descriptor de Búfer para AC'97 Bus Master
#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct Ac97BdlEntry {
    pub buffer_ptr: u32,
    pub sample_count: u16,
    pub flags: u16, // bit 15 = BUP, bit 14 = IOC
}

/// Metadatos parseados de un archivo de audio WAVE estándar
#[derive(Debug, Clone, Copy)]
pub struct WavHeader {
    pub num_channels: u16,
    pub sample_rate: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub data_size: u32,
    pub data_offset: usize,
    pub duration_ms: u32,
}

impl WavHeader {
    /// Parsea los encabezados RIFF WAVE estándar de un búfer de bytes
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 44 {
            return Err("Archivo WAV demasiado pequeño (menor a 44 bytes de cabecera)");
        }

        // 1. Validar cabecera 'RIFF' y formato 'WAVE'
        if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err("Firma RIFF/WAVE no válida");
        }

        let mut offset = 12;
        let mut fmt_found = false;
        let mut channels = 2_u16;
        let mut sample_rate = 44100_u32;
        let mut byte_rate = 176400_u32;
        let mut block_align = 4_u16;
        let mut bits_per_sample = 16_u16;

        let mut data_offset = 0;
        let mut data_size = 0_u32;

        while offset + 8 <= bytes.len() {
            let chunk_id = &bytes[offset..offset + 4];
            let chunk_size = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;

            let chunk_data = offset + 8;

            if chunk_id == b"fmt " {
                if chunk_size >= 16 && chunk_data + 16 <= bytes.len() {
                    let audio_format = u16::from_le_bytes([bytes[chunk_data], bytes[chunk_data + 1]]);
                    if audio_format != 1 && audio_format != 3 {
                        return Err("Formato de compresión no soportado (solo PCM lineal)");
                    }
                    channels = u16::from_le_bytes([bytes[chunk_data + 2], bytes[chunk_data + 3]]);
                    sample_rate = u32::from_le_bytes([
                        bytes[chunk_data + 4],
                        bytes[chunk_data + 5],
                        bytes[chunk_data + 6],
                        bytes[chunk_data + 7],
                    ]);
                    byte_rate = u32::from_le_bytes([
                        bytes[chunk_data + 8],
                        bytes[chunk_data + 9],
                        bytes[chunk_data + 10],
                        bytes[chunk_data + 11],
                    ]);
                    block_align = u16::from_le_bytes([bytes[chunk_data + 12], bytes[chunk_data + 13]]);
                    bits_per_sample = u16::from_le_bytes([bytes[chunk_data + 14], bytes[chunk_data + 15]]);
                    fmt_found = true;
                }
            } else if chunk_id == b"data" {
                data_offset = chunk_data;
                data_size = chunk_size.min(bytes.len().saturating_sub(chunk_data)) as u32;
                break;
            }

            offset = chunk_data + ((chunk_size + 1) & !1);
        }

        if !fmt_found || data_offset == 0 {
            return Err("Estructura WAV incompleta (falta chunk fmt o data)");
        }

        let duration_ms = if byte_rate > 0 {
            ((data_size as u64 * 1000) / (byte_rate as u64)) as u32
        } else {
            0
        };

        Ok(Self {
            num_channels: channels,
            sample_rate,
            byte_rate,
            block_align,
            bits_per_sample,
            data_size,
            data_offset,
            duration_ms,
        })
    }
}

/// Inicializa el subsistema de audio y escanea dispositivos de sonido en el bus PCI
pub fn init() {
    let mut state = AUDIO_STATE.lock();

    // Escanear bus PCI buscando controladores de sonido (Clase 0x04 = Multimedia)
    for bus in 0..=4 {
        for device in 0..32 {
            let id_reg = pci_read_config_u32(bus, device, 0, 0x00);
            let vendor_id = (id_reg & 0xFFFF) as u16;
            if vendor_id == 0xFFFF || vendor_id == 0x0000 {
                continue;
            }
            let device_id = ((id_reg >> 16) & 0xFFFF) as u16;

            let class_reg = pci_read_config_u32(bus, device, 0, 0x08);
            let class_code = ((class_reg >> 24) & 0xFF) as u8;
            let subclass = ((class_reg >> 16) & 0xFF) as u8;

            if class_code == 0x04 {
                state.pci_audio_found = true;
                state.pci_vendor_id = vendor_id;
                state.pci_device_id = device_id;

                // Habilitar Bus Master y Memory/IO Space en el comando PCI (Offset 0x04)
                let cmd = pci_read_config_u32(bus, device, 0, 0x04);
                pci_write_config_u32(bus, device, 0, 0x04, cmd | 0x0007);

                if subclass == 0x03 {
                    // Intel High Definition Audio (HDA)
                    state.controller_type = AudioControllerType::IntelHda;
                    state.pci_device_name = "Intel High Definition Audio (HDA / ICH)";
                    let bar0 = pci_read_config_u32(bus, device, 0, 0x10);
                    state.mmio_base = (bar0 & !0x0F) as u64;

                    init_hda_hardware(state.mmio_base, &mut state);
                } else if subclass == 0x01 {
                    // AC'97 Audio Controller
                    state.controller_type = AudioControllerType::Ac97;
                    state.pci_device_name = "AC'97 Audio Codec Controller";
                    let bar0 = pci_read_config_u32(bus, device, 0, 0x10);
                    let bar1 = pci_read_config_u32(bus, device, 0, 0x14);
                    state.nambar_io = (bar0 & !0x01) as u16;
                    state.nabmbar_io = (bar1 & !0x01) as u16;

                    init_ac97_hardware(state.nambar_io, state.nabmbar_io, &mut state);
                } else {
                    state.pci_device_name = "Controlador Multimedia / Sound Card PCI";
                }
                break;
            }
        }
        if state.pci_audio_found {
            break;
        }
    }

    // Asegurar que el altavoz PIT esté silenciado al arranque
    stop_sound();
}

/// Configura los registros y descriptores DMA de Intel HDA
fn init_hda_hardware(mmio: u64, state: &mut AudioDeviceInfo) {
    if mmio == 0 {
        return;
    }
    let mmio_ptr = mmio as *mut u8;

    unsafe {
        // 1. Resetear el controlador HDA mediante el bit CRST en GCTL
        let gctl_ptr = mmio_ptr.add(HDA_REG_GCTL) as *mut u32;
        let prev_gctl = core::ptr::read_volatile(gctl_ptr);
        core::ptr::write_volatile(gctl_ptr, prev_gctl & !0x01); // Apagar CRST
        for _ in 0..10_000 { core::hint::spin_loop(); }
        core::ptr::write_volatile(gctl_ptr, prev_gctl | 0x01);  // Encender CRST
        for _ in 0..10_000 { core::hint::spin_loop(); }

        // 2. Leer capacidades globales (GCAP)
        let gcap_ptr = mmio_ptr.add(HDA_REG_GCAP) as *const u16;
        let gcap = core::ptr::read_volatile(gcap_ptr);
        let num_iss = ((gcap >> 8) & 0x0F) as u8;  // Input Streams
        let num_oss = ((gcap >> 12) & 0x0F) as u8; // Output Streams
        state.num_input_streams = num_iss;
        state.num_output_streams = num_oss;

        // 3. Configurar Buffer Descriptor List (BDL) en AUDIO_DMA_BUFFER_ADDR
        // Estructura: BDL en 0x0100_0000, Buffer 0 en 0x0100_1000, Buffer 1 en 0x0100_9000
        let bdl_ptr = AUDIO_DMA_BUFFER_ADDR as *mut HdaBdlEntry;
        let buf0_addr = (AUDIO_DMA_BUFFER_ADDR + 0x1000) as u32;
        let buf1_addr = (AUDIO_DMA_BUFFER_ADDR + 0x9000) as u32;
        let buf_size = 32768_u32; // 32 KiB por búfer

        *bdl_ptr = HdaBdlEntry {
            addr_low: buf0_addr,
            addr_high: 0,
            length: buf_size,
            flags: 1, // IOC
        };
        *bdl_ptr.add(1) = HdaBdlEntry {
            addr_low: buf1_addr,
            addr_high: 0,
            length: buf_size,
            flags: 1, // IOC
        };

        // 4. Configurar Output Stream 0
        // Base de registros del primer output stream: 0x80 + 0x20 * num_input_streams
        let stream0_offset = 0x80 + (num_iss as usize) * 0x20;
        let stream_ptr = mmio_ptr.add(stream0_offset);

        // Reset del stream
        let ctl_ptr = stream_ptr.add(HDA_SD_CTL) as *mut u32;
        core::ptr::write_volatile(ctl_ptr, 0x01); // SRST = 1
        for _ in 0..5_000 { core::hint::spin_loop(); }
        core::ptr::write_volatile(ctl_ptr, 0x00); // SRST = 0
        for _ in 0..5_000 { core::hint::spin_loop(); }

        // Asignar Stream Tag 1 (bits 23:20)
        core::ptr::write_volatile(ctl_ptr, 0x0010_0000);

        // Asignar puntero BDL
        let bdpl_ptr = stream_ptr.add(HDA_SD_BDPL) as *mut u32;
        let bdpu_ptr = stream_ptr.add(HDA_SD_BDPU) as *mut u32;
        core::ptr::write_volatile(bdpl_ptr, AUDIO_DMA_BUFFER_ADDR as u32);
        core::ptr::write_volatile(bdpu_ptr, 0);

        // Cyclic Buffer Length (CBL = 64 KiB total)
        let cbl_ptr = stream_ptr.add(HDA_SD_CBL) as *mut u32;
        core::ptr::write_volatile(cbl_ptr, buf_size * 2);

        // Last Valid Index (LVI = 1 para 2 descriptores)
        let lvi_ptr = stream_ptr.add(HDA_SD_LVI) as *mut u16;
        core::ptr::write_volatile(lvi_ptr, 1);

        // Formato predeterminado: 44.1 kHz, 16-bit, Estéreo (0x4011)
        let fmt_ptr = stream_ptr.add(HDA_SD_FMT) as *mut u16;
        core::ptr::write_volatile(fmt_ptr, 0x4011);
    }
}

/// Configura los registros y descriptores DMA de AC'97
fn init_ac97_hardware(nambar: u16, nabmbar: u16, _state: &mut AudioDeviceInfo) {
    if nabmbar == 0 {
        return;
    }
    unsafe {
        // Reset bus mastering
        outb(nabmbar + AC97_PO_CR, 0x02); // Reset
        for _ in 0..5_000 { core::hint::spin_loop(); }

        // Configurar descriptores AC'97 en AUDIO_DMA_BUFFER_ADDR
        let bdl_ptr = AUDIO_DMA_BUFFER_ADDR as *mut Ac97BdlEntry;
        let buf0_addr = (AUDIO_DMA_BUFFER_ADDR + 0x1000) as u32;
        let buf1_addr = (AUDIO_DMA_BUFFER_ADDR + 0x9000) as u32;

        *bdl_ptr = Ac97BdlEntry {
            buffer_ptr: buf0_addr,
            sample_count: 16384, // 16384 muestras
            flags: 0x8000,      // BUP
        };
        *bdl_ptr.add(1) = Ac97BdlEntry {
            buffer_ptr: buf1_addr,
            sample_count: 16384,
            flags: 0x8000,
        };

        outl(nabmbar + AC97_PO_BDBAR, AUDIO_DMA_BUFFER_ADDR as u32);
        outb(nabmbar + AC97_PO_LVI, 1);

        // Ajustar volumen del mezclador si NAMBAR está presente
        if nambar != 0 {
            outw(nambar + 0x02, 0x0000); // Master Volume 0dB
            outw(nambar + 0x18, 0x0000); // PCM Out Volume 0dB
        }
    }
}

/// Carga muestras PCM en los búferes DMA y dispara la reproducción por hardware
pub fn play_pcm_dma(pcm_data: &[u8], sample_rate: u32, channels: u16, bits_per_sample: u16) {
    let mut state = AUDIO_STATE.lock();
    state.sample_rate = sample_rate;
    state.channels = channels;
    state.bits_per_sample = bits_per_sample;
    state.total_pcm_bytes_played += pcm_data.len() as u64;

    // Copiar muestras a la memoria DMA
    let dma_dest = (AUDIO_DMA_BUFFER_ADDR + 0x1000) as *mut u8;
    let copy_len = pcm_data.len().min(65536);
    unsafe {
        core::ptr::copy_nonoverlapping(pcm_data.as_ptr(), dma_dest, copy_len);
    }

    match state.controller_type {
        AudioControllerType::IntelHda => {
            if state.mmio_base != 0 {
                let mmio_ptr = state.mmio_base as *mut u8;
                let stream0_offset = 0x80 + (state.num_input_streams as usize) * 0x20;
                let stream_ptr = unsafe { mmio_ptr.add(stream0_offset) };

                unsafe {
                    // Configurar formato según el archivo
                    // Base rate: bit 14 (0 = 48 kHz, 1 = 44.1 kHz)
                    let base_bit = if sample_rate == 44100 { 1 << 14 } else { 0 };
                    let bits_code = if bits_per_sample == 16 { 1 << 4 } else { 0 };
                    let chan_code = (channels.saturating_sub(1)) & 0x0F;
                    let fmt_code = base_bit | bits_code | chan_code;

                    let fmt_ptr = stream_ptr.add(HDA_SD_FMT) as *mut u16;
                    core::ptr::write_volatile(fmt_ptr, fmt_code);

                    // Ajustar longitud total del búfer
                    let cbl_ptr = stream_ptr.add(HDA_SD_CBL) as *mut u32;
                    core::ptr::write_volatile(cbl_ptr, copy_len as u32);

                    // Activar RUN (bit 1) en SD_CTL
                    let ctl_ptr = stream_ptr.add(HDA_SD_CTL) as *mut u32;
                    let cur_ctl = core::ptr::read_volatile(ctl_ptr);
                    core::ptr::write_volatile(ctl_ptr, cur_ctl | 0x02);
                }
            }
        }
        AudioControllerType::Ac97 => {
            if state.nabmbar_io != 0 {
                unsafe {
                    outb(state.nabmbar_io + AC97_PO_CR, 0x01); // RUN
                }
            }
        }
        AudioControllerType::PcSpeaker => {}
    }
}

/// Reproduce un archivo WAVE completo ubicado en el VFS (/disk/*.wav)
pub fn play_wav_file(path: &str) -> Result<String, &'static str> {
    let bytes = {
        let vfs = crate::fs::vfs::VFS.lock();
        vfs.read_file_bytes(path, 0, 0).map_err(|_| "No se pudo leer el archivo WAV desde VFS")?
    };

    let header = WavHeader::parse(&bytes)?;
    let pcm_slice = &bytes[header.data_offset..header.data_offset + (header.data_size as usize)];

    // 1. Enviar el flujo PCM a las controladoras DMA (Intel HDA / AC'97)
    play_pcm_dma(pcm_slice, header.sample_rate, header.num_channels, header.bits_per_sample);

    // 2. Ejecutar la secuencia acústica institucional Beaver en PC Speaker
    // para garantizar feedback inmediato en cualquier emulador o hardware
    startup_chime();

    Ok(alloc::format!(
        "Formato: PCM {} {}Hz {}-bit | Muestras: {} bytes | Duracion: {}ms",
        if header.num_channels == 2 { "Estéreo" } else { "Mono" },
        header.sample_rate,
        header.bits_per_sample,
        header.data_size,
        header.duration_ms
    ))
}

/// Genera un archivo binario RIFF WAVE auténtico de 16 bits estéreo a 44.1 kHz con el Acorde Beaver
pub fn create_beaver_chime_wav() -> Vec<u8> {
    let sample_rate = 44100_u32;
    let channels = 2_u16;
    let bits_per_sample = 16_u16;
    let block_align = (channels * bits_per_sample) / 8; // 4 bytes por frame
    let byte_rate = sample_rate * (block_align as u32); // 176400 bytes/sec

    // Duración: 0.35 segundos = 15435 frames estéreo = 61740 bytes de audio
    let num_samples = 15435_usize;
    let data_size = (num_samples * (block_align as usize)) as u32;
    let total_file_size = 44 + data_size;

    let mut wav = Vec::with_capacity(total_file_size as usize);

    // 1. Encabezado RIFF (12 bytes)
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(total_file_size - 8).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // 2. Encabezado 'fmt ' (24 bytes)
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes()); // Subchunk size (16 bytes)
    wav.extend_from_slice(&1_u16.to_le_bytes());  // AudioFormat (1 = PCM)
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // 3. Encabezado 'data' (8 bytes)
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    // 4. Síntesis armónica del Acorde Beaver en 16-bit signed stereo
    // Armónicos: C5 (523 Hz), E5 (659 Hz), G5 (784 Hz), C6 (1046 Hz)
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;

        // Envolvente de decaimiento suave (Attack rápido de 5ms, Decay exponencial)
        let envelope = if t < 0.005 {
            t / 0.005
        } else {
            let remain = 1.0 - (t - 0.005) / 0.345;
            remain * remain
        };

        // Aproximación polinómica bare-metal de seno para pureza sonora sin std
        let sample_c5 = approx_sin(523.25 * t * 6.283185);
        let sample_e5 = approx_sin(659.25 * t * 6.283185);
        let sample_g5 = approx_sin(783.99 * t * 6.283185);
        let sample_c6 = approx_sin(1046.50 * t * 6.283185);

        let mixed = (sample_c5 * 0.35 + sample_e5 * 0.25 + sample_g5 * 0.25 + sample_c6 * 0.15) * envelope;
        let sample_i16 = (mixed * 24000.0) as i16;

        // Canal Izquierdo y Canal Derecho (Estéreo)
        wav.extend_from_slice(&sample_i16.to_le_bytes());
        wav.extend_from_slice(&sample_i16.to_le_bytes());
    }

    wav
}

/// Función auxiliar para aproximar seno bare-metal
fn approx_sin(mut x: f32) -> f32 {
    let two_pi = 6.2831853;
    x = x % two_pi;
    if x < 0.0 {
        x += two_pi;
    }
    // Aproximación parabólica de Bhaskara I
    let pi = 3.1415926;
    if x > pi {
        -approx_sin(x - pi)
    } else {
        (16.0 * x * (pi - x)) / (5.0 * pi * pi - 4.0 * x * (pi - x))
    }
}

/// Inicializa los archivos de audio en disco (/disk/chime.wav) si no están presentes
pub fn init_disk_audio() {
    let vfs = crate::fs::vfs::VFS.lock();
    if vfs.read_file_bytes("/disk/chime.wav", 0, 0).is_err() {
        drop(vfs);
        let chime_bytes = create_beaver_chime_wav();
        let _ = crate::fs::vfs::VFS.lock().create_file("/disk/chime.wav", &chime_bytes, 0, 0);
    }
}

// ============================================================================
// Funciones del Altavoz PC Speaker (PIT 8254 Timer 2)
// ============================================================================

/// Inicia la reproducción de una frecuencia pura en Hertz a través del timer 2
pub fn start_tone(frequency_hz: u32) {
    if frequency_hz < 20 || frequency_hz > 20000 {
        return;
    }

    let divisor = PIT_BASE_FREQUENCY / frequency_hz;

    unsafe {
        // Configurar PIT Canal 2: Modo 3 (Square Wave Generator), formato Low/High byte (0xB6)
        outb(TIMER_COMMAND, 0xB6);
        outb(TIMER_CHANNEL_2, (divisor & 0xFF) as u8);
        outb(TIMER_CHANNEL_2, ((divisor >> 8) & 0xFF) as u8);

        // Habilitar bits 0 y 1 del puerto 0x61 para conectar el altavoz al Timer 2
        let prev = inb(SPEAKER_PORT);
        if (prev & 0x03) != 0x03 {
            outb(SPEAKER_PORT, prev | 0x03);
        }
    }
}

/// Detiene la emisión de sonido apagando la compuerta del altavoz
pub fn stop_sound() {
    unsafe {
        let prev = inb(SPEAKER_PORT);
        outb(SPEAKER_PORT, prev & 0xFC); // Apaga bits 0 y 1
    }
}

/// Pequeño retardo de ciclo para temporización de notas
fn delay_cycles(count: u64) {
    for _ in 0..count {
        core::hint::spin_loop();
    }
}

/// Reproduce un tono con frecuencia en Hz y duración en milisegundos aproximados
pub fn play_tone(frequency_hz: u32, duration_ms: u32) {
    start_tone(frequency_hz);
    let cycles_per_ms = 2_000;
    delay_cycles((duration_ms as u64) * cycles_per_ms);
    stop_sound();
    // Pausa breve entre notas (2ms)
    delay_cycles(2 * cycles_per_ms);

    if let Some(mut state) = AUDIO_STATE.try_lock() {
        state.total_beeps_played += 1;
    }
}

/// Chime institucional de arranque de Beaver OS
pub fn startup_chime() {
    // Acorde Beaver en ascenso: C5 (523 Hz), E5 (659 Hz), G5 (784 Hz), C6 (1046 Hz)
    play_tone(523, 70);
    play_tone(659, 70);
    play_tone(784, 80);
    play_tone(1046, 160);
}

/// Reproduce el Chime desde disco o mediante síntesis PIT
pub fn play_chime() {
    let _ = play_wav_file("/disk/chime.wav");
    startup_chime();
}

/// Clic sutil para retroalimentación sonora de la interfaz gráfica
pub fn click_feedback() {
    start_tone(1200);
    delay_cycles(30_000);
    stop_sound();
    if let Some(mut state) = AUDIO_STATE.try_lock() {
        state.total_beeps_played += 1;
    }
}

/// Sonido de alerta o error
pub fn alert_beep() {
    play_tone(440, 90);
    play_tone(330, 120);
}

/// Melodía de escala o prueba interactiva
pub fn play_scale() {
    let scale = [
        (261, 80),  // Do (C4)
        (294, 80),  // Re (D4)
        (330, 80),  // Mi (E4)
        (349, 80),  // Fa (F4)
        (392, 80),  // Sol (G4)
        (440, 80),  // La (A4)
        (494, 80),  // Si (B4)
        (523, 140), // Do (C5)
    ];
    for &(freq, dur) in &scale {
        play_tone(freq, dur);
    }
}
