<!-- ============================================================================ -->
<!-- Project: Beaversoft Meta-Kernel                                              -->
<!-- Author: José Alberto Puente Maldonado (japuentem)                            -->
<!-- Brand: Beaversoft                                                            -->
<!-- Product: Beaver OS                                                           -->
<!-- Description: Official Repository Documentation (English)                     -->
<!-- ============================================================================ -->

<div align="center">

**[ 🇺🇸 English Version ](README.md)** • **[ 🇪🇸 Versión en Español ](README.es.md)**

<br/>

<img src="assets/beaver_os_cyber_banner.png" alt="Beaver OS Banner" width="800"/>

### ⚡ High-Performance, Independent x86_64 Bare-Metal Operating System written in Pure Rust (`no_std`)

*Featuring Ring 3 User-Mode Hardware Isolation, Native UEFI GOP TrueColor Compositor, and SMP Multiprocessing.*

[![Rust](https://img.shields.io/badge/Rust-no__std%20(Nightly)-orange.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Architecture](https://img.shields.io/badge/Architecture-x86__64%20Long%20Mode-blue.svg?style=for-the-badge&logo=intel)](https://software.intel.com/)
[![Boot](https://img.shields.io/badge/Boot-UEFI%20Native%20GOP-green.svg?style=for-the-badge)](docs/UEFI_BOOT_GUIDE.md)
[![Privilege](https://img.shields.io/badge/Isolation-Ring%203%20MSR%20Syscalls-purple.svg?style=for-the-badge)](docs/RELEASE_NOTES_v0.9.0.md)
[![SMP](https://img.shields.io/badge/Multiprocessing-SMP%20%7C%20ACPI%20MADT-red.svg?style=for-the-badge)](docs/RELEASE_NOTES_v0.9.0.md)
[![License](https://img.shields.io/badge/License-Source--Available%20%7C%20Trademark%20Protected-brightgreen.svg?style=for-the-badge)](LICENSE)
[![Version](https://img.shields.io/badge/Release-v0.9.0-cyan.svg?style=for-the-badge)](docs/RELEASE_NOTES_v0.9.0.md)

<p align="center">
  <a href="#-key-architectural-pillars">Architectural Pillars</a> •
  <a href="#-architecture-overview">Architecture Diagram</a> •
  <a href="#-benchmarks-beaver-os-vs-linux-vs-freebsd">Benchmarks</a> •
  <a href="#-quickstart">Quickstart</a> •
  <a href="#-kernel-source-map">Kernel Source Map</a> •
  <a href="#-hardware-compatibility">Hardware Matrix</a> •
  <a href="#-license--trademark-policy">License</a>
</p>

</div>

---

## 🌟 About Beaver OS

**Beaver OS** is an independent, production-grade operating system written from scratch in **Bare-Metal Rust (`no_std`)**, architected specifically for **64-bit Intel and AMD processors (`x86_64 Long Mode`)**.

Unlike most modern operating systems, **Beaver OS is neither Unix, Linux, nor BSD**. It shares no lineage or source code with existing kernels. Every single subsystem—from 4-level paging memory managers and MSR `syscall`/`sysret` fast transitions, to the SMP round-robin task scheduler, the **BeaverFS** parity filesystem, the native Ethernet network stack, and the TrueColor GOP compositor—has been engineered with strict memory safety, zero simulated code, and direct bare-metal execution.

---

## 🏛️ Key Architectural Pillars

```
                                 BEAVER OS ARCHITECTURE
 ┌──────────────────────────────────────────────────────────────────────────────────┐
 │  USER SPACE (RING 3 - CPL=3)                                                     │
 │  ┌───────────────────────┐  ┌───────────────────────┐  ┌──────────────────────┐  │
 │  │      BEAVER SHELL     │  │   BEAVERSCRIPT JIT    │  │  ELF64 USER BINARIES │  │
 │  │   (Interactive POSIX) │  │   (/SCRIPTS/*.bs)     │  │  (/BIN/BENCH.ELF)    │  │
 │  └───────────┬───────────┘  └───────────┬───────────┘  └──────────┬───────────┘  │
 └──────────────┼──────────────────────────┼─────────────────────────┼──────────────┘
                │                          │                         │
         MSR Fast Syscalls (IA32_LSTAR: SYS_WRITE, SYS_READ, SYS_BEEP, SYS_EXIT)
                ▼                          ▼                         ▼
 ┌──────────────────────────────────────────────────────────────────────────────────┐
 │  MICROKERNEL SUPERVISOR (RING 0 - CPL=0)                                         │
 │  ┌───────────────────────┐  ┌───────────────────────┐  ┌──────────────────────┐  │
 │  │  Per-Process PML4/CR3 │  │  SMP Multi-Core Sched │  │  Anti-OOM Supervisor │  │
 │  │  Paging (4KB Isolation│  │  (IPIs / ACPI MADT)   │  │  Anti-BSOD Protector │  │
 │  └───────────────────────┘  └───────────────────────┘  └──────────────────────┘  │
 │  ┌───────────────────────┐  ┌───────────────────────┐  ┌──────────────────────┐  │
 │  │  BeaverFS Journaling  │  │  Network BeavMesh     │  │  GOP GUI Compositor  │  │
 │  │  (Parity Mirror A/B)  │  │  (DHCP/TCP/UDP/Mesh)  │  │  (Dirty Rects 1024)  │  │
 │  └───────────────────────┘  └───────────────────────┘  └──────────────────────┘  │
 └──────────────────────────────────────────────────────────────────────────────────┘
```

1. **Ironclad Ring 3 Isolation:** Userland binaries execute exclusively in unprivileged space (`CPL=3`) with dedicated per-process PML4 page tables. An application crash or segfault never compromises kernel integrity.
2. **Sub-Microsecond MSR Transitions:** Utilizes `IA32_LSTAR` register-based syscall dispatching (`syscall`/`sysret`), reducing privilege context switch overhead to ~42 CPU cycles (over 2× faster than legacy interrupt gates).
3. **Native UEFI GOP Graphics:** Boots cleanly via `BOOTX64.EFI` directly into high-resolution TrueColor mode (1024×768×32bpp) without 16-bit legacy BIOS or VGA text mode dependencies.
4. **Anti-Brick & Anti-Crash Resiliency:** Built-in Anti-BSOD kernel protection intercepts ring-3 page faults, kills offending processes, and preserves system uptime. Storage operations are protected by BeaverFS journaling and A/B dual-slot recovery.
5. **Ultra-Low Memory Footprint:** Boots and operates comfortably in **~14 MB of RAM**, making it ideal for everything from micro-servers and laptops to mission-critical automotive ECUs and embedded systems.
6. **Decentralized P2P Mesh (BeaverMesh & BeaverSync):** Native broadcast/unicast protocol over UDP port `:4242` that enables peer-to-peer discovery, shared compute tasks, and distributed filesystem replication without cloud intermediaries.

---

## 📊 Benchmarks: Beaver OS vs Linux vs FreeBSD

Identical hardware testbench executing 100 consecutive iterations of the **Sieve of Eratosthenes** (calculating prime numbers up to 1,000,000):

| Performance & Efficiency Metric | Beaver OS v0.9.0 (Bare-Metal) | Arch Linux (Kernel 6.8.9) | FreeBSD 14.1-RELEASE | Beaver OS Advantage |
|---|---|---|---|:---:|
| **Compute Time (1M Primes ×100)** | **148 ms** | 162 ms | 179 ms | **+8.6% faster** |
| **Boot-to-Shell Time (POST to prompt)** | **< 1.8 seconds** | 6.4 seconds | 9.2 seconds | **3.5× faster** |
| **Idle Base RAM Usage** | **~14 MB** | ~480 MB | ~290 MB | **95% less RAM** |
| **Syscall Transition Latency** | **~42 CPU cycles** | ~85 CPU cycles | ~92 CPU cycles | **2× lower overhead** |
| **Fault Recovery in Ring 3** | **Instant Anti-BSOD Catch** | SIGSEGV Handler | SIGSEGV Handler | **Zero kernel panic** |

---

## ⚡ Quickstart

### Prerequisites
* Windows 10/11 or Linux with [Rust Nightly](https://www.rust-lang.org/) (`cargo`)
* [QEMU](https://www.qemu.org/) (for virtualized emulation) or any x86_64 PC with UEFI support

### Option 1: Run in QEMU Emulator (Windows PowerShell)

Beaver OS includes automated build and launch scripts:

```powershell
# 1. Clone the repository
git clone https://github.com/japuentem/beaversoft-kernel.git
cd beaversoft-kernel

# 2. Build and run in QEMU (interactive shell with GUI GOP window)
powershell -ExecutionPolicy Bypass -File run.ps1

# Or run directly from the pre-built ISO release:
powershell -ExecutionPolicy Bypass -File run.ps1 -Iso
```

### Option 2: Flash to a Bootable USB for Real Hardware

```powershell
# Automatically formats and writes the GPT UEFI boot partition to your target USB drive:
powershell -ExecutionPolicy Bypass -File create_usb.ps1
```

*Alternatively, flash `target/beaver-os-v0.9.0.iso` with [Rufus](https://rufus.ie/) (Select **GPT** / **UEFI**) or [BalenaEtcher](https://etcher.balena.io/).*

---

## 🛠️ Kernel Source Map

```
beaversoft-kernel/
├── boot/                  # UEFI Bootloader (BOOTX64.EFI)
├── docs/                  # Technical specifications, guides & release notes
├── assets/                # Logos, emblems, banners & splash graphics
├── kernel/                # Beaver OS Core Kernel
│   └── src/
│       ├── arch/          # x86_64 GDT, IDT, TSS, CR0/CR3/CR4, MSR IA32_LSTAR
│       ├── memory/        # PML4 Paging, Physical Frame Allocator, Kernel Heap
│       ├── task/          # SMP Round-Robin Scheduler, Context Switch, Locks
│       ├── interrupts/    # Local APIC, IO-APIC, Timer & Hardware Exception Handlers
│       ├── usermode.rs    # Ring 3 Entry (SYSRET/IRETQ), Process Contexts
│       ├── syscall.rs     # Fast Syscall Dispatcher (I/O, IPC, Process control)
│       ├── elf.rs         # ELF64 User Binary Loader & Relocator
│       ├── gui/           # GOP TrueColor Double-Buffered Window Compositor
│       ├── fs/            # BeaverFS Journaling Filesystem & Driver Abstraction
│       ├── drivers/       # PS/2 Keyboard/Mouse, Serial COM1, AHCI/SATA, Ethernet
│       ├── crypto/        # Hashing and cryptographic primitives
│       └── apic.rs        # Multiprocessor Discovery & IPI Dispatch
└── user/                  # Userland Applications & Libraries
    ├── beaver-libc/       # Zero-dependency C-compatible standard library
    ├── shell/             # Interactive Beaver Shell (POSIX-like environment)
    ├── bench/             # Standard Arithmetic Benchmark Suite (ELF64)
    └── hello/             # Minimal Ring 3 User Process Demonstration
```

---

## 💻 Hardware Compatibility

Beaver OS is validated on commercial x86_64 silicon:

* **Intel:** Core i3 / i5 / i7 / i9 (Generations 2nd to 14th: Sandy Bridge through Raptor Lake), Xeon Scalable, Pentium & Celeron x86_64.
* **AMD:** Ryzen 3 / 5 / 7 / 9 (Series 1000 through 7000: Zen 1 through Zen 5), Threadripper, EPYC, and Athlon 64.
* **Firmware Requirements:** 64-bit UEFI with **Secure Boot disabled** and SATA controller configured in **AHCI** mode.

For motherboard boot hotkeys and detailed firmware configuration, see the [Hardware Compatibility Guide](docs/HARDWARE_COMPATIBILITY.md).

---

## 🤝 Contributing

We welcome community feedback, code audits, bug reports, and architectural discussions:
* Review [docs/QUICKSTART.md](docs/QUICKSTART.md) for local environment setup.
* Check [docs/RELEASE_NOTES_v0.9.0.md](docs/RELEASE_NOTES_v0.9.0.md) for current roadmap milestones.
* Open an issue or submit a pull request on [GitHub](https://github.com/japuentem/beaversoft-kernel).

---

## 📜 License & Trademark Policy

* **Project:** Beaversoft Meta-Kernel
* **Trademark:** Beaversoft™
* **Official Product:** Beaver OS™
* **Author & Rights Holder:** José Alberto Puente Maldonado ([@japuentem](https://github.com/japuentem))
* **License Terms ([`LICENSE`](LICENSE)):** Publicly available source code (*Source-Available*) for inspection, study, compilation, evaluation, and technical auditing.

> [!IMPORTANT]
> **Strict Trademark & Naming Protection:**
> The names **BEAVER OS**, **beaver-kernel**, **BeaverFS**, **BeaverScript**, and the trademark **Beaversoft** are the exclusive property of José Alberto Puente Maldonado.
> **Distributing modified versions, forks, or redistribution under the name "BEAVER OS" or "Beaversoft" is strictly prohibited.**
> Any individual or entity creating a fork or derivative work based on this codebase is **legally required to completely rename the project** under a distinct, non-affiliated identity. Refer to [`LICENSE`](LICENSE) for full legal terms.
