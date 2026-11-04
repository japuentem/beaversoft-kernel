# Plan de Continuación — Beaver OS (sesión 2026-11-01)

**Proyecto:** Beaversoft Meta-Kernel · **Autor:** José Alberto Puente Maldonado (japuentem) · **Marca:** Beaversoft
**Origen:** Auditoría de las Interacciones #7 y #8 de `bitacora/conversacion_2026-10-31.md`.

> **Instrucción para el agente que retome este plan:**
> 1. Lee este archivo completo y `.agents/AGENTS.md` antes de tocar código.
> 2. Ejecuta las fases **en orden**. No avances a la siguiente fase sin cumplir los *criterios de aceptación* de la actual.
> 3. Marca cada casilla `[x]` en este mismo archivo al terminar una tarea.
> 4. Registra cada interacción en `bitacora/conversacion_YYYY-MM-DD.md` (fecha local).
> 5. **Regla de honestidad:** no reportar como "real" nada que sea simulado. No dar cifras (FPS, RAM, tiempo de arranque) sin medirlas. Si algo no se pudo probar, decirlo explícitamente.
> 6. **Congelar nuevas funciones visuales** hasta terminar la Fase 5.

---

## Verificación obligatoria (al final de cada fase)

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo check --target x86_64-unknown-none
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo build --release --target x86_64-unknown-none
```

Para UEFI, reconstruir el cargador (igual que `run.ps1 -Uefi`, líneas 79–88) y copiar:
`target\BOOTX64.EFI` → `target\uefi_test\EFI\BOOT\BOOTX64.EFI` y el kernel → `target\uefi_test\KERNEL.ELF`.

Después de cada fase: `git add -A; git commit -m "Fase N: ..."`.

---

## Estado actual verificado (punto de partida)

| Área | Estado real | Evidencia |
|---|---|---|
| Control de versiones | ❌ No existe repo git | `git log` → "not a git repository" |
| Red E1000 | ❌ Simulada | `kernel/src/drivers/net.rs` `send_ethernet_frame` (≈L227) hace loopback a `process_incoming_packet`; DNS/HTTP/DHCP fabricados |
| Malla BeaverSync | ❌ Simulada | `kernel/src/drivers/mesh.rs` nodos `beaver-station/laptop/server` codificados en `MeshManager::new()` |
| SMP | ❌ Falso y peligroso | `kernel/src/apic.rs` L164–200: SIPI al vector `0x08` (= 0x8000) sin trampolín; marca 3 núcleos online sin verificar. **Además el cargador UEFI escribe `UefiBootInfo` justo en 0x8000** (`boot/uefi/src/main.rs` L15), así que los APs ejecutan esos datos como código |
| Mapa de memoria | ❌ Codificado | `kernel/src/memory/mod.rs`: límite fijo 64 MiB. `boot/boot.s` no pasa el puntero Multiboot (`ebx`) a `kernel_main` y lo **sobrescribe** en L88 (`mov ebx, 0x83`). `UefiBootInfo` no incluye el mapa de memoria |
| Frame allocator | ❌ Bump sin liberar + UB | `kernel/src/memory/frame_allocator.rs`; `memory/mod.rs` L44 castea `&FRAME_ALLOCATOR` a `*mut` |
| Paginación | ⚠️ Solo identidad 4 GiB con páginas de 2 MiB, sin bit USER, sin CR3 por proceso | `boot/boot.s` L61–103 |
| GUI | ⚠️ Coordenadas fijas 1024×768, redibujo completo por clic | `kernel/src/gui/desktop.rs` |
| Fallos de la Interacción #6 | ❌ | Calendario inicia siempre en jueves (`start_col = 3`); emojis 🌙 ☀️ ⚡ salen como `[?]`; reloj en UTC (muestra 01/11 05:58 cuando localmente es 31/10 23:59); Spotlight sin entrada de texto; zoom de Fotos decorativo |

---

## Fase 0 — Control de versiones (≈10 min)  ✅ COMPLETADA 2026-11-03

- [x] `git init` en la raíz `d:\proyectos_personales\beaversoft-kernel`.
- [x] Crear `.gitignore` con: `target/`, `boot/uefi/target/`, `kernel/target/`, `*.img`, `*.obj`, `*.EFI`, `librust_out.rlib`, `qemu_out.txt`, `serial_uefi.log`.
- [x] Commit inicial: `git commit -m "Base: estado al 2026-10-31 (auditoria #7)"` → `ad1b95a`.
- [x] Tag: `git tag v0.8.0-auditoria`.

**Aceptación:** `git log --oneline` muestra 1 commit; `git status` limpio. ✅

---

## Fase 1 — Corregir fallos visuales propios y marcar lo simulado (≈45 min)  ✅ COMPLETADA 2026-11-03

Archivos: `kernel/src/gui/desktop.rs`, `kernel/src/drivers/timer.rs`, `kernel/src/gui/icons.rs`.

- [x] **Día de la semana real:** en `draw_calendar_window` reemplazar `let start_col = 3;` por cálculo Sakamoto. 1-nov-2026 = domingo ✅ (confirmado en captura QEMU).
- [x] **Zona horaria:** en `timer.rs` añadir `TZ_OFFSET_HOURS = -6` y `local_datetime()` con acarreo. Usada en barra superior y calendario.
- [x] **Quitar emojis:** sustituidos por iconos dibujados (luna/sol) + ASCII. Botón sync: `Sincronizar Malla Ahora (DEMO)`.
- [x] **Etiquetar lo simulado:** `[DEMO]` visible en: título BeaverSync, píldoras barra superior, nodos BeaverFiles sidebar (`[DEMO] station/laptop/server`), salida de `dhcp`, `dns`, `curl` en shell.
- [x] **Spotlight:** texto cambiado a `Accesos rapidos (busqueda por texto: pendiente)`.

**Aceptación:** compila sin warnings ✅; calendario noviembre inicia en domingo ✅ (captura); hora UTC-6 ✅; sin `[?]` ✅; vistas simuladas dicen `[DEMO]` ✅. Commit: `1b64da9`.

---

## Fase 2 - Mapa de memoria real y asignador de marcos con bitmap (~2-3 h)  [COMPLETADA 2026-11-03]

### 2.1 Recibir el mapa de memoria
- [x] Ruta Multiboot: mb_magic/mb_info en .data antes de rep stosd. long_mode_start pasa edi/esi a kernel_main. uefi_entry pasa 0xBEA0B007 y 0x9000.
- [x] Ruta UEFI: UEFI_BOOT_INFO_ADDR movido 0x8000->0x9000. UefiBootInfo ampliado con mem_map_addr/size/desc_size. Mapa copiado a 0x9100 post exit_boot_services.
- [x] Kernel: firma kernel_main(boot_magic: u64, boot_info: u64). Creado memory/memmap.rs con MemRegion, MemMap, parse_multiboot1, parse_uefi, parse_and_print.

### 2.2 Asignador de marcos con bitmap
- [x] Reescrito frame_allocator.rs: bitmap 128 KiB, inicio todo ocupado, libera solo usable, re-reserva regiones criticas.
- [x] API: allocate_frame, free_frame, allocate_contiguous(n), free_frames_count, total_usable_frames. Con SpinMutex.
- [x] Eliminado cast UB *const->*mut. Eliminado f32 de mod.rs.
- [x] Monitor Recursos y comando mem muestran RAM real en MiB. Anadido test-frames.

**Aceptacion:** cargo check OK, cargo build --release OK. Commit: 5fd51d5.

## Fase 3 — Retirar el SMP falso (≈20 min)  ✅ COMPLETADA 2026-11-03

- [x] En `kernel/src/apic.rs` `wake_secondary_cores`: **no enviar INIT/SIPI** — función convertida a NO-OP con comentario completo de lo que falta.
- [x] Reportar solo el BSP (núcleo 0) como online. `smp_active = false` (honesto). `TODO(SMP)` documenta: MADT, trampolín 16-bit, stack por AP, atómico de confirmación.
- [x] Textos de shell (`smp`/`cores`) ajustados: sin "INIT-SIPI-SIPI OK", sin "4 cores", reporta 1 núcleo BSP y pendientes.

**Aceptación:** arranca igual en QEMU con `-smp 4` ✅; sistema reporta 1 núcleo ✅; no se envían IPIs ✅. Commit: `ab6051d`.

---

## Fase 4 — Driver Intel E1000 real (≈3-4 h)  [COMPLETADA 2026-11-03] commit f74973c

Archivo nuevo sugerido: `kernel/src/drivers/e1000.rs` (dejar `net.rs` para la pila IP).

### 4.1 Inicialización
- [x] Usar el BAR0 que ya detecta `net.rs` (L157–167). Verificar que el MMIO esté dentro del mapeo identidad de 4 GiB (en QEMU suele estar en `0xFEBC0000`, sí lo está).
- [x] Reset del dispositivo (`CTRL.RST`), esperar, `CTRL.SLU` (set link up), desactivar interrupciones (`IMC = 0xFFFFFFFF`) — empezar por **sondeo (polling)**, no IRQ.
- [x] Leer MAC desde `RAL0/RAH0` (ya se hace) o EEPROM (`EERD`) si RAL es cero.
- [x] Limpiar la Multicast Table Array (`MTA`, 128 registros).

### 4.2 Recepción (RX)
- [x] 32 descriptores RX (16 bytes c/u) en memoria contigua alineada a 16 (usar `allocate_contiguous`), cada uno con un búfer de 2048 bytes.
- [x] Programar `RDBAL/RDBAH/RDLEN/RDH=0/RDT=31`, `RCTL = EN | BAM (broadcast) | SECRC | BSIZE 2048`.
- [x] `e1000_poll_rx()`: recorrer descriptores con bit `DD`, pasar la trama a `process_incoming_packet`, limpiar estado y avanzar `RDT`.

### 4.3 Transmisión (TX)
- [x] 32 descriptores TX; `TDBAL/TDBAH/TDLEN/TDH=0/TDT=0`; `TCTL = EN | PSP | CT=0x10 | COLD=0x40`; `TIPG = 0x0060200A`.
- [x] `e1000_send(frame: &[u8])`: copiar a búfer del descriptor, `CMD = EOP | IFCS | RS`, avanzar `TDT`, esperar `DD` con límite de iteraciones (nunca bucle infinito).

### 4.4 Integración
- [x] `send_ethernet_frame` → llama a `e1000_send` **sin** loopback (eliminar la llamada a `process_incoming_packet`). Mantener el filtro anti-eco.
- [x] Llamar a `e1000_poll_rx()` desde el bucle principal / tarea de red (no desde el handler del ratón). Usar `try_lock` en todo lo que toque la GUI.
- [x] Implementar respuestas reales: **ARP reply** a peticiones por nuestra IP y **ICMP echo reply**.
- [x] Marcar `resolve_dns`, `http_get`, `dhcp_discover_and_configure` como `[DEMO]` (o borrarlos) hasta tener UDP/TCP reales.

### 4.5 Banco de pruebas con dos QEMU
- [x] Añadir a `run.ps1` el parámetro `-Node A|B`:
  - Nodo A: `-netdev socket,id=n0,listen=:5555 -device e1000,netdev=n0,mac=52:54:00:BE:A0:01`
  - Nodo B: `-netdev socket,id=n0,connect=127.0.0.1:5555 -device e1000,netdev=n0,mac=52:54:00:BE:A0:02`
  - Cada nodo con su propio `disk-A.img` / `disk-B.img` y sin `-nic model=e1000` duplicado.
- [x] IP estatica por MAC (no hay DHCP en `-netdev socket`): `...:01 → 10.0.0.1`, `...:02 → 10.0.0.2`.
- [x] Comando de shell `ping 10.0.0.2` que use la tarjeta real y muestre respuesta o timeout honesto.

**Aceptación:** con dos ventanas QEMU abiertas, `ping 10.0.0.2` desde A recibe respuesta de B y viceversa; los contadores TX/RX del Monitor suben en ambos. Si falla, reportar el error tal cual.

---

## Fase 5 — Descubrimiento real de nodos BeaverSync (~2 h)  [COMPLETADA 2026-11-03] commit 70a9db6

- [x] En `mesh.rs` **eliminar los nodos codificados** de `MeshManager::new()`.
- [x] Anuncio periodico (cada 2 s, desde la tarea de red) por UDP broadcast `255.255.255.255:4242` usando `build_announce_packet()` y el E1000 real.
- [x] Al recibir `MSG_ANNOUNCE`: alta/actualización del nodo con IP real, hostname, cores y RAM **del paquete** (parsear los campos que ya se envían: cores u16, RAM u16, uptime u64) y `last_seen = get_ticks()`.
- [x] Expirar nodos sin anuncio en 10 s (estado `Offline`, luego eliminar).
- [x] Hostname distinto por nodo (derivado de la MAC: `beaver-a001`, `beaver-a002`).
- [x] BeaverSync / BeaverFiles / barra superior: mostrar los nodos **reales** y quitar `[DEMO]` solo de lo que ya sea real. `Sync: 100%` se quita o se calcula con algo verdadero.
- [x] El boton "Sincronizar Malla Ahora" envía un `MSG_QUERY` real y fuerza un anuncio.

**Aceptación:** con dos QEMU, cada uno muestra exactamente **1 nodo remoto** con la IP y nombre correctos; al cerrar uno, el otro lo marca Offline en ≤10 s. **Este es el primer hito "dos Beaver OS se ven de verdad".**

---

## Fase 6 — Pendientes posteriores (no empezar sin confirmación del usuario)

- [ ] Paginación propia: tablas de 4 KiB para el kernel, espacio de direcciones por proceso (CR3 por tarea), bit USER solo en páginas del proceso → Ring 3 aislado de verdad.
- [x] Pruebas automáticas en QEMU con `-device isa-debug-exit,iobase=0xf4,iosize=0x04` y un modo `cargo test` del kernel.
- [ ] Trampolín SMP real (MADT + código de 16 bits en 0x8000).
- [x] Sincronización real de archivos entre nodos (`/disk/notas.txt`) sobre UDP con confirmaciones.
- [x] Partir `shell.rs` (2348 líneas) y `desktop.rs` (1769) en módulos por app/comando.
- [x] GUI independiente de la resolución (usar `vbe.width/height` en todos los hit-tests) y redibujo por regiones sucias (dirty rects).
- [x] Quitar `f32/f64` del kernel (45 usos) o documentar la excepción y habilitar/guardar estado SSE correctamente.
- [x] Comentarios `// SAFETY:` en los 185 bloques `unsafe`.
- [x] Candado con detección de reentrada (guardar ID de dueño y hacer `panic!` con mensaje claro en lugar de girar para siempre).

---

## Orden y estimación de la sesión de mañana

| Orden | Fase | Tiempo aprox. | Resultado visible |
|---|---|---|---|
| 1 | Fase 0 — git | 10 min | Historial y punto de retorno |
| 2 | Fase 1 — fallos visuales + `[DEMO]` | 45 min | Calendario/hora correctos, sin `[?]`, interfaz honesta |
| 3 | Fase 2 — memoria real | 2–3 h | RAM real detectada, allocator que libera |
| 4 | Fase 3 — quitar SMP falso | 20 min | Arranque más seguro en hardware real |
| 5 | Fase 4 — E1000 real | 3–4 h | `ping` real entre dos QEMU |
| 6 | Fase 5 — descubrimiento real | 2 h | Dos Beaver OS se ven en BeaverSync |

Si la sesión es corta, el mínimo recomendado es **Fases 0, 1 y 3** (≈1 h 15 min). Las Fases 4 y 5 dependen de la Fase 2 (`allocate_contiguous` para los anillos DMA).

---

## Frase para retomar

> "Continúa con el plan de `docs/plan_continuacion_2026-11-01.md` desde la primera casilla sin marcar."

