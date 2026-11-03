# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Description: Automated build and QEMU execution Makefile
# ============================================================================

TARGET ?= x86_64-unknown-none
CARGO_TARGET_DIR ?= target
KERNEL_BIN = $(CARGO_TARGET_DIR)/$(TARGET)/release/beaversoft-kernel

.PHONY: all clean run run-nographic build

all: run

build:
	cargo build --release --target $(TARGET)

run: build
	qemu-system-x86_64 -kernel $(KERNEL_BIN) -serial stdio

run-nographic: build
	qemu-system-x86_64 -kernel $(KERNEL_BIN) -serial stdio -display none

clean:
	cargo clean