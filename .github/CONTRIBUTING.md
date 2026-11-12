# Contributing to Beaver OS

Thank you for your interest in contributing to **Beaver OS**!

Beaver OS is an independent bare-metal operating system built from scratch in Rust (`no_std`) for `x86_64` long mode.

---

## 🛠️ Development Environment

To build and test the kernel locally, you will need:
- **Rust Nightly Toolchain** with `rust-src` and the `x86_64-unknown-none` target:
  ```bash
  rustup toolchain install nightly
  rustup default nightly
  rustup target add x86_64-unknown-none
  rustup component add rust-src
  ```
- **QEMU** (`qemu-system-x86_64`) for running virtual machine tests.
- **PowerShell** (Windows) or `make` for executing build scripts.

---

## 🧪 Testing Your Changes

Before submitting a pull request:
1. Verify that the workspace compiles cleanly without errors:
   ```bash
   cargo check --target x86_64-unknown-none
   ```
2. Test kernel boot in QEMU:
   ```powershell
   powershell -ExecutionPolicy Bypass -File run.ps1
   ```
3. Ensure no regressions in Ring 3 isolation, scheduler routines, or memory allocators.

---

## 📜 Trademark Notice

When submitting contributions, please note that **BEAVER OS**, **beaver-kernel**, and **Beaversoft** are trademarks of José Alberto Puente Maldonado. Any forks or redistributed derivative works must be renamed in accordance with the [`LICENSE`](../LICENSE).
