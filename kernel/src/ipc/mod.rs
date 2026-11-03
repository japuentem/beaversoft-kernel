// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Inter-Process Communication (IPC) Manager, Mailboxes & Streaming Pipes
// ============================================================================

extern crate alloc;

pub mod mailbox;
pub mod pipe;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use crate::sync::SpinMutex;
pub use mailbox::{Mailbox, IpcMessage};
pub use pipe::Pipe;

pub struct IpcManager {
    mailboxes: Vec<Mailbox>,
    pipes: Vec<Pipe>,
}

impl IpcManager {
    pub const fn new() -> Self {
        Self {
            mailboxes: Vec::new(),
            pipes: Vec::new(),
        }
    }

    pub fn init(&mut self) {
        if self.mailboxes.is_empty() {
            self.mailboxes.push(Mailbox::new("system"));
            self.mailboxes.push(Mailbox::new("telemetry"));
            self.mailboxes.push(Mailbox::new("events"));
        }
        if self.pipes.is_empty() {
            self.pipes.push(Pipe::new("stdout_stream"));
        }
    }

    pub fn get_or_create_mailbox(&mut self, name: &str) -> &mut Mailbox {
        if let Some(idx) = self.mailboxes.iter().position(|m| m.name == name) {
            &mut self.mailboxes[idx]
        } else {
            self.mailboxes.push(Mailbox::new(name));
            self.mailboxes.last_mut().unwrap()
        }
    }

    pub fn get_mailbox(&self, name: &str) -> Option<&Mailbox> {
        self.mailboxes.iter().find(|m| m.name == name)
    }

    pub fn send_message(&mut self, channel: &str, sender_tid: u64, sender_uid: u32, payload: &str) -> Result<u64, &'static str> {
        let mbox = self.get_or_create_mailbox(channel);
        mbox.send(sender_tid, sender_uid, payload)
    }

    pub fn receive_message(&mut self, channel: &str) -> Result<IpcMessage, &'static str> {
        if let Some(mbox) = self.mailboxes.iter_mut().find(|m| m.name == channel) {
            mbox.receive().ok_or("Buzón vacío (EAGAIN): No hay mensajes pendientes")
        } else {
            Err("Canal no encontrado (ENOENT): El buzón especificado no existe")
        }
    }

    pub fn write_pipe(&mut self, name: &str, data: &[u8]) -> usize {
        if let Some(idx) = self.pipes.iter().position(|p| p.name == name) {
            self.pipes[idx].write(data)
        } else {
            let mut pipe = Pipe::new(name);
            let written = pipe.write(data);
            self.pipes.push(pipe);
            written
        }
    }

    pub fn read_pipe(&mut self, name: &str, buf: &mut [u8]) -> usize {
        if let Some(pipe) = self.pipes.iter_mut().find(|p| p.name == name) {
            pipe.read(buf)
        } else {
            0
        }
    }

    /// Genera un informe detallado y transparente del estado del subsistema IPC
    pub fn generate_status(&self) -> String {
        let mut out = String::from("==================== ESTADO DEL SUBSISTEMA IPC ====================\n");
        out.push_str(" [BUZONES DE MENSAJES ASINCRONOS (MAILBOXES)]\n");
        out.push_str("  Canal        Mensajes  Capacidad  Enviados  Recibidos  Estado\n");
        out.push_str("  -----------  --------  ---------  --------  ---------  --------\n");

        if self.mailboxes.is_empty() {
            out.push_str("  (Sin buzones activos)\n");
        } else {
            for m in &self.mailboxes {
                let status = if m.pending_count() == 0 {
                    "Vacio"
                } else if m.pending_count() >= m.max_capacity {
                    "LLENO"
                } else {
                    "Activo"
                };
                out.push_str(&format!(
                    "  {:<11}  {:>8}   {:>8}  {:>8}  {:>9}  {}\n",
                    m.name, m.pending_count(), m.max_capacity, m.total_sent, m.total_received, status
                ));
            }
        }

        out.push_str("\n [TUBERIAS DE STREAMING CIRCULAR (PIPES)]\n");
        out.push_str("  Nombre            Ocupado   Libre   Escrito   Leido     Capacidad\n");
        out.push_str("  ----------------  --------  ------  --------  --------  ---------\n");

        if self.pipes.is_empty() {
            out.push_str("  (Sin tuberías activas)\n");
        } else {
            for p in &self.pipes {
                out.push_str(&format!(
                    "  {:<16}  {:>5} B   {:>4} B  {:>6} B  {:>6} B  {} B\n",
                    p.name, p.available_to_read(), p.available_to_write(),
                    p.total_bytes_written, p.total_bytes_read, pipe::PIPE_BUFFER_SIZE
                ));
            }
        }
        out.push_str("===================================================================\n");
        out
    }
}

pub static IPC_MANAGER: SpinMutex<IpcManager> = SpinMutex::new(IpcManager::new());

/// Inicializa el subsistema de comunicación entre procesos
pub fn init_ipc() {
    IPC_MANAGER.lock().init();
}
