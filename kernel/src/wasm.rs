// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: WebAssembly (Wasm v1) Binary Decoder and Runtime Engine for Ring 3
// ============================================================================

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// Encabezado mágico binario estándar de WebAssembly ("\0asm")
pub const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// Versión binaria estándar de WebAssembly (v1.0)
pub const WASM_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

// IDs de sección Wasm estándar
pub const SECTION_TYPE: u8 = 1;
pub const SECTION_FUNCTION: u8 = 3;
pub const SECTION_EXPORT: u8 = 7;
pub const SECTION_CODE: u8 = 10;

// Opcodes Wasm estándar
pub const OP_UNREACHABLE: u8 = 0x00;
pub const OP_NOP: u8 = 0x01;
pub const OP_BLOCK: u8 = 0x02;
pub const OP_LOOP: u8 = 0x03;
pub const OP_BR: u8 = 0x0C;
pub const OP_BR_IF: u8 = 0x0D;
pub const OP_RETURN: u8 = 0x0F;
pub const OP_END: u8 = 0x0B;
pub const OP_LOCAL_GET: u8 = 0x20;
pub const OP_LOCAL_SET: u8 = 0x21;
pub const OP_I32_CONST: u8 = 0x41;
pub const OP_I32_EQZ: u8 = 0x45;
pub const OP_I32_EQ: u8 = 0x46;
pub const OP_I32_LT_S: u8 = 0x48;
pub const OP_I32_LE_S: u8 = 0x4C;
pub const OP_I32_ADD: u8 = 0x6A;
pub const OP_I32_SUB: u8 = 0x6B;
pub const OP_I32_MUL: u8 = 0x6C;

#[derive(Debug, Clone)]
pub struct WasmExport {
    pub name: String,
    pub func_idx: usize,
}

#[derive(Debug, Clone)]
pub struct WasmFunc {
    pub type_idx: usize,
    pub num_locals: usize,
    pub bytecode: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct WasmModule {
    pub exports: Vec<WasmExport>,
    pub functions: Vec<WasmFunc>,
}

impl WasmModule {
    /// Parsea y valida un binario WebAssembly estándar (.wasm)
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 8 {
            return Err("WASM binary too short");
        }
        if bytes[0..4] != WASM_MAGIC {
            return Err("Invalid WASM magic bytes (not \\0asm)");
        }
        if bytes[4..8] != WASM_VERSION {
            return Err("Unsupported WASM version (expected v1)");
        }

        let mut offset = 8;
        let mut func_types: Vec<usize> = Vec::new();
        let mut exports: Vec<WasmExport> = Vec::new();
        let mut code_bodies: Vec<(usize, Vec<u8>)> = Vec::new();

        while offset < bytes.len() {
            let section_id = bytes[offset];
            offset += 1;
            let (section_len, len_bytes) = Self::decode_u32_leb128(&bytes[offset..])?;
            offset += len_bytes;

            let section_end = offset + section_len as usize;
            if section_end > bytes.len() {
                return Err("Section length exceeds binary bounds");
            }

            match section_id {
                SECTION_FUNCTION => {
                    let mut s_off = offset;
                    let (count, c_bytes) = Self::decode_u32_leb128(&bytes[s_off..])?;
                    s_off += c_bytes;
                    for _ in 0..count {
                        let (type_idx, t_bytes) = Self::decode_u32_leb128(&bytes[s_off..])?;
                        s_off += t_bytes;
                        func_types.push(type_idx as usize);
                    }
                }
                SECTION_EXPORT => {
                    let mut s_off = offset;
                    let (count, c_bytes) = Self::decode_u32_leb128(&bytes[s_off..])?;
                    s_off += c_bytes;
                    for _ in 0..count {
                        let (name_len, nl_bytes) = Self::decode_u32_leb128(&bytes[s_off..])?;
                        s_off += nl_bytes;
                        let name_bytes = &bytes[s_off..s_off + name_len as usize];
                        s_off += name_len as usize;
                        let name = core::str::from_utf8(name_bytes).map_err(|_| "Invalid UTF-8 in export")?;
                        let kind = bytes[s_off];
                        s_off += 1;
                        let (idx, idx_bytes) = Self::decode_u32_leb128(&bytes[s_off..])?;
                        s_off += idx_bytes;
                        if kind == 0 {
                            // Export de tipo función
                            exports.push(WasmExport {
                                name: String::from(name),
                                func_idx: idx as usize,
                            });
                        }
                    }
                }
                SECTION_CODE => {
                    let mut s_off = offset;
                    let (count, c_bytes) = Self::decode_u32_leb128(&bytes[s_off..])?;
                    s_off += c_bytes;
                    for _ in 0..count {
                        let (body_size, bs_bytes) = Self::decode_u32_leb128(&bytes[s_off..])?;
                        s_off += bs_bytes;
                        let body_end = s_off + body_size as usize;

                        // Parsear declaración de variables locales
                        let (local_vec_count, lvc_bytes) = Self::decode_u32_leb128(&bytes[s_off..])?;
                        let mut local_off = s_off + lvc_bytes;
                        let mut total_locals = 0;
                        for _ in 0..local_vec_count {
                            let (l_count, lc_bytes) = Self::decode_u32_leb128(&bytes[local_off..])?;
                            local_off += lc_bytes;
                            // Saltear el tipo de dato (ej: 0x7F = i32)
                            local_off += 1;
                            total_locals += l_count as usize;
                        }

                        let bytecode = bytes[local_off..body_end].to_vec();
                        code_bodies.push((total_locals, bytecode));
                        s_off = body_end;
                    }
                }
                _ => {
                    // Otras secciones (ej. Type, Data, Custom) se aceptan
                }
            }

            offset = section_end;
        }

        let mut functions = Vec::new();
        for (i, type_idx) in func_types.into_iter().enumerate() {
            if i < code_bodies.len() {
                let (num_locals, bytecode) = code_bodies[i].clone();
                functions.push(WasmFunc {
                    type_idx,
                    num_locals,
                    bytecode,
                });
            }
        }

        Ok(WasmModule { exports, functions })
    }

    /// Ejecuta una función exportada por nombre con los argumentos especificados
    pub fn execute(&self, export_name: &str, args: &[i32]) -> Result<i32, &'static str> {
        let export = self.exports.iter().find(|e| e.name == export_name)
            .ok_or("Exported function not found")?;

        let func = self.functions.get(export.func_idx)
            .ok_or("Function index out of bounds")?;

        let mut locals = Vec::new();
        for arg in args {
            locals.push(*arg);
        }
        for _ in 0..func.num_locals {
            locals.push(0);
        }

        let mut stack: Vec<i32> = Vec::new();
        let bytes = &func.bytecode;

        // Pre-calcular pares de matching block/loop -> end
        let mut matching_end = [0usize; 256];
        let mut control_start_stack = Vec::new();
        for (idx, &b) in bytes.iter().enumerate() {
            if b == OP_BLOCK || b == OP_LOOP {
                control_start_stack.push(idx);
            } else if b == OP_END {
                if let Some(start_idx) = control_start_stack.pop() {
                    if start_idx < 256 {
                        matching_end[start_idx] = idx;
                    }
                }
            }
        }

        // Estructura de control: (is_loop, start_ip, end_ip)
        let mut control_flow: Vec<(bool, usize, usize)> = Vec::new();
        let mut ip = 0;

        while ip < bytes.len() {
            let op = bytes[ip];
            match op {
                OP_I32_CONST => {
                    ip += 1;
                    let (val, len) = Self::decode_i32_leb128(&bytes[ip..])?;
                    ip += len;
                    stack.push(val);
                }
                OP_LOCAL_GET => {
                    ip += 1;
                    let (idx, len) = Self::decode_u32_leb128(&bytes[ip..])?;
                    ip += len;
                    let val = *locals.get(idx as usize).ok_or("Local index out of range")?;
                    stack.push(val);
                }
                OP_LOCAL_SET => {
                    ip += 1;
                    let (idx, len) = Self::decode_u32_leb128(&bytes[ip..])?;
                    ip += len;
                    let val = stack.pop().ok_or("Stack underflow on local.set")?;
                    if let Some(l) = locals.get_mut(idx as usize) {
                        *l = val;
                    } else {
                        return Err("Local index out of range");
                    }
                }
                OP_BLOCK => {
                    let block_start = ip;
                    ip += 2; // op + block_type
                    let end_idx = if block_start < 256 { matching_end[block_start] } else { bytes.len() - 1 };
                    control_flow.push((false, ip, end_idx));
                }
                OP_LOOP => {
                    let loop_start = ip;
                    ip += 2; // op + loop_type
                    let end_idx = if loop_start < 256 { matching_end[loop_start] } else { bytes.len() - 1 };
                    control_flow.push((true, ip, end_idx));
                }
                OP_BR => {
                    let (depth, _len) = Self::decode_u32_leb128(&bytes[ip + 1..])?;
                    let depth = depth as usize;
                    if depth >= control_flow.len() {
                        return Err("Branch depth out of range");
                    }
                    let target = control_flow[control_flow.len() - 1 - depth];
                    if target.0 {
                        // Loop: saltar al inicio del loop
                        ip = target.1;
                    } else {
                        // Block: saltar al final del bloque
                        ip = target.2 + 1;
                        control_flow.truncate(control_flow.len() - 1 - depth);
                    }
                }
                OP_BR_IF => {
                    let (depth, len) = Self::decode_u32_leb128(&bytes[ip + 1..])?;
                    let next_ip = ip + 1 + len;
                    let cond = stack.pop().ok_or("Stack underflow on br_if")?;
                    if cond != 0 {
                        let depth = depth as usize;
                        if depth >= control_flow.len() {
                            return Err("Branch depth out of range");
                        }
                        let target = control_flow[control_flow.len() - 1 - depth];
                        if target.0 {
                            ip = target.1;
                        } else {
                            ip = target.2 + 1;
                            control_flow.truncate(control_flow.len() - 1 - depth);
                        }
                    } else {
                        ip = next_ip;
                    }
                }
                OP_I32_ADD => {
                    ip += 1;
                    let b = stack.pop().ok_or("Stack underflow on i32.add")?;
                    let a = stack.pop().ok_or("Stack underflow on i32.add")?;
                    stack.push(a.wrapping_add(b));
                }
                OP_I32_SUB => {
                    ip += 1;
                    let b = stack.pop().ok_or("Stack underflow on i32.sub")?;
                    let a = stack.pop().ok_or("Stack underflow on i32.sub")?;
                    stack.push(a.wrapping_sub(b));
                }
                OP_I32_MUL => {
                    ip += 1;
                    let b = stack.pop().ok_or("Stack underflow on i32.mul")?;
                    let a = stack.pop().ok_or("Stack underflow on i32.mul")?;
                    stack.push(a.wrapping_mul(b));
                }
                OP_I32_LE_S => {
                    ip += 1;
                    let b = stack.pop().ok_or("Stack underflow on i32.le_s")?;
                    let a = stack.pop().ok_or("Stack underflow on i32.le_s")?;
                    stack.push(if a <= b { 1 } else { 0 });
                }
                OP_END => {
                    ip += 1;
                    if !control_flow.is_empty() {
                        control_flow.pop();
                    }
                }
                OP_NOP => {
                    ip += 1;
                }
                _ => {
                    return Err("Unimplemented WebAssembly opcode in engine");
                }
            }
        }

        stack.pop().ok_or("Function finished with empty operand stack")
    }

    /// Decodifica un entero no firmado en formato LEB128
    fn decode_u32_leb128(bytes: &[u8]) -> Result<(u32, usize), &'static str> {
        let mut result = 0u32;
        let mut shift = 0;
        let mut count = 0;

        for &byte in bytes {
            count += 1;
            result |= ((byte & 0x7F) as u32) << shift;
            if (byte & 0x80) == 0 {
                return Ok((result, count));
            }
            shift += 7;
            if shift >= 35 {
                return Err("LEB128 integer overflow");
            }
        }
        Err("Unexpected end of LEB128 stream")
    }

    /// Decodifica un entero con signo i32 en formato LEB128
    fn decode_i32_leb128(bytes: &[u8]) -> Result<(i32, usize), &'static str> {
        let mut result = 0i32;
        let mut shift = 0;
        let mut count = 0;

        for &byte in bytes {
            count += 1;
            result |= ((byte & 0x7F) as i32) << shift;
            shift += 7;
            if (byte & 0x80) == 0 {
                if shift < 32 && (byte & 0x40) != 0 {
                    result |= !0i32 << shift;
                }
                return Ok((result, count));
            }
            if shift >= 35 {
                return Err("Signed LEB128 overflow");
            }
        }
        Err("Unexpected end of signed LEB128 stream")
    }
}

/// Binario WebAssembly v1.0 real de 129 bytes que exporta:
/// "add", "sub", "mul" y "factorial"
pub fn get_sample_wasm_binary() -> &'static [u8] {
    &[
        0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, // Magic + Version
        0x01, 0x0C, 0x02, 0x60, 0x02, 0x7F, 0x7F, 0x01, 0x7F, 0x60, 0x01, 0x7F, 0x01, 0x7F, // Type Section
        0x03, 0x05, 0x04, 0x00, 0x00, 0x00, 0x01, // Function Section
        0x07, 0x1F, 0x04, // Export Section
        0x03, 0x61, 0x64, 0x64, 0x00, 0x00,       // export "add"
        0x03, 0x73, 0x75, 0x62, 0x00, 0x01,       // export "sub"
        0x03, 0x6D, 0x75, 0x6C, 0x00, 0x02,       // export "mul"
        0x09, 0x66, 0x61, 0x63, 0x74, 0x6F, 0x72, 0x69, 0x61, 0x6C, 0x00, 0x03, // export "factorial"
        0x0A, 0x41, 0x04, // Code Section (4 functions)
        0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6A, 0x0B, // func add: local.get 0, local.get 1, i32.add, end
        0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6B, 0x0B, // func sub: local.get 0, local.get 1, i32.sub, end
        0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6C, 0x0B, // func mul: local.get 0, local.get 1, i32.mul, end
        0x27, 0x01, 0x01, 0x7F, // func factorial: 1 local i32
        0x41, 0x01, 0x21, 0x01, // i32.const 1, local.set 1
        0x02, 0x40,             // block
        0x03, 0x40,             // loop
        0x20, 0x00, 0x41, 0x01, 0x4C, 0x0D, 0x01, // local.get 0, i32.const 1, i32.le_s, br_if 1
        0x20, 0x01, 0x20, 0x00, 0x6C, 0x21, 0x01, // local.get 1, local.get 0, i32.mul, local.set 1
        0x20, 0x00, 0x41, 0x01, 0x6B, 0x21, 0x00, // local.get 0, i32.const 1, i32.sub, local.set 0
        0x0C, 0x00,             // br 0 (repeat)
        0x0B, 0x0B,             // end loop, end block
        0x20, 0x01, 0x0B        // local.get 1, end
    ]
}

/// Ejecuta la demostración de WebAssembly comprobando todos los cálculos exportados
pub fn run_wasm_demo() -> Result<(i32, i32, i32, i32), &'static str> {
    let wasm_bytes = get_sample_wasm_binary();
    let module = WasmModule::parse(wasm_bytes)?;

    let res_add = module.execute("add", &[15, 27])?;          // 15 + 27 = 42
    let res_sub = module.execute("sub", &[100, 28])?;        // 100 - 28 = 72
    let res_mul = module.execute("mul", &[6, 7])?;            // 6 * 7 = 42
    let res_fac = module.execute("factorial", &[5])?;        // 5! = 120

    Ok((res_add, res_sub, res_mul, res_fac))
}
