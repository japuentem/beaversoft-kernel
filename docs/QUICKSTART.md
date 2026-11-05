# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Product: Beaver OS
# Document: Guía de Inicio Rápido (Quickstart Guide) - Beaver OS v0.9.0
# ============================================================================

# ⚡ Guía de Inicio Rápido: Beaver OS v0.9.0
¡Bienvenido a **Beaver OS v0.9.0**! Esta guía te permitirá poner en marcha el sistema operativo en menos de 3 minutos, ya sea en un emulador local (QEMU) o en una computadora física mediante una memoria USB booteable.

---

## 📋 Requisitos Previos

- Computadora con procesador **x86_64** (Intel o AMD).
- **Opción A (Emulador):** Windows, Linux o macOS con QEMU instalado (o PowerShell en Windows para ejecución automática).
- **Opción B (Hardware Físico):** Unidad flash USB de al menos 64 MB y UEFI con *Secure Boot* desactivado temporalmente.

---

## 🚀 Método 1: Ejecución Instantánea en QEMU (Recomendado para Pruebas)

Si tienes clonado el repositorio de Beaver OS en tu equipo, puedes arrancar el sistema completo con un solo comando:

### En Windows (PowerShell):
```powershell
powershell -ExecutionPolicy Bypass -File run.ps1
```

### ¿Qué hace este comando?
1. Detecta automáticamente las herramientas de compilación de Rust bare-metal (`no_std`).
2. Compila el kernel supervisor `beaver-kernel` en modo release.
3. Empaqueta el medio de arranque UEFI.
4. Lanza QEMU emulando una máquina completa con procesador multinúcleo x86_64, 512 MB de RAM, tarjeta de red Intel Gigabit e1000, disco AHCI SATA y gráficos VBE TrueColor.

---

## 💾 Método 2: Grabado en Memoria USB para Computadora Física

Puedes instalar y arrancar Beaver OS directamente en cualquier PC o laptop moderna mediante UEFI.

### 1. Comprobar la Integridad de la Imagen
Antes de grabar la memoria, verifica la suma de comprobación criptográfica SHA-256:

```powershell
Get-FileHash target\beaver-os-v0.9.0.iso -Algorithm SHA256
```
**Hash oficial:** `E0039E532FA180554646B5A516C52F8FD0ACCCC607A796CEB1BAAD17DC307F95`

### 2. Grabado con Rufus (Windows)
1. Descarga e inicia [Rufus](https://rufus.ie/).
2. Inserta tu memoria USB (se formateará por completo).
3. En **Elección de arranque**, presiona *Seleccionar* y elige `target\beaver-os-v0.9.0.iso` (o `target\beaver-os-v0.9.0.img`).
4. Esquema de partición: **GPT** (Destino: **UEFI (no CSM)**).
5. Sistema de archivos: **FAT32** / Tamaño de clúster por defecto.
6. Haz clic en **Empezar**. Al finalizar, la unidad estará lista para arrancar.

### 3. Arranque en la Computadora Física
1. Conecta la memoria USB en tu PC o laptop apagada.
2. Enciende el equipo y pulsa repetidamente la tecla del menú de arranque (normalmente `F12`, `F11`, `F8` o `Esc` según el fabricante).
3. Asegúrate de que **Secure Boot** esté desactivado en la configuración del BIOS/UEFI.
4. Selecciona la unidad USB UEFI en la lista de dispositivos de arranque.
5. ¡En menos de 2 segundos verás la pantalla de bienvenida y el prompt de Beaver OS!

---

## 🧭 Primeros Pasos dentro de Beaver OS

Al arrancar, el sistema te recibirá con el banner oficial y el intérprete **Beaver Shell**:

```text
  ____   _____     _    __     __ _____  ____      ___   ____  
 | __ ) | ____|   / \   \ \   / /| ____||  _ \    / _ \ / ___| 
 |  _ \ |  _|    / _ \   \ \ / / |  _|  | |_) |  | | | |\___ \ 
 | |_) || |___  / ___ \   \ V /  | |___ |  _ <   | |_| | ___) |
 |____/ |_____|/_/   \_\   \_/   |_____||_| \_\   \___/ |____/ 

root@beaver:/# _
```

### Comandos Esenciales de la Consola:

| Comando | Descripción |
|---|---|
| `help` | Muestra el listado de todos los comandos integrados y utilidades. |
| `sys` | Muestra las especificaciones del hardware detectado (CPU, RAM, PCI, ACPI). |
| `dashboard` | Abre el monitor de telemetría del sistema en vivo con actualización en tiempo real. |
| `gui` | Inicia el entorno gráfico **Beaver Desktop** a resolución 1024x768 TrueColor. |
| `sh` | Conmuta al shell nativo en **Ring 3** (modo usuario aislado con llamadas `syscall` MSR). |
| `exec /disk/bin/hello.elf` | Ejecuta directamente un binario ELF64 en Ring 3 aislado. |
| `exec /disk/bin/bench.elf` | Ejecuta el benchmark de procesamiento en Ring 3. |
| `net` | Muestra la configuración de red y estado del adaptador Intel Gigabit. |
| `ping 10.0.2.2` | Envía paquetes ICMP Echo de prueba a través de la pila TCP/IP BeaverNet. |
| `curl http://beaver.lan/info.json` | Realiza una petición HTTP/1.1 nativa sobre socket TCP. |
| `ls /disk` | Explora los archivos y carpetas en la partición BeaverFS. |
| `logo` | Despliega el banner oficial y la ficha técnica del sistema. |
| `reboot` / `shutdown` | Reinicia o apaga la máquina de forma ordenada vía ACPI. |

---

## 🖥 Explorando el Entorno Gráfico (Beaver Desktop)

Escribe `gui` en la consola para acceder al entorno de escritorio. En él podrás:
- Mover el ratón e interactuar con ventanas mediante arrastre de barra de título.
- Abrir **BeaverFiles** para inspeccionar las particiones del disco.
- Utilizar **BeaverNote** para editar archivos de texto.
- Monitorear nodos en la red con **BeaverSync**.
- Para volver a la consola de texto, haz clic en el botón de salida o presiona `Esc`.

---

## 🛡 Soporte y Reportes

Si encuentras algún problema de compatibilidad con tu hardware o deseas proponer mejoras, visita el repositorio oficial:
- **GitHub:** [https://github.com/japuentem/beaversoft-kernel](https://github.com/japuentem/beaversoft-kernel)
- **Desarrollador:** José Alberto Puente Maldonado (`japuentem`)
- **Firma:** Beaversoft

