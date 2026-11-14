# Beaver OS — Especificación de Arquitectura: BeaverRAID
**Marca:** Beaversoft  
**Producto:** Beaver OS (`beaver-kernel`)  
**Módulo:** BeaverRAID (Subsistema de Arreglos de Almacenamiento por Software)  
**Autor:** José Alberto Puente Maldonado (@japuentem)  
**Fecha:** 2026-11-13  
**Estado:** Documento de Diseño y Expansión Futura (`pending/`)

---

## 1. Resumen Ejecutivo y Motivación

En entornos críticos, servidores de alta densidad, telecomunicaciones y dispositivos de borde (*Edge Computing*), la falla física de una unidad de almacenamiento masivo (SATA SSD/HDD o NVMe) no debe comprometer la disponibilidad del sistema operativo ni provocar caídas de servicio.

Actualmente, **Beaver OS** soporta nativamente arreglos por hardware (donde una controladora física como Dell PERC, HP Smart Array o LSI expone un LUN unificado al bus AHCI/PCI). Sin embargo, muchas computadoras industriales, routers soberanos y PCs de escritorio no cuentan con costosas controladoras RAID físicas dedicadas.

**BeaverRAID** es el diseño del subsistema nativo de arreglos por software para **Beaver OS**, desarrollado en **Rust bare-metal (`no_std`)**, concebido para:
1. **Eliminar la dependencia de software externo:** No requiere utilidades monolíticas heredadas de Linux como `mdadm` o demonios de espacio de usuario.
2. **Garantizar continuidad operativa 24/7:** Tolerancia a la desconexión o fallo catastrófico de discos físicos en caliente (*hot-plug failure*).
3. **Integración con BeaverFS y Dual Slot A/B:** Proteger tanto el sistema de archivos jerárquico como las particiones de arranque del sistema operativo.

---

## 2. Niveles de Arreglo Soportados

```
                     ┌───────────────────────────────────┐
                     │     Sistema de Archivos BeaverFS   │
                     │  y Particiones UEFI GPT Slot A/B  │
                     └─────────────────┬─────────────────┘
                                       │ LBA Virtual (0..Total)
                     ┌─────────────────▼─────────────────┐
                     │          BeaverRAID Engine        │
                     │      (Virtual Block Device)       │
                     └───────┬───────────────┬───────────┘
                             │               │
             ┌───────────────▼──────┐ ┌──────▼──────────────┐
             │  Puerto AHCI 0 (DMA) │ │ Puerto AHCI 1 (DMA) │
             │   Disco Físico A     │ │   Disco Físico B    │
             └──────────────────────┘ └─────────────────────┘
```

### 2.1. BeaverRAID 1 (Mirroring / Espejo de Alta Disponibilidad)
* **Requisito:** $N \ge 2$ discos de almacenamiento independientes.
* **Modelo de Escritura:** Atómica y síncrona. Cada operación `write_sector(lba, data)` se despacha simultáneamente a todas las unidades miembro del espejo. El comando solo se confirma al supervisor cuando ambas unidades completan la transferencia por DMA.
* **Modelo de Lectura:** Balanceo de carga round-robin o basado en latencia de cola. Si el Disco A está ocupado atendiendo una petición previa, la lectura del sector se despacha al Disco B, reduciendo los tiempos de espera a la mitad.
* **Inmunidad ante Fallos:** Si uno de los discos experimenta errores de I/O, tiempo de espera (*timeout*) o desconexión física:
  1. El kernel marca el disco en estado `Degraded` de forma instantánea.
  2. Todas las lecturas y escrituras se redirigen al disco sano sin emitir Kernel Panic ni interrumpir los procesos en Ring 3.
  3. Se notifica al administrador en la barra HUD de Beaver Shell: `[STORAGE ALERT: BeaverRAID 1 Degradado - Puerto 0 Offline]`.

### 2.2. BeaverRAID 0 (Striping de Alto Rendimiento)
* **Requisito:** $N \ge 2$ discos.
* **Modelo de Datos:** Segmentación de bloques LBA entre las unidades miembro:
  $$\text{disco\_index} = \text{lba} \pmod N, \quad \text{lba\_físico} = \left\lfloor \frac{\text{lba}}{N} \right\rfloor$$
* **Beneficio:** Multiplica el ancho de banda agregado por el número de canales SATA/NVMe disponibles. Ideal para sistemas de captura continua de datos de red o procesamiento multimedia intensivo.

### 2.3. BeaverRAID 5 (Paridad Distribuida Rotativa)
* **Requisito:** $N \ge 3$ discos físicos.
* **Operación de Paridad:** Cálculo de paridad binaria mediante operaciones XOR aceleradas:
  $$P = D_0 \oplus D_1 \oplus \dots \oplus D_{N-2}$$
* **Rotación Left-Symmetric:** El bloque de paridad rota dinámicamente entre los discos en cada franja (*stripe*) para prevenir desgaste asimétrico y evitar cuellos de botella en un disco dedicado.
* **Reconstrucción en Caliente (*Hot-Rebuild*):** Ante la sustitución de un disco dañado, un hilo en segundo plano del kernel recalcula los datos perdidos aplicando:
  $$D_{\text{fallido}} = P \oplus \left(\bigoplus_{i \neq \text{fallido}} D_i\right)$$
  permitiendo que el sistema siga operando mientras se regenera la redundancia completa.

---

## 3. Arquitectura del Superbloque BeaverRAID

Cada disco miembro de un arreglo contendrá una estructura de metadatos de 512 bytes (**Superbloque BeaverRAID**) grabada en el último sector reservado del disco físico (para no colisionar con la cabecera GPT ni con el MBR de arranque):

```rust
#[repr(C, packed)]
pub struct BeaverRaidSuperblock {
    /// Número mágico identificador: 0x52414944_42454156 ("BEAVRAID")
    pub magic: u64,
    /// Versión de la especificación BeaverRAID (ej: 0x0001_0000)
    pub version: u32,
    /// Nivel de RAID: 0 = Striping, 1 = Mirror, 5 = Parity, 10 = Stripe+Mirror
    pub raid_level: u8,
    /// Estado del arreglo: 0 = Clean, 1 = Degraded, 2 = Rebuilding, 3 = Corrupt
    pub array_state: u8,
    /// Número total de discos configurados en el arreglo
    pub total_disks: u8,
    /// Índice ordinal del disco actual dentro del arreglo (0..total_disks - 1)
    pub member_index: u8,
    /// UUID único de 128 bits del arreglo (compartido por todos los miembros)
    pub array_uuid: [u8; 16],
    /// UUID propio del disco físico individual
    pub device_uuid: [u8; 16],
    /// Contador de secuencia de escritura (detecta discos desactualizados)
    pub sequence_number: u64,
    /// Tamaño de bloque de segmentación (stripe size) en sectores (ej: 128 = 64 KiB)
    pub stripe_sectors: u32,
    /// Total de sectores útiles expuestos hacia el sistema operativo
    pub logical_sectors: u64,
    /// Mapa de sectores sincronizados durante reconstrucción
    pub sync_checkpoint: u64,
    /// Campos reservados para expansión futura
    pub reserved: [u8; 420],
    /// Checksum CRC32 IEEE 802.3 de los metadatos (previene split-brain y corrupción)
    pub crc32: u32,
}
```

### Reglas de Ensamblado Automático:
1. Durante el arranque en `init_storage()`, el kernel sondea los puertos SATA AHCI y los controladores NVMe.
2. Si detecta el número mágico `0x52414944_42454156` y valida el `crc32`, agrupa los discos por `array_uuid`.
3. Comprueba el `sequence_number`: si un disco tiene una secuencia menor, se identifica como desactualizado y se programa su resincronización.
4. Si el número de miembros activos satisface el mínimo del nivel RAID, el arreglo se monta automáticamente como el `BlockDevice` primario del sistema.

---

## 4. Integración con BeaverFS y Resiliencia Anti-Brickeo

1. **Transparencia para el VFS:** El sistema de archivos [`BeaverFS`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/storage/beaverfs.rs) no requiere conocer la topología de los discos; interactúa con el `RaidBlockDevice` de forma estándar.
2. **Protección Multicapa:**
   - **Capa 1 (Hardware):** BeaverRAID tolera la pérdida física de un disco completo.
   - **Capa 2 (Bloques / BeaverFS):** Paridad interna y checksums CRC32 corrigen errores silenciosos en sectores individuales (*bit rot*).
   - **Capa 3 (Firmware / Boot):** Particionamiento dual **Slot A/B** en el arreglo permite revertir actualizaciones defectuosas del kernel.

---

## 5. Herramientas de Consola y Telemetría Humana

Se proyecta la inclusión de comandos de diagnóstico en **Beaver Shell**:

* `raid status`: Muestra la matriz de discos, canales AHCI asociados, temperatura, sectores reasignados y estado del arreglo (`OPTIMAL` / `DEGRADED`).
* `raid add <puerto>`: Incorpora un nuevo disco en caliente para reemplazar una unidad fallida.
* `raid rebuild`: Inicia la resincronización en segundo plano con control de prioridad de CPU para no degradar la respuesta interactiva.
* `raid scrub`: Lee la totalidad de los datos y recalcula la paridad para detectar y corregir inconsistencias silenciosas.

---

## 6. Plan de Implementación por Fases

| Fase | Hito Técnico | Entregable |
| :---: | :--- | :--- |
| **Fase 1** | Trait `BlockDevice` ampliado y parser del Superbloque BeaverRAID | [`kernel/src/storage/raid/superblock.rs`](file:///d:/proyectos_personales/beaversoft-kernel/kernel/src/storage/raid/superblock.rs) |
| **Fase 2** | BeaverRAID 1 (Mirroring activo-activo y tolerancia a caídas) | Multiplexación de escrituras AHCI DMA simultáneas |
| **Fase 3** | BeaverRAID 0 (Striping con balanceo de carga LBA) | Benchmark comparativo de rendimiento de I/O en QEMU |
| **Fase 4** | BeaverRAID 5 (Paridad XOR distribuida y reconstrucción) | Hilo de recuperación en caliente en segundo plano |
| **Fase 5** | Automatización en `run.ps1` con múltiples discos virtuales | Suite de pruebas de desconexión en caliente de discos |

---
*Documento registrado en la arquitectura oficial de Beaversoft bajo las directrices del Meta-Kernel Beaver OS.*
