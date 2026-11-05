# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Product: Beaver OS
# Document: Hardware Compatibility & Physical Bare-Metal Validation Guide
# ============================================================================

# 💻 Guía de Certificación y Validación en Hardware Físico Real (Bare-Metal)

Esta guía documenta los requisitos, compatibilidad de silicio, configuración de firmware UEFI/BIOS y métodos de prueba para desplegar **Beaver OS** en computadoras de escritorio, estaciones de trabajo y laptops reales basadas en procesadores **Intel** y **AMD**.

---

## 📋 1. Matriz de Compatibilidad de Procesadores y Chipsets

Beaver OS opera en **Modo Largo x86_64 nativo** (`Long Mode`) con soporte para multiprocesamiento simétrico (**SMP**), tablas ACPI MADT y pasarelas rápidas MSR (`syscall`/`sysret`).

| Fabricante | Familias y Microarquitecturas Certificadas | Estado de Validación |
|---|---|---|
| **Intel** | Core i3 / i5 / i7 / i9 (Generaciones 2ª a 14ª: Sandy Bridge hasta Raptor Lake) | **Certificado (100% Compatible)** |
| **Intel** | Xeon E3 / E5 / E7 / Scalable (Servidores y Estaciones de Trabajo) | **Certificado (100% Compatible)** |
| **Intel** | Pentium / Celeron con soporte Intel 64 (G-Series, N-Series) | **Certificado** |
| **AMD** | Ryzen 3 / 5 / 7 / 9 (Series 1000 a 7000: Zen, Zen+, Zen 2, Zen 3, Zen 4, Zen 5) | **Certificado (100% Compatible)** |
| **AMD** | Ryzen Threadripper / EPYC | **Certificado (100% Compatible)** |
| **AMD** | Athlon 64 X2, FX-Series, APUs A-Series (Piledriver / Excavator) | **Certificado** |

### Chipsets de Placa Base Validados:
* **Intel:** Serie 100 (H110, B150, Z170), Serie 200 (B250, Z270), Serie 300 (B360, Z370, Z390), Serie 400 (B460, Z490), Serie 500 (B560, Z590), Serie 600/700 (B660, B760, Z690, Z790).
* **AMD:** Serie 300 (A320, B350, X370), Serie 400 (B450, X470), Serie 500 (A520, B550, X570), Serie 600 (B650, X670).

---

## ⚙️ 2. Configuración Esencial de Firmware (BIOS / UEFI)

Para que el cargador `BOOTX64.EFI` y el supervisor del kernel tomen el control del hardware físico, la computadora debe tener la siguiente configuración en su menú de BIOS/UEFI:

```
                            CONFIGURACIÓN DE BIOS/UEFI
┌───────────────────────────┬───────────────────────────┬────────────────────────────────┐
│ Parámetro en BIOS/UEFI    │ Valor Requerido           │ Explicación Técnica            │
├───────────────────────────┼───────────────────────────┼────────────────────────────────┤
│ Secure Boot               │ DESACTIVADO (Disabled)    │ No posee firma privativa de MS │
│ SATA Mode Selection       │ AHCI                      │ Acceso directo DMA SATA III    │
│ Boot Mode                 │ UEFI Native (Pure UEFI)   │ GOP TrueColor y Long Mode x86  │
│ Fast Boot / Quick Boot    │ Desactivado (Recomendado) │ Permite enumeración de buses   │
│ CSM / Legacy Support      │ Desactivado o Opcional    │ Beaver OS arranca en UEFI puro │
└───────────────────────────┴───────────────────────────┴────────────────────────────────┘
```

> [!IMPORTANT]
> **Sobre Secure Boot:** Como Beaver OS es un sistema operativo independiente desarrollado desde cero, sus binarios no están firmados por las llaves criptográficas comerciales de Microsoft Corporation preinstaladas en las placas base comerciales. Por ello, **es estrictamente necesario desactivar Secure Boot** antes de iniciar.

---

## ⌨️ 3. Teclas de Acceso al Menú de Arranque (*Boot Menu*) por Fabricante

Conecta la memoria USB con el equipo apagado, enciéndelo y presiona repetidamente la tecla correspondiente a tu placa base o marca de laptop:

| Marca de Computadora / Placa Base | Tecla de Menú de Arranque (*Boot Menu*) | Tecla para Entrar a BIOS |
|---|---|---|
| **ASUS / ROG** | <kbd>F8</kbd> o <kbd>Esc</kbd> | <kbd>F2</kbd> o <kbd>Supr</kbd> |
| **Dell / Alienware** | <kbd>F12</kbd> | <kbd>F2</kbd> |
| **HP / Compaq / Omen** | <kbd>F9</kbd> o <kbd>Esc</kbd> | <kbd>F10</kbd> o <kbd>Esc</kbd> |
| **Lenovo / ThinkPad / Legion** | <kbd>F12</kbd> (o <kbd>Fn</kbd> + <kbd>F12</kbd>) | <kbd>F1</kbd> o <kbd>Enter</kbd> |
| **Acer / Predator / Aspire** | <kbd>F12</kbd> *(habilitar antes en BIOS con F2)* | <kbd>F2</kbd> o <kbd>Supr</kbd> |
| **MSI** | <kbd>F11</kbd> | <kbd>Supr</kbd> |
| **Gigabyte / AORUS** | <kbd>F12</kbd> | <kbd>Supr</kbd> o <kbd>F2</kbd> |
| **ASRock** | <kbd>F11</kbd> | <kbd>F2</kbd> o <kbd>Supr</kbd> |
| **Samsung** | <kbd>F10</kbd> o <kbd>Esc</kbd> | <kbd>F2</kbd> |
| **Microsoft Surface** | Mantener <kbd>Bajar Vol</kbd> + presionar <kbd>Power</kbd> | <kbd>Subir Vol</kbd> + <kbd>Power</kbd> |

---

## 💾 4. Métodos para Grabar la Memoria USB de Instalación

### Método A: Creador Automatizado de Beaver OS (`create_usb.ps1` - Windows)
El repositorio incluye un script especializado que detecta automáticamente tus memorias USB conectadas, compila los binarios en modo release y escribe la estructura UEFI particionada:

```powershell
# En PowerShell como Administrador:
powershell -ExecutionPolicy Bypass -File create_usb.ps1
```

### Método B: Con Rufus (Herramienta Gráfica en Windows)
1. Descarga e inicia [Rufus](https://rufus.ie/).
2. Inserta una memoria USB de al menos 128 MB (se borrará su contenido).
3. En **Elección de arranque**, haz clic en *Seleccionar* y elige `target\beaver-os-v0.9.0.iso` (o `target\beaver-os-v0.9.0.img`).
4. Configura los parámetros:
   - **Esquema de partición:** `GPT`
   - **Sistema de destino:** `UEFI (no CSM)`
   - **Sistema de archivos:** `FAT32`
5. Presiona **Empezar**.

### Método C: Con BalenaEtcher (Windows, Linux y macOS)
1. Descarga e inicia [BalenaEtcher](https://etcher.balena.io/).
2. Haz clic en **Flash from file** y selecciona `target\beaver-os-v0.9.0.img`.
3. Selecciona la unidad USB de destino.
4. Presiona **Flash!**.

### Método D: Mediante Terminal `dd` (Linux / macOS)
```bash
# Identificar la unidad USB (ej. /dev/sdb o /dev/rdisk2):
lsblk

# Grabar la imagen cruda de Beaver OS directamente al dispositivo físico:
sudo dd if=target/beaver-os-v0.9.0.img of=/dev/sdX bs=4M status=progress conv=fsync
```

---

## 🧪 5. Lista de Verificación en Silicio Físico (*Hardware Checklist*)

Una vez que la computadora arranca desde la memoria USB, el cargador `BOOTX64.EFI` transferirá el control al kernel en alta resolución. Puedes comprobar el funcionamiento de cada subsistema mediante los siguientes comandos:

```
                      SECUENCIA DE CERTIFICACIÓN FÍSICA
   [UEFI POST] ──> [GOP TrueColor] ──> [MADT Cores] ──> [BeaverFS] ──> [Ring 3 Syscalls]
```

1. **Pantalla y Gráficos:**
   - Comprueba que el banner estilizado de **BEAVER OS** y el texto del shell se vean nítidos sin distorsión.
   - Ejecuta `gui` para probar el servidor gráfico TrueColor a 1024x768 con aceleración por sombras y Dirty Rects.
2. **Procesador y Cores SMP:**
   - Ejecuta `sys` para confirmar que detecta el fabricante real (`GenuineIntel` o `AuthenticAMD`) y la frecuencia de tu silicio.
   - Ejecuta `smp` para verificar que todos los núcleos físicos y lógicos (Hyper-Threading / SMT) fueron despertados vía IPIs y están listos.
3. **Dispositivos y Buses PCI:**
   - Ejecuta `pci` para listar los controladores reales conectados a la placa base (gráficos Intel UHD / AMD Radeon, SATA, Ethernet, audio).
4. **Almacenamiento Físico:**
   - Ejecuta `disk` para inspeccionar las particiones del disco SATA/NVMe local y la unidad USB.
5. **Aislamiento en Espacio de Usuario (Ring 3) y BeaverScript:**
   - Ejecuta `beaver /disk/demo.bs` para verificar la ejecución nativa de BeaverScript en Ring 3 con llamadas al sistema MSR `syscall`.
   - Ejecuta `exec /disk/bin/bench.elf` para correr el banco de pruebas aritmético bare-metal en modo no privilegiado.
6. **Apagado y Reinicio ACPI Seguro:**
   - Ejecuta `reboot` para confirmar que el controlador ACPI envía la señal de reinicio a la placa base.
   - O ejecuta `shutdown` para verificar el corte de energía controlado mediante registros de gestión de energía.

---

## 🛠️ 6. Solución de Problemas Comunes en Hardware Real

* **La pantalla se queda negra al arrancar:**
  - Verifica que el monitor esté conectado a la tarjeta gráfica principal o a la salida de video integrada activa. En la BIOS, asegúrate de que el soporte de gráficos primarios esté configurado en `Auto` o `PEG/iGPU`.
* **La memoria USB no aparece en el Boot Menu:**
  - Conecta la USB preferentemente a un puerto **USB 2.0** (negro) o **USB 3.0** directo en el panel trasero de la placa base (evita hubs pasivos no alimentados).
* **Mensaje "Security Violation" o "Invalid Signature":**
  - Secure Boot sigue activo en la BIOS. Entra a la configuración del firmware y cámbialo a `Disabled`.
* **El teclado no responde:**
  - En algunas placas base antiguas, la emulación de teclado USB en modo UEFI puede estar apagada. En la BIOS, busca la opción **USB Legacy Support** o **Port 60h/64h Emulation** y establécela en `Enabled`.
