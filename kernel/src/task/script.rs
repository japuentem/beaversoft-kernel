// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: BeaverScript (BS) - Extended Native Scripting Language & Interpreter
//              Supports Functions, Arrays, Arithmetic, Control Loops, File I/O & Audio
// ============================================================================

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;
use alloc::format;
use crate::fs::vfs::VFS;

#[derive(Debug, Clone)]
pub enum Value {
    Number(i64),
    Str(String),
    Bool(bool),
    Array(Vec<Value>),
    Nil,
}

impl Value {
    pub fn to_string(&self) -> String {
        match self {
            Value::Number(n) => format!("{}", n),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => format!("{}", b),
            Value::Array(arr) => {
                let mut s = String::from("[");
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&v.to_string());
                }
                s.push(']');
                s
            }
            Value::Nil => String::from("nil"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Ring3Action {
    Print(String),
    Beep(u32, u32),
    FileWrite(String, String),
}

#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<String>,
}

pub struct BeaverScriptEnv {
    pub vars: Vec<(String, Value)>,
    pub functions: Vec<FunctionDef>,
    pub output: String,
    pub last_return: Option<Value>,
    pub ring3_actions: Vec<Ring3Action>,
}

impl BeaverScriptEnv {
    pub fn new() -> Self {
        Self {
            vars: Vec::new(),
            functions: Vec::new(),
            output: String::new(),
            last_return: None,
            ring3_actions: Vec::new(),
        }
    }

    pub fn set_var(&mut self, name: &str, val: Value) {
        for (n, v) in self.vars.iter_mut() {
            if n == name {
                *v = val;
                return;
            }
        }
        self.vars.push((name.to_string(), val));
    }

    pub fn get_var(&self, name: &str) -> Option<Value> {
        for (n, v) in &self.vars {
            if n == name {
                return Some(v.clone());
            }
        }
        None
    }

    pub fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        let func = self.functions.iter().find(|f| f.name == name).cloned();
        if let Some(func) = func {
            if func.params.len() != args.len() {
                return Err(format!(
                    "Funcion '{}' esperaba {} argumentos pero recibio {}",
                    name,
                    func.params.len(),
                    args.len()
                ));
            }

            // Crear ámbito local preservando variables externas
            let saved_vars = self.vars.clone();
            for (p, arg) in func.params.iter().zip(args.into_iter()) {
                self.set_var(p, arg);
            }

            let mut ret_val = Value::Nil;
            for stmt in &func.body {
                if let Some(ret_expr) = stmt.strip_prefix("return ") {
                    ret_val = self.eval_expression(ret_expr)?;
                    break;
                }
                self.execute_line(stmt)?;
                if let Some(val) = self.last_return.take() {
                    ret_val = val;
                    break;
                }
            }

            // Restaurar entorno
            self.vars = saved_vars;
            Ok(ret_val)
        } else {
            Err(format!("Funcion no definida: '{}'", name))
        }
    }

    pub fn eval_expression(&mut self, expr: &str) -> Result<Value, String> {
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            return Ok(Value::Nil);
        }

        // Literales de cadena entre comillas
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            return Ok(Value::Str(trimmed[1..trimmed.len() - 1].to_string()));
        }

        // Booleanos
        if trimmed == "true" {
            return Ok(Value::Bool(true));
        }
        if trimmed == "false" {
            return Ok(Value::Bool(false));
        }

        // Literales de Array [a, b, c]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = trimmed[1..trimmed.len() - 1].trim();
            if inner.is_empty() {
                return Ok(Value::Array(Vec::new()));
            }
            let mut elements = Vec::new();
            for item in inner.split(',') {
                elements.push(self.eval_expression(item.trim())?);
            }
            return Ok(Value::Array(elements));
        }

        // Invocación de función mediante 'call fn(args)'
        if let Some(rest) = trimmed.strip_prefix("call ") {
            if let Some(paren_idx) = rest.find('(') {
                if rest.ends_with(')') {
                    let fn_name = rest[..paren_idx].trim();
                    let args_str = &rest[paren_idx + 1..rest.len() - 1].trim();
                    let mut args = Vec::new();
                    if !args_str.is_empty() {
                        for arg_item in args_str.split(',') {
                            args.push(self.eval_expression(arg_item.trim())?);
                        }
                    }
                    return self.call_function(fn_name, args);
                }
            }
        }

        // Indexación de Array: arr[idx]
        if trimmed.ends_with(']') {
            if let Some(bracket_idx) = trimmed.find('[') {
                let var_name = trimmed[..bracket_idx].trim();
                let idx_expr = &trimmed[bracket_idx + 1..trimmed.len() - 1].trim();
                let idx_val = self.eval_expression(idx_expr)?;
                if let Value::Number(idx_num) = idx_val {
                    if let Some(Value::Array(arr)) = self.get_var(var_name) {
                        if idx_num >= 0 && (idx_num as usize) < arr.len() {
                            return Ok(arr[idx_num as usize].clone());
                        } else {
                            return Err(format!("Indice fuera de rango: {} (tamano {})", idx_num, arr.len()));
                        }
                    }
                }
            }
        }

        // Operaciones aritméticas y relacionales
        for op in &["+", "-", "*", "/", "%", "==", "!=", ">", "<"] {
            if let Some(idx) = trimmed.find(op) {
                if idx > 0 && idx + op.len() < trimmed.len() {
                    let left = self.eval_expression(&trimmed[..idx])?;
                    let right = self.eval_expression(&trimmed[idx + op.len()..])?;

                    match (left, right) {
                        (Value::Number(a), Value::Number(b)) => {
                            let res = match *op {
                                "+" => Value::Number(a + b),
                                "-" => Value::Number(a - b),
                                "*" => Value::Number(a * b),
                                "/" => {
                                    if b == 0 {
                                        return Err("Division por cero".into());
                                    }
                                    Value::Number(a / b)
                                }
                                "%" => Value::Number(a % b),
                                "==" => Value::Bool(a == b),
                                "!=" => Value::Bool(a != b),
                                ">" => Value::Bool(a > b),
                                "<" => Value::Bool(a < b),
                                _ => Value::Nil,
                            };
                            return Ok(res);
                        }
                        (Value::Str(a), Value::Str(b)) if *op == "+" => {
                            return Ok(Value::Str(format!("{}{}", a, b)));
                        }
                        (Value::Str(a), Value::Number(b)) if *op == "+" => {
                            return Ok(Value::Str(format!("{}{}", a, b)));
                        }
                        (Value::Str(a), Value::Array(b)) if *op == "+" => {
                            return Ok(Value::Str(format!("{}{}", a, Value::Array(b).to_string())));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Parseo de número literal
        if let Ok(num) = trimmed.parse::<i64>() {
            return Ok(Value::Number(num));
        }

        // Consulta de variable existente
        if let Some(val) = self.get_var(trimmed) {
            return Ok(val);
        }

        Err(format!("Variable o expresion no valida: '{}'", trimmed))
    }

    pub fn execute_line(&mut self, line: &str) -> Result<(), String> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            return Ok(());
        }

        // return expr
        if let Some(rest) = trimmed.strip_prefix("return ") {
            let val = self.eval_expression(rest)?;
            self.last_return = Some(val);
            return Ok(());
        }

        // let x = expresion
        if let Some(rest) = trimmed.strip_prefix("let ") {
            if let Some(eq_idx) = rest.find('=') {
                let var_name = rest[..eq_idx].trim();
                let expr = &rest[eq_idx + 1..];
                let val = self.eval_expression(expr)?;
                self.set_var(var_name, val);
                return Ok(());
            }
        }

        // println expresion
        if let Some(rest) = trimmed.strip_prefix("println ") {
            let val = self.eval_expression(rest)?;
            let s = val.to_string();
            self.output.push_str(&s);
            self.output.push('\n');
            self.ring3_actions.push(Ring3Action::Print(format!("{}\n", s)));
            return Ok(());
        }

        // print expresion
        if let Some(rest) = trimmed.strip_prefix("print ") {
            let val = self.eval_expression(rest)?;
            let s = val.to_string();
            self.output.push_str(&s);
            self.ring3_actions.push(Ring3Action::Print(s));
            return Ok(());
        }

        // beep(hz, ms)
        if let Some(rest) = trimmed.strip_prefix("beep(") {
            if let Some(end) = rest.find(')') {
                let args_str = &rest[..end];
                let parts: Vec<&str> = args_str.split(',').collect();
                if parts.len() == 2 {
                    let hz = parts[0].trim().parse::<u32>().unwrap_or(440);
                    let ms = parts[1].trim().parse::<u32>().unwrap_or(100);
                    crate::drivers::audio::play_tone(hz, ms);
                    self.output.push_str(&format!("[AUDIO BEEP: {} Hz, {} ms]\n", hz, ms));
                    self.ring3_actions.push(Ring3Action::Beep(hz, ms));
                    return Ok(());
                }
            }
        }

        // file_write(ruta, contenido)
        if let Some(rest) = trimmed.strip_prefix("file_write(") {
            if let Some(end) = rest.rfind(')') {
                let args_str = &rest[..end];
                if let Some(comma_idx) = args_str.find(',') {
                    let path_expr = &args_str[..comma_idx].trim();
                    let content_expr = &args_str[comma_idx + 1..].trim();
                    let path_val = self.eval_expression(path_expr)?;
                    let content_val = self.eval_expression(content_expr)?;

                    let path_str = path_val.to_string();
                    let content_bytes = content_val.to_string();
                    let mut vfs = VFS.lock();
                    if let Err(e) = vfs.create_file(&path_str, content_bytes.as_bytes(), 0, 0) {
                        return Err(format!("Error en file_write: {}", e));
                    }
                    self.ring3_actions.push(Ring3Action::FileWrite(path_str, content_bytes));
                    return Ok(());
                }
            }
        }

        // loop N { ... }
        if let Some(rest) = trimmed.strip_prefix("loop ") {
            if let Some(brace_idx) = rest.find('{') {
                let count_str = rest[..brace_idx].trim();
                let count = count_str.parse::<usize>().unwrap_or(1);
                let inside = rest[brace_idx + 1..].trim_end_matches('}').trim();
                for _ in 0..count {
                    self.execute_line(inside)?;
                }
                return Ok(());
            }
        }

        // Invocación directa de funcion 'call fn(args)' como sentencia
        if trimmed.starts_with("call ") {
            let _ = self.eval_expression(trimmed)?;
            return Ok(());
        }

        Err(format!("Sintaxis no reconocida: '{}'", trimmed))
    }
}

/// Ejecuta un script completo de BeaverScript y devuelve la salida generada (Modo Supervisor)
pub fn run_script(source: &str) -> Result<String, String> {
    let mut env = BeaverScriptEnv::new();
    let mut in_function: Option<FunctionDef> = None;

    for (line_num, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }

        // Procesar cuerpo de función multilínea
        if let Some(mut func) = in_function.take() {
            if line == "}" || line.ends_with('}') {
                let inside = line.trim_end_matches('}').trim();
                if !inside.is_empty() {
                    func.body.push(inside.to_string());
                }
                env.functions.push(func);
            } else {
                func.body.push(line.to_string());
                in_function = Some(func);
            }
            continue;
        }

        // Definición de función: fn nombre(arg1, arg2) { ... }
        if line.starts_with("fn ") {
            if let Some(brace_idx) = line.find('{') {
                let head = line[3..brace_idx].trim();
                if let Some(paren_idx) = head.find('(') {
                    let fn_name = head[..paren_idx].trim();
                    let params_str = head[paren_idx + 1..].trim_end_matches(')').trim();
                    let params = if params_str.is_empty() {
                        Vec::new()
                    } else {
                        params_str.split(',').map(|s| s.trim().to_string()).collect()
                    };

                    let inside = line[brace_idx + 1..].trim();
                    if inside.ends_with('}') {
                        // Función de una sola línea
                        let body_str = inside.trim_end_matches('}').trim();
                        let mut body = Vec::new();
                        if !body_str.is_empty() {
                            for stmt in body_str.split(';') {
                                let s = stmt.trim();
                                if !s.is_empty() {
                                    body.push(s.to_string());
                                }
                            }
                        }
                        env.functions.push(FunctionDef {
                            name: fn_name.to_string(),
                            params,
                            body,
                        });
                    } else {
                        // Inicio de función multilínea
                        let mut body = Vec::new();
                        if !inside.is_empty() {
                            body.push(inside.to_string());
                        }
                        in_function = Some(FunctionDef {
                            name: fn_name.to_string(),
                            params,
                            body,
                        });
                    }
                    continue;
                }
            }
        }

        if let Err(err) = env.execute_line(line) {
            return Err(format!("Error en linea {}: {} -> '{}'", line_num + 1, err, line));
        }
    }

    Ok(env.output)
}

/// Compila las acciones emitidas por el script a un binario ELF64 auténtico con instrucciones máquina x86-64
pub fn build_script_elf(actions: &[Ring3Action]) -> Vec<u8> {
    use crate::elf::{Elf64Header, Elf64ProgramHeader, ELF_TYPE_EXEC, ELF_MACHINE_X86_64, PT_LOAD, PF_R, PF_W, PF_X};
    use crate::memory::paging::USER_ELF_VADDR;

    let mut code: Vec<u8> = Vec::new();
    let mut data_pool: Vec<u8> = Vec::new();

    let data_base_vaddr = USER_ELF_VADDR + 0x1000;

    for action in actions {
        match action {
            Ring3Action::Print(msg) => {
                let off = data_pool.len();
                data_pool.extend_from_slice(msg.as_bytes());
                let msg_vaddr = data_base_vaddr + off as u64;

                // mov rax, 1 (SYS_WRITE)
                code.extend_from_slice(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
                // mov rdi, 1 (stdout)
                code.extend_from_slice(&[0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00]);
                // mov rsi, msg_vaddr (10 bytes: 0x48, 0xBE, 8 bytes LE)
                code.extend_from_slice(&[0x48, 0xBE]);
                code.extend_from_slice(&msg_vaddr.to_le_bytes());
                // mov rdx, msg_len (7 bytes: 0x48, 0xC7, 0xC2, 4 bytes LE)
                code.extend_from_slice(&[0x48, 0xC7, 0xC2]);
                code.extend_from_slice(&(msg.len() as u32).to_le_bytes());
                // syscall (2 bytes: 0x0F, 0x05)
                code.extend_from_slice(&[0x0F, 0x05]);
            }

            Ring3Action::Beep(hz, ms) => {
                // mov rax, 103 (SYS_BEEP)
                code.extend_from_slice(&[0x48, 0xC7, 0xC0, 0x67, 0x00, 0x00, 0x00]);
                // mov rdi, hz
                code.extend_from_slice(&[0x48, 0xC7, 0xC7]);
                code.extend_from_slice(&hz.to_le_bytes());
                // mov rsi, ms
                code.extend_from_slice(&[0x48, 0xC7, 0xC6]);
                code.extend_from_slice(&ms.to_le_bytes());
                // syscall
                code.extend_from_slice(&[0x0F, 0x05]);
            }

            Ring3Action::FileWrite(path, content) => {
                let p_off = data_pool.len();
                data_pool.extend_from_slice(path.as_bytes());
                data_pool.push(0); // Terminador nulo para SYS_OPEN
                let p_vaddr = data_base_vaddr + p_off as u64;

                let c_off = data_pool.len();
                data_pool.extend_from_slice(content.as_bytes());
                let c_vaddr = data_base_vaddr + c_off as u64;

                // mov rax, 2 (SYS_OPEN)
                code.extend_from_slice(&[0x48, 0xC7, 0xC0, 0x02, 0x00, 0x00, 0x00]);
                // mov rdi, p_vaddr
                code.extend_from_slice(&[0x48, 0xBF]);
                code.extend_from_slice(&p_vaddr.to_le_bytes());
                // mov rsi, 64 (O_CREAT)
                code.extend_from_slice(&[0x48, 0xC7, 0xC6, 0x40, 0x00, 0x00, 0x00]);
                // syscall
                code.extend_from_slice(&[0x0F, 0x05]);
                // mov r12, rax (guardar fd devuelto)
                code.extend_from_slice(&[0x49, 0x89, 0xC4]);
                // mov rax, 1 (SYS_WRITE)
                code.extend_from_slice(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
                // mov rdi, r12 (fd)
                code.extend_from_slice(&[0x4C, 0x89, 0xE7]);
                // mov rsi, c_vaddr
                code.extend_from_slice(&[0x48, 0xBE]);
                code.extend_from_slice(&c_vaddr.to_le_bytes());
                // mov rdx, content_len
                code.extend_from_slice(&[0x48, 0xC7, 0xC2]);
                code.extend_from_slice(&(content.len() as u32).to_le_bytes());
                // syscall
                code.extend_from_slice(&[0x0F, 0x05]);
                // mov rax, 3 (SYS_CLOSE)
                code.extend_from_slice(&[0x48, 0xC7, 0xC0, 0x03, 0x00, 0x00, 0x00]);
                // mov rdi, r12 (fd)
                code.extend_from_slice(&[0x4C, 0x89, 0xE7]);
                // syscall
                code.extend_from_slice(&[0x0F, 0x05]);
            }
        }
    }

    // Terminar limpiamente con SYS_EXIT (60), código 0
    // mov rax, 60
    code.extend_from_slice(&[0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00]);
    // xor rdi, rdi
    code.extend_from_slice(&[0x48, 0x31, 0xFF]);
    // syscall
    code.extend_from_slice(&[0x0F, 0x05]);

    // Rellenar con NOPs para alineación a 16 bytes
    while code.len() % 16 != 0 {
        code.push(0x90);
    }
    if data_pool.is_empty() {
        data_pool.push(0);
    }

    let mut elf = Vec::with_capacity(0x3000);

    // 1. Cabecera ELF64 (64 bytes)
    let header = Elf64Header {
        ident: [0x7F, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        elf_type: ELF_TYPE_EXEC,
        machine: ELF_MACHINE_X86_64,
        version: 1,
        entry: USER_ELF_VADDR,
        ph_offset: 64,
        sh_offset: 0,
        flags: 0,
        header_size: 64,
        ph_entry_size: 56,
        ph_num: 2, // 2 segmentos: código y datos
        sh_entry_size: 0,
        sh_num: 0,
        sh_str_index: 0,
    };

    let header_bytes = unsafe {
        core::slice::from_raw_parts(&header as *const Elf64Header as *const u8, 64)
    };
    elf.extend_from_slice(header_bytes);

    // 2. Program Header 1: Código ejecutable (PT_LOAD, RX, offset 0x1000)
    let ph1 = Elf64ProgramHeader {
        seg_type: PT_LOAD,
        flags: PF_R | PF_X,
        offset: 0x1000,
        vaddr: USER_ELF_VADDR,
        paddr: USER_ELF_VADDR,
        file_size: code.len() as u64,
        mem_size: code.len() as u64,
        align: 0x1000,
    };
    let ph1_bytes = unsafe {
        core::slice::from_raw_parts(&ph1 as *const Elf64ProgramHeader as *const u8, 56)
    };
    elf.extend_from_slice(ph1_bytes);

    // 3. Program Header 2: Datos de cadenas (PT_LOAD, RW, offset 0x2000)
    let ph2 = Elf64ProgramHeader {
        seg_type: PT_LOAD,
        flags: PF_R | PF_W,
        offset: 0x2000,
        vaddr: data_base_vaddr,
        paddr: data_base_vaddr,
        file_size: data_pool.len() as u64,
        mem_size: data_pool.len() as u64,
        align: 0x1000,
    };
    let ph2_bytes = unsafe {
        core::slice::from_raw_parts(&ph2 as *const Elf64ProgramHeader as *const u8, 56)
    };
    elf.extend_from_slice(ph2_bytes);

    // Rellenar hasta offset 0x1000 y anexar código de máquina
    while elf.len() < 0x1000 {
        elf.push(0);
    }
    elf.extend_from_slice(&code);

    // Rellenar hasta offset 0x2000 y anexar datos
    while elf.len() < 0x2000 {
        elf.push(0);
    }
    elf.extend_from_slice(&data_pool);

    elf
}

/// Ejecuta un script BeaverScript dentro de un sandbox aislado de Ring 3 (CPL=3)
/// con pasarela de fast syscalls MSR y protección Anti-BSOD
pub fn run_script_in_ring3(source: &str) -> Result<String, String> {
    let mut env = BeaverScriptEnv::new();
    let mut in_function: Option<FunctionDef> = None;

    for (line_num, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }

        if let Some(mut func) = in_function.take() {
            if line == "}" || line.ends_with('}') {
                let inside = line.trim_end_matches('}').trim();
                if !inside.is_empty() {
                    func.body.push(inside.to_string());
                }
                env.functions.push(func);
            } else {
                func.body.push(line.to_string());
                in_function = Some(func);
            }
            continue;
        }

        if line.starts_with("fn ") {
            if let Some(brace_idx) = line.find('{') {
                let head = line[3..brace_idx].trim();
                if let Some(paren_idx) = head.find('(') {
                    let fn_name = head[..paren_idx].trim();
                    let params_str = head[paren_idx + 1..].trim_end_matches(')').trim();
                    let params = if params_str.is_empty() {
                        Vec::new()
                    } else {
                        params_str.split(',').map(|s| s.trim().to_string()).collect()
                    };

                    let inside = line[brace_idx + 1..].trim();
                    if inside.ends_with('}') {
                        let body_str = inside.trim_end_matches('}').trim();
                        let mut body = Vec::new();
                        if !body_str.is_empty() {
                            for stmt in body_str.split(';') {
                                let s = stmt.trim();
                                if !s.is_empty() {
                                    body.push(s.to_string());
                                }
                            }
                        }
                        env.functions.push(FunctionDef {
                            name: fn_name.to_string(),
                            params,
                            body,
                        });
                    } else {
                        let mut body = Vec::new();
                        if !inside.is_empty() {
                            body.push(inside.to_string());
                        }
                        in_function = Some(FunctionDef {
                            name: fn_name.to_string(),
                            params,
                            body,
                        });
                    }
                    continue;
                }
            }
        }

        if let Err(err) = env.execute_line(line) {
            return Err(format!("Error en linea {}: {} -> '{}'", line_num + 1, err, line));
        }
    }

    // Compilar las acciones analizadas a un binario ELF64 auténtico y ejecutarlo en Ring 3
    let elf_bytes = build_script_elf(&env.ring3_actions);
    let parsed_elf = crate::elf::ElfBinary::parse(&elf_bytes)
        .map_err(|e| format!("Error en parseo ELF64 de BeaverScript: {}", e))?;

    parsed_elf.load_and_execute()
        .map_err(|e| format!("Error en ejecución Ring 3: {}", e))?;

    Ok(env.output)
}

/// Instala scripts de prueba predeterminados en /disk
pub fn init_disk_scripts() {
    let mut vfs = VFS.lock();

    let demo_script = b"// BeaverScript v1.0 - Script de Demostracion
println \"=== INICIANDO SCRIPT EN BEAVER OS ===\"
let a = 15
let b = 25
let total = a + b
println \"Calculo matematico a + b: \" + total
let mi_array = [100, 200, 300]
println \"Array persistente: \" + mi_array
println \"Elemento indexado [1]: \" + mi_array[1]
println \"Generando melodia acustica...\"
beep(523, 70)
beep(659, 70)
beep(784, 80)
loop 3 { println \"Iteracion de bucle interactivo\" }
println \"=== SCRIPT EJECUTADO CON EXITO EN RING 3 ===\"
";

    let func_script = b"// BeaverScript - Funciones, Arrays y Persistencia ATA
println \"=== DEMO DE FUNCIONES Y ARRAYS EN BEAVER OS ===\"

fn duplicar(n) {
    let res = n * 2
    return res
}

fn calcular_area(base, altura) {
    let prod = base * altura
    let area = prod / 2
    return area
}

let num = 25
let doble = call duplicar(num)
println \"El doble de 25 es: \" + doble

let base = 10
let altura = 8
let triangulo = call calcular_area(base, altura)
println \"Area del triangulo (10 x 8 / 2): \" + triangulo

let lista = [10, 20, 30, 40]
println \"Array de prueba: \" + lista
println \"Elemento lista[0]: \" + lista[0]
println \"Elemento lista[2]: \" + lista[2]

file_write(\"/disk/script_out.txt\", \"Salida generada por BeaverScript: Doble=\" + doble)
println \"Archivo /disk/script_out.txt escrito con exito en ATA!\"

beep(600, 80)
beep(880, 100)
println \"=== TEST DE FUNCIONES Y ARRAYS COMPLETADO ===\"
";

    let fib_script = b"// Serie de Fibonacci en BeaverScript
println \"Serie de Fibonacci (Primeros terminos):\"
let f0 = 0
let f1 = 1
println f0
println f1
let f2 = f0 + f1
println f2
let f3 = f1 + f2
println f3
let f4 = f2 + f3
println f4
let f5 = f3 + f4
println f5
println \"Fibonacci completado con exito.\"
";

    if !vfs.exists("/disk/demo.bs") {
        let _ = vfs.create_file("/disk/demo.bs", demo_script, 0, 0);
    }
    if !vfs.exists("/disk/functions.bs") {
        let _ = vfs.create_file("/disk/functions.bs", func_script, 0, 0);
    }
    if !vfs.exists("/disk/fibonacci.bs") {
        let _ = vfs.create_file("/disk/fibonacci.bs", fib_script, 0, 0);
    }
}
