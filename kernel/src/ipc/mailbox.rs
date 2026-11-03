// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Asynchronous Message Mailboxes & Channel Queue Management
// ============================================================================

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::string::String;
use crate::drivers::timer::get_ticks;

pub const MAX_MAILBOX_CAPACITY: usize = 32;

#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub id: u64,
    pub sender_tid: u64,
    pub sender_uid: u32,
    pub timestamp_ticks: u64,
    pub payload: String,
}

pub struct Mailbox {
    pub name: String,
    pub queue: VecDeque<IpcMessage>,
    pub max_capacity: usize,
    pub total_sent: u64,
    pub total_received: u64,
    next_msg_id: u64,
}

impl Mailbox {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            queue: VecDeque::new(),
            max_capacity: MAX_MAILBOX_CAPACITY,
            total_sent: 0,
            total_received: 0,
            next_msg_id: 1,
        }
    }

    /// Envía un mensaje estructurado al buzón
    pub fn send(&mut self, sender_tid: u64, sender_uid: u32, payload: &str) -> Result<u64, &'static str> {
        if self.queue.len() >= self.max_capacity {
            return Err("Buzon lleno (EAGAIN): Capacidad maxima de mensajes alcanzada");
        }

        let msg_id = self.next_msg_id;
        self.next_msg_id += 1;

        let msg = IpcMessage {
            id: msg_id,
            sender_tid,
            sender_uid,
            timestamp_ticks: get_ticks(),
            payload: String::from(payload),
        };

        self.queue.push_back(msg);
        self.total_sent += 1;
        Ok(msg_id)
    }

    /// Extrae el siguiente mensaje en cola (FIFO)
    pub fn receive(&mut self) -> Option<IpcMessage> {
        if let Some(msg) = self.queue.pop_front() {
            self.total_received += 1;
            Some(msg)
        } else {
            None
        }
    }

    /// Consulta el número de mensajes en espera
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }
}
