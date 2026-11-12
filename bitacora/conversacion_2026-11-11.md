// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Bitácora: Sesión 2026-11-11
// Total de interacciones del día: 12
// ============================================================================

# Bitácora de Sesión — 2026-11-11

## Interacción #1 [13:09:20]

### 👤 Pregunta / Petición del Usuario:
como podria instalar el sistema operativo en un dispositivo movil?

---

### 🤖 Respuesta del Asistente (Antigravity):
Se explicaron de forma concisa y realista las 3 vías para ejecutar o instalar **Beaver OS** (`beaver-kernel`) en un teléfono o tablet (ARM64), distinguiendo lo que funciona hoy y lo que falta por desarrollar en bare-metal:

1. **Vía 1 — Sin tocar el sistema del teléfono (Funciona HOY mismo):**
   - Usando **Termux + QEMU** (`pkg install qemu-system-aarch64` o `qemu-system-x86_64`) o **UTM SE** (en iOS/Android).
   - Permite correr la imagen de **Beaver OS** (`beaver_os.iso` o el binario `aarch64-unknown-none`) directamente en el celular sin riesgo de brickearlo.

2. **Vía 2 — Instalación Bare-Metal vía Fastboot (`boot.img` en Android con Bootloader Desbloqueado):**
   - Los teléfonos Android (Pixel, Xiaomi, OnePlus, PinePhone) usan el bootloader **ABOOT / Little Kernel (LK)** o **U-Boot**, que espera un contenedor `boot.img` con encabezado **Android Boot Image** o **ARM64 Linux Image header** de 64 bytes + árbol de dispositivos (**DTB / Device Tree Blob**).
   - Se prueba en RAM sin borrar Android con `fastboot boot beaver_boot.img` (al reiniciar vuelve a Android normal).

3. **Qué falta en `beaver-kernel` para usar un teléfono físico de forma nativa (Bare-Metal):**
   - **Header ARM64 + lector de DTB (Flattened Device Tree):** Para descubrir las direcciones de memoria reales de cada SoC (Snapdragon, MediaTek, Exynos, Allwinner).
   - **SimpleFB / Framebuffer de Bootloader:** Leer el framebuffer pre-inicializado por el bootloader del teléfono para dibujar **Beaver Smart Hub** en la pantalla AMOLED/IPS sin escribir un driver propietario de GPU Adreno/Mali.
   - **Controlador I2C físico:** Conectar [`touchscreen.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/drivers/touchscreen.rs) a los registros MMIO del bus I2C del teléfono para recibir los toques reales de la pantalla.

================================================================================

## Interacción #2 [13:14:53]

### 👤 Pregunta / Petición del Usuario:
quiero instalarlo en un equipo samsung viejo que puedo usar para prueba, dame los pasos para probarlo primero en mi celular con una app como Vectras VM / UTM  y tambien dame los pasos y agrega la cabecera ARM64 + lector de Framebuffer (DTB) en boot_aarch64.s para poder usar fastboot boot

---

### 🤖 Respuesta del Asistente (Antigravity):
Se implementó la cabecera de arranque móvil ARM64 y el analizador binario real de Flattened Device Tree (DTB) + SimpleFB, junto con la guía paso a paso para un teléfono Samsung de pruebas:

1. **Cabecera Estándar ARM64 Boot Image (64 bytes) en [`boot/boot_aarch64.s`](file:///d:/proyectos_personales/beaversoft-kernel/boot/boot_aarch64.s):**
   - Firma mágica `0x644D5241` (`"ARM\x64"`) en el offset `0x38`, `text_offset = 0x80000`, `image_size` dinámico y descenso automático de `EL2 -> EL1h` si el bootloader del móvil arranca en modo hipervisor.
   - Preservación del registro `x0` (puntero físico al DTB entregado por S-Boot / ABL / LK / U-Boot) hacia `kernel_main_aarch64(dtb_ptr: u64)`.
2. **Parser Real de Flattened Device Tree (FDT `0xD00DFEED`) y `simple-framebuffer` en [`kernel/src/arch/aarch64/dtb.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/arch/aarch64/dtb.rs):**
   - Decodificación binaria Big-Endian de tokens `FDT_BEGIN_NODE`, `FDT_PROP`, `FDT_END_NODE` y `FDT_END`.
   - Extracción del modelo del dispositivo/SoC (`model`), rango de RAM (`/memory`) y parámetros de pantalla pre-inicializada por el bootloader (`simple-framebuffer`: `reg`, `width`, `height`, `stride`, formatos `a8r8g8b8` 32bpp y `r5g6b5` 16bpp típico de Samsung S-Boot).
   - Renderizado directo en el framebuffer físico cuando el nodo está presente.
3. **Guía Paso a Paso Entregada:**
   - **Opción A (Inmediata en el Samsung):** Uso de **Vectras VM / Limbo PC Emulator (Android)** cargando `beaver_os.iso`.
   - **Opción B (Bare-Metal en Samsung):** Nota técnica importante sobre Samsung (usa **S-Boot / Odin / Heimdall** con partición `boot.img` o **TWRP -> Install Image** en lugar de `fastboot` estándar, salvo que tenga TWRP/LineageOS con modo fastbootd) y comandos con `mkbootimg` / `heimdall`.
4. **Verificación (Regla 7):**
   - Compilación limpia en `x86_64-unknown-none` y `aarch64-unknown-none`, y suite `run.ps1 -Test` verificada al **100% (16/16)**.

================================================================================

## Interacción #3 [17:59:52]

### 👤 Pregunta / Petición del Usuario:
sube los ultimos cambios al repositorio

---

### 🤖 Respuesta del Asistente (Antigravity):
Se preparó y subió el commit al repositorio remoto en GitHub conteniendo todas las mejoras, correcciones y nuevas capacidades implementadas:
1. **Correcciones de Auditoría:** Resolución de deadlock en `cmd_sys.rs` (HDMI-CEC), eliminación de locks anidados en drivers CEC y táctil, implementación real de ARM PSCI 0.2 (`SYSTEM_RESET` / `SYSTEM_OFF` vía `hvc #0`), restauración de comentarios `// SAFETY:`, limpieza de warnings de compilación.
2. **Soporte Móvil ARM64:** Cabecera estándar ARM64 Boot Image (64 bytes) en `boot_aarch64.s`, analizador binario de Device Tree (DTB/FDT `0xD00DFEED`) y controlador `simple-framebuffer` en `dtb.rs` para arranque en smartphones y tablets.
3. **Control de Versiones:** Registro y sincronización en la rama `master`.

================================================================================

## Interacción #4 [18:09:26]

### 👤 Pregunta / Petición del Usuario:
me encontre con esto, y es a lo que quiero llegar
[Referencia a KneOS: portfolio web 3D interactivo con escritorio, ventanas, apps y terminal, acompañado de captura de pantalla retro/cyberpunk con iconos verdes de escritorio]

---

### 🤖 Respuesta del Asistente (Antigravity):
Se analizó la referencia de KneOS comparándola con el estado actual de Beaver OS:
1. **Diferenciación Técnica Fundamental:**
   - KneOS es una simulación web (JavaScript vanilla + Express + PostgreSQL).
   - **Beaver OS** es ingeniería bare-metal real en Rust (microkernel con Ring 0/Ring 3, paginación, BeaverFS con paridad, TCP/IP RTL8139, HDMI-CEC, touchscreen, x86_64 y ARM64).
2. **Estrategia para Alcanzar y Superar este Nivel de Impacto:**
   - **En el Kernel (Beaver Desktop):** Incorporar un set estilizado de aplicaciones visuales (Calculadora, BeaverNote, explorador con papelera/carpetas, juegos bare-metal como Snake/Pong, app BeaverMesh P2P) con iconografía retro-moderna de alta legibilidad.
   - **En la Web (Showcase 3D Real con WebAssembly / v86):** En lugar de hacer una maqueta simulada en JS, embeber una PC 3D que arranque directamente el binario/ISO real de Beaver OS en el navegador mediante v86 (WASM x86), permitiendo a cualquier visitante o reclutador interactuar con el sistema operativo genuino sin instalar nada.

================================================================================

## Interacción #5 [18:21:01]

### 👤 Pregunta / Petición del Usuario:
pero en su descripcion dice "arranca un sistema operativo propio" lo que me gusta es la interfaz del escritorio, muy sencilla con iconos, si alguien quiere usar gui seria buena idea que se mostrara algo asi, si no quiere, puede usar la linea de comando como linux

---

### 🤖 Respuesta del Asistente (Antigravity):
Se aclaró y validó la visión del usuario:
1. **Modelo Dual Flexible (Filosofía Unix / Linux):**
   - **Modo Terminal por defecto:** El sistema arranca directamente en **Beaver Shell** (`root@beaver:/# `), ligero, rápido, ideal para programadores, servidores o terminal bare-metal.
   - **Modo GUI opcional:** Si el usuario desea entorno gráfico, escribe `gui` o `startx` y entra a **Beaver Desktop**.
2. **Diseño de Escritorio Minimalista con Iconos (Estilo Retro / Cyberpunk / CRT):**
   - Cuadrícula de iconos limpios sobre el fondo del escritorio en la columna izquierda (Terminal, Archivos, Notas, Calculadora, Juegos, BeaverMesh, Papelera, Configuración).
   - Acceso con un solo clic o doble clic con el ratón o pantalla táctil.
   - Barra de tareas inferior discreta con botón de inicio, reloj en tiempo real y buscador.
3. **Propuesta de Implementación:**
   - Incorporar la cuadrícula de iconos directamente sobre el lienzo de `desktop.rs` manteniendo la compatibilidad con el dock actual o reemplazándolo por esta barra inferior minimalista con iconos en el escritorio.

================================================================================

## Interacción #6 [18:23:42]

### 👤 Pregunta / Petición del Usuario:
implementa la cuadricula de iconos

---

### 🤖 Respuesta del Asistente (Antigravity):
Se implementó la cuadrícula de iconos de escritorio interactiva en **Beaver Desktop**:
1. **Cuadrícula de Iconos en el Escritorio ([`kernel/src/gui/desktop.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/desktop.rs)):**
   - Matriz de 2 columnas x 6 filas (12 aplicaciones del sistema) ubicada estratégicamente en el flanco izquierdo (x: 28..196, y: 42..570):
     - **Columna 0:** Terminal (acento verde hacker), Archivos (BeaverFiles), BeaverNote (editor persistente), Monitor (CPU/RAM en tiempo real), Calculadora, Papelera de reciclaje.
     - **Columna 1:** BeaverMesh (malla P2P), Audio (BeaverAudio), Fotos (BeaverPhotos), Calendario (reloj RTC/eventos), TV / Hub (Smart Hub), BeaverInfo (CV/bio de autor).
   - Glifos vectoriales y pixel-art estilizados en cajas de 48x48 con bordes redondeados (r=12), relieve de luz superior, sombra y etiquetas tipográficas con sombra de alto contraste.
2. **Interactividad y Enrutamiento de Clics (`hit_test_desktop_grid`):**
   - El sistema detecta clics de ratón o pantalla táctil directamente sobre cada icono del escritorio, emitiendo retroalimentación de audio y trayendo la ventana seleccionada al frente (`unminimize_and_focus`).
   - Las ventanas en `init_defaults` arrancan minimizadas por defecto para ofrecer una experiencia inicial de escritorio completamente despejada y limpia como la referencia.
3. **Verificación de No Regresión (Regla 7):**
   - Compilación limpia sin errores ni warnings en `x86_64-unknown-none` y `aarch64-unknown-none`.
   - Suite automatizada de pruebas `run.ps1 -Test` aprobada al **100% (16/16)** con código de salida 33 (`0x10`).

================================================================================

## Interacción #7 [18:59:15]

### 👤 Pregunta / Petición del Usuario:
-terminal si abre la terminal pero dice CALC
-calculadora abre una terminal
-papelera abre explorador vfs
-audio permite sonar Beaver chime pero no termina y no deja seleccionar otro, tampoco deja navegar a Efectos o algun otro sonido
-fotos no deja cambiar al directorio Paisajes o algun otro
-Beaverinfo abre bloc de notas
-TV / Hub no se que muestra
[Se adjuntaron 7 capturas de pantalla de la interfaz de Beaver Desktop y Smart Hub evidenciando cada uno de los comportamientos]

---

### 🤖 Respuesta del Asistente (Antigravity):
Se auditaron y corrigieron exhaustivamente los 7 puntos reportados en Beaver Desktop y Beaver Smart Hub:

1. **Terminal sin textos `[CALC]` residuales:**
   - Se purgó la inyección de cadenas `[CALC]` en `GUI_TERMINAL.lines` que ocurría al abrir la terminal o calculadora. Al abrir la terminal se limpian las líneas huérfanas con `retain(|l| !l.starts_with("[CALC]"))`, manteniendo el prompt limpio `root@beaver:/# `.
2. **Ventana de Calculadora Gráfica Interactiva ([`WindowId::Calculator`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/window.rs)):**
   - Se creó una aplicación gráfica dedicada (270x360 px) en [`kernel/src/gui/apps_system.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/apps_system.rs) con pantalla digital LCD TrueColor, cuadrícula de botones táctiles (`0-9`, `+`, `-`, `*`, `/`, `=`, `C`, `DEL`, `00`), aritmética entera segura de 64 bits (`saturating_add`, `saturating_mul`, división protegida contra cero) y clic feedback acústico.
3. **Papelera de Reciclaje Visual (`FilesViewTab::Trash`):**
   - Se añadió la pestaña `Trash` en `BeaverFiles` y enrutamiento exclusivo para el icono de Papelera en el escritorio. Muestra una vista dedicada con icono de papelera vacía ("Papelera de Reciclaje (Vacia) | 0 elementos eliminados"), distinguiéndola del disco raíz `/disk`.
4. **BeaverAudio ([`kernel/src/gui/apps_media.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/apps_media.rs)):**
   - Se habilitó la selección interactiva de categorías en la barra lateral (`Canciones`, `Efectos`, `Sintesis`), el cambio de fila de pistas activas y la reproducción inmediata de 12 efectos acústicos y chimes distintos vía `play_tone` / `play_chime` en hardware real y PC Speaker / Intel HDA DMA.
5. **BeaverPhotos ([`kernel/src/gui/apps_media.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/apps_media.rs)):**
   - Se implementó la selección interactiva de pestañas de álbumes (`Todas`, `Paisajes`, `Malla P2P`), filtrando dinámicamente las miniaturas visuales generadas por código TrueColor y respondiendo a los clics del usuario.
6. **BeaverInfo — Ventana "Acerca de Beaver OS" ([`WindowId::About`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/window.rs)):**
   - Se reemplazó la apertura de BeaverNote por una ventana modal institucional estilizada (480x320 px) en [`apps_system.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/apps_system.rs) que detalla especificaciones del kernel bare-metal (`x86_64` / `ARM64`), memoria, arquitectura de protección Ring 3, autoría oficial de José Alberto Puente Maldonado (japuentem) y marca comercial Beaversoft.
7. **Beaver Smart Hub (TV / Hub) y Depuración Visual UTF-8:**
   - Se explicó en detalle el propósito de Beaver Smart Hub (interfaz 10-Foot UI de sala para Smart TVs con control remoto HDMI-CEC y kioscos táctiles domóticos como refrigeradores inteligentes, saliendo con `[ESC]` a Beaver Desktop).
   - Se eliminaron todos los caracteres multi-byte UTF-8 no soportados por el renderizador tipográfico ASCII (`DOMÓTICO`, `•`, `°C`, `Gráfica`, `Iluminación`, `Cálida`, `FRÍO`, `Electrodoméstico`), eliminando por completo los símbolos corruptos `?` observados en las capturas.

**Verificación:**
- Compilación cruzada exitosa en `x86_64-unknown-none` y `aarch64-unknown-none`.
- 16/16 pruebas de `run.ps1 -Test` aprobadas (100% de éxito, código 33).

================================================================================

## Interacción #8 [19:18:30]

### 👤 Pregunta / Petición del Usuario:
-que sera mejor, tener los iconos en el escritorio o en la barra inferior?
-que muestra la barra superior?
[Se adjuntaron 3 imágenes mostrando la cuadrícula de iconos del escritorio, el dock inferior y la barra superior]

---

### 🤖 Respuesta del Asistente (Antigravity):
Se brindó asesoría técnica y de diseño de UX sobre la disposición del escritorio y se desglosó detalladamente cada elemento de la barra superior:
1. **Evaluación de Iconos (Escritorio vs. Barra Inferior / Dock):**
   - En la captura se observaba redundancia al tener ambos elementos activos y apiñamiento horizontal en el dock inferior (etiquetas truncadas como "BeaverS...").
   - Se recomendó no saturar ambos espacios a la vez y definir una identidad clara para Beaver OS.
2. **Desglose de la Barra Superior (Top Bar):**
   - Explicación de los 7 elementos activos: Identidad `[B] Beaver OS`, menús contextuales, buscador Spotlight, monitor BeaverMesh P2P, conmutador de tema Día/Noche, acceso a TV/Hub y telemetría de hardware en tiempo real (RTC, usuario y uptime).

================================================================================

## Interacción #9 [19:26:00]

### 👤 Pregunta / Petición del Usuario:
-tener los iconos en el escritorio me suena a windows, tener los iconos en la barra inferior me suena a macos, quisiera que tengamos nuestra propia identidad, dame algunas propuestas
-de la barra superior quisiera que estuviera separada por marcos cada seccion o un pipe al menos

---

### 🤖 Respuesta del Asistente (Antigravity):
Se presentaron 4 propuestas de identidad original para Beaver OS (lejos de Windows y macOS) y se modularizó la barra superior:

1. **Barra Superior Modular Enmarcada con Separadores Pipe ([`kernel/src/gui/desktop.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/desktop.rs)):**
   - Se dividió la barra en 4 módulos autónomos con marcos de bordes redondeados (`draw_rounded_rect`), fondos sutiles y líneas divisorias verticales (`|` pipes):
     - **Módulo 1:** Emblema `[B] Beaver OS` + Menús principales en cápsula integrada con marco.
     - **Divisor `|` + Módulo 2:** Buscador Spotlight con marco propio + Telemetría BeaverMesh P2P con marco propio.
     - **Divisor `|` + Módulo 3:** Conmutadores de control alineados dinámicamente a la derecha (`Dia/Noche` y `TV/Hub` con marcos delimitados).
     - **Módulo 4:** Cápsula de telemetría de hardware (RTC + Usuario + Uptime) con marco independiente, eliminando cualquier solapamiento en cualquier resolución.
2. **Propuestas de Identidad Única para Beaver OS:**
   - **Propuesta 1 — Beaver Command Rail (Rail Lateral de Ingeniería / Telemetría HUD):** Barra vertical tipo consola aeroespacial con módulos hexagonales / métricas vivas de kernel (CPU, RAM, disco libre).
   - **Propuesta 2 — Radial Cyberdeck / HexHub Ocultable:** Escritorio 100% despejado que despliega un panel de control interactivo segmentado al invocarlo o pulsar una tecla rápida.
   - **Propuesta 3 — Beaver Deskbar Modular Superior (Inspirada en BeOS / SGI IRIX moderna):** Todo el sistema se gobierna desde la barra superior multifuncional con cajones desplegables, dejando el área de trabajo limpia y sin barra inferior.
   - **Propuesta 4 — Workspace Pinned Dock / Tiling Beaver:** Segmentos flotantes estilo estación de trabajo Unix técnica con la paleta de identidad Beaver (ámbar, obsidiana, cian).

**Verificación:** Compilación limpia en `x86_64-unknown-none` y `aarch64-unknown-none`.

================================================================================

## Interacción #10 [19:30:50]

### 👤 Pregunta / Petición del Usuario:
dame una imagen de las 4 propuestas para tener una mejor vision

---

### 🤖 Respuesta del Asistente (Antigravity):
Se generó y presentó un panel comparativo visual en alta resolución (`beaver_os_desktop_identity_concepts_1794447039896.jpg`) mostrando las 4 propuestas de identidad gráfica para **Beaver OS**:
- **Panel A (Beaver Command Rail):** Rail lateral de ingeniería con telemetría viva de kernel (CPU, RAM, disco, mesh) y terminal flotante.
- **Panel B (Beaver Modular Deskbar):** Estilo BeOS/SGI IRIX con escritorio 100% limpio y barra superior integrada con pestañas de tareas y lanzador.
- **Panel C (HexHub Cyberdeck Launcher):** Menú geométrico retráctil con visualizadores y acentos cian/ámbar.
- **Panel D (Workspace Matrix):** Tiling de ventanas con selector de espacios de trabajo inferior `[1: Core][2: Media][3: Mesh][4: Sys]`.

================================================================================

## Interacción #11 [19:39:55]

### 👤 Pregunta / Petición del Usuario:
me gusta la opcion B, implementala y ya vere si se queda o despues la cambio

---

### 🤖 Respuesta del Asistente (Antigravity):
Se implementó la **Opción B: Beaver Modular Deskbar** (Inspirada en BeOS / SGI IRIX modernizada) en [`kernel/src/gui/desktop.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/desktop.rs):

1. **Escritorio Limpio y Expansivo (Zero Ruido Visual):**
   - Se retiró la cuadrícula de iconos del escritorio y se eliminó el dock inferior redundante. El tapiz cósmico TrueColor queda 100% disponible para el flujo de trabajo multitarea.
2. **Lanzador Desplegable (Beaver App Drawer):**
   - El botón `[B] Beaver OS v` en la esquina superior izquierda despliega un menú flotante estilizado (`BEAVER_DRAWER`) con las 12 aplicaciones del sistema codificadas por color, cerrándose al hacer clic fuera o al seleccionar una app.
3. **Bandeja de Tareas en la Deskbar (Task Tabs):**
   - Cada ventana abierta se proyecta en tiempo real como una pestaña enmarcada en la barra superior (`[Terminal]`, `[Archivos]`, `[Calc]`, etc.).
   - La ventana activa al frente se resalta en cian/azul con punto indicador LED. Al hacer clic en una pestaña activa se minimiza; al hacer clic en otra, se trae al frente.
4. **Módulos de Herramientas y Telemetría Integrados:**
   - Buscador Spotlight `[Buscar]`, estado BeaverMesh P2P, conmutador de tema Día/Noche, botón directo a TV/Hub y cápsula de telemetría de hardware (RTC + Usuario + Uptime).
5. **Pie de Página Técnico Minimalista:**
   - Ticker sutil de una línea en el fondo: `Beaver OS Bare-Metal | BeaverFS Transaccional A/B | MSR Syscalls Ring 3 | Clic [B] para Lanzador | ESC = TTY1`.

**Verificación:**
- Compilación limpia en `x86_64-unknown-none` y `aarch64-unknown-none`.
- 16/16 pruebas superadas en `run.ps1 -Test` (100% de éxito, código 33).

================================================================================

## Interacción #12 [19:48:20]

### 👤 Pregunta / Petición del Usuario:
en Beaver OS donde se despliega el menu, se puede colocar con otro color de fondo para indicar que es un elemento especial que mostrara el menu?

---

### 🤖 Respuesta del Asistente (Antigravity):
Se personalizó el botón maestro del menú desplegable `[B] Beaver OS v` en [`kernel/src/gui/desktop.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/gui/desktop.rs) con un esquema de color de alto impacto visual y contraste distintivo:

1. **Estado Normal (Reposo):**
   - Fondo ámbar bronce profundo (`0x2D1703` en modo oscuro / `0xFEF3C7` en modo nórdico).
   - Borde dorado brillante en ámbar Beaver (`Color::BEAVER_AMBER` / `0xF59E0B`).
   - Texto dorado luminoso "Beaver OS" (`0xFDE68A`) con flecha ámbar `v`.
   - Emblema ámbar con letra "B" blanca.
2. **Estado Activo (Menú Desplegado):**
   - Fondo ámbar radiante pleno (`Color::BEAVER_AMBER`), borde dorado brillante `0xFBBF24`, texto oscuro obsidiana (`0x0F172A`) de máximo contraste, emblema oscuro y flecha invertida `^`.
3. **Resultado:**
   - El botón destaca de forma evidente y jerárquica frente a los demás elementos de la barra, indicando inmediatamente al usuario que es el activador principal del sistema operativo.

**Verificación:**
- Compilación cruzada exitosa en `x86_64-unknown-none` y `aarch64-unknown-none`.
- 16/16 pruebas automáticas aprobadas en `run.ps1 -Test` (100% de pase, código 33).

================================================================================


