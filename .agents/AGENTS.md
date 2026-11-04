# Instrucciones de Proyecto para Agentes de IA (AGENTS.md)
# Proyecto: Beaversoft Meta-Kernel
# Autor: José Alberto Puente Maldonado (japuentem)
# Marca: Beaversoft

## 1. Bitácora Automática Obligatoria de Cada Conversación

En cada sesión de trabajo, el agente de IA **DEBE registrar todas las interacciones** (tanto las preguntas/peticiones del usuario como las respuestas y explicaciones del asistente) en el archivo de bitácora diario:

- **Ruta del archivo diario:** `bitacora/conversacion_YYYY-MM-DD.md` (utilizando la fecha local del sistema).
- **Creación / Continuación:** Si el archivo del día no existe, se crea con el encabezado institucional de Beaversoft. Si ya existe, se añaden las nuevas interacciones al final respetando la numeración cronológica (`## Interacción #N [HH:MM:SS]`).
- **Estructura Estricta de Cada Entrada:**
  ```markdown
  ## Interacción #N [HH:MM:SS]

  ### 👤 Pregunta / Petición del Usuario:
  <Texto exacto o solicitud del usuario>

  ---

  ### 🤖 Respuesta del Asistente (Antigravity):
  <Resumen de la respuesta, decisiones de arquitectura y soluciones entregadas>

  ================================================================================
  ```
- **Actualización de Metadatos:** Mantener actualizado en el encabezado del archivo el conteo total de interacciones del día.

---

## 2. Principios de Arquitectura del Kernel

1. **Rust Bare-Metal (`no_std`):**
   - No usar la biblioteca estándar (`std`).
   - Usar `alloc::*` exclusivamente cuando el Heap dinámico esté disponible.
   - Respetar la seguridad de memoria y evitar bloques `unsafe` innecesarios.

2. **Compilación y Verificación Cruzada:**
   - Arquitectura objetivo: `x86_64-unknown-none`.
   - Antes de dar por finalizada una tarea, verificar siempre la compilación con:
     `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo check --target x86_64-unknown-none`
     `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo build --release --target x86_64-unknown-none`

3. **Filosofía Beaversoft (Diferenciación de Sistemas Tradicionales):**
   - **Transparencia Humana:** Monitores visuales claros, sin métricas ofuscadas ni buffers fantasma.
   - **Resiliencia Anti-Brickeo:** Protección de arranque con doble slot A/B y espejo de paridad.
   - **Protección Anti-OOM:** Memoria reservada de emergencia para el kernel supervisor.
   - **Aislamiento Seguro:** Ejecución en Ring 3 con pasarela MSR Syscalls.

4. **Preservación y Documentación:**
   - Mantener encabezados de autoría en cada archivo fuente:
     ```rust
     // ============================================================================
     // Project: Beaversoft Meta-Kernel
     // Author: José Alberto Puente Maldonado (japuentem)
     // Brand: Beaversoft
     // Description: ...
     // ============================================================================
     ```
   - No eliminar comentarios explicativos existentes.

5. **Prohibición Estricta de Datos Simulados y Código Hardcodeado (Honestidad y Veracidad):**
   - Queda terminantemente prohibido utilizar datos simulados, mocks o código hardcodeado para fingir funcionalidades.
   - Todo subsistema (red, almacenamiento, SMP, procesos, memoria, GUI, telemetría) DEBE operar con información, controladores de hardware, paquetes de red, tablas ACPI/MADT y estructuras reales del sistema.
   - Las pruebas, comandos de diagnóstico y suites de verificación deben medir y certificar el comportamiento auténtico en ejecución sobre hardware o QEMU.

---

## 3. Roles Especializados de Agencia (.agents/rules/)
- [Embedded Firmware Engineer](file:///d:/proyectos_personales/beaversoft-kernel/.agents/rules/embedded-firmware-engineer.md): Guía de desarrollo bare-metal, registros de I/O, interrupciones deterministas e inmunidad contra crashes.
- [Software Architect](file:///d:/proyectos_personales/beaversoft-kernel/.agents/rules/software-architect.md): Guía de diseño modular de microkernel, aislamiento de anillos y pasarelas de syscalls.

