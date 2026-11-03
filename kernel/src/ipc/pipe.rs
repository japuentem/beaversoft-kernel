// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Circular Ring-Buffer Unidirectional Stream Pipe for IPC
// ============================================================================

extern crate alloc;

use alloc::string::String;

pub const PIPE_BUFFER_SIZE: usize = 1024;

pub struct Pipe {
    pub name: String,
    buffer: [u8; PIPE_BUFFER_SIZE],
    read_pos: usize,
    write_pos: usize,
    count: usize,
    pub total_bytes_written: usize,
    pub total_bytes_read: usize,
}

impl Pipe {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            buffer: [0u8; PIPE_BUFFER_SIZE],
            read_pos: 0,
            write_pos: 0,
            count: 0,
            total_bytes_written: 0,
            total_bytes_read: 0,
        }
    }

    /// Escribe bytes en la tubería
    pub fn write(&mut self, data: &[u8]) -> usize {
        let mut written = 0;
        for &byte in data {
            if self.count >= PIPE_BUFFER_SIZE {
                break; // Buffer lleno
            }
            self.buffer[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % PIPE_BUFFER_SIZE;
            self.count += 1;
            written += 1;
        }
        self.total_bytes_written += written;
        written
    }

    /// Lee bytes disponibles de la tubería
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let mut read = 0;
        for byte_slot in buf.iter_mut() {
            if self.count == 0 {
                break; // Buffer vacío
            }
            *byte_slot = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % PIPE_BUFFER_SIZE;
            self.count -= 1;
            read += 1;
        }
        self.total_bytes_read += read;
        read
    }

    pub fn available_to_read(&self) -> usize {
        self.count
    }

    pub fn available_to_write(&self) -> usize {
        PIPE_BUFFER_SIZE - self.count
    }
}
