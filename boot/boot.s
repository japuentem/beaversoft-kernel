/* ============================================================================
 * Project: Beaversoft Meta-Kernel
 * Author: José Alberto Puente Maldonado (japuentem)
 * Brand: Beaversoft
 * Description: Stage 1 Bootstrapping - Multiboot 1/2 to 64-bit Long Mode Trampoline
 * ============================================================================ */

/* --- Constantes Multiboot 1 --- */

.set MB1_MAGIC,        0x1BADB002
.set MB1_FLAGS,        0x00010003 /* ALIGN (bit 0) | MEMINFO (bit 1) | AOUT_KLUDGE (bit 16) */
.set MB1_CHECKSUM,     -(MB1_MAGIC + MB1_FLAGS)

/* --- Constantes Multiboot 2 --- */
.set MB2_MAGIC,        0xe85250d6
.set MB2_ARCH_I386,    0

.section .multiboot_header, "a"
.align 8

/* Cabecera Multiboot 1 (compatible nativamente con QEMU -kernel) */
mb1_start:
    .long MB1_MAGIC
    .long MB1_FLAGS
    .long MB1_CHECKSUM
    .long mb1_start
    .long _kernel_start
    .long _kernel_data_end
    .long _kernel_end
    .long _start
mb1_end:

.align 8
/* Cabecera Multiboot 2 (compatible con GRUB2 multiboot2) */
mb2_start:
    .long MB2_MAGIC
    .long MB2_ARCH_I386
    .long (mb2_end - mb2_start)
    .long -(MB2_MAGIC + MB2_ARCH_I386 + (mb2_end - mb2_start))
    .short 0
    .short 0
    .long 8
mb2_end:

.align 8
.global beaver_uefi_header
beaver_uefi_header:
    .ascii "BEAVUEFI"
    .quad uefi_entry

.section .text
.code32
.global _start
.type _start, @function
_start:
    cli

    /* 1. Configurar pila temporal de 32 bits */
    mov esp, offset stack_top

    /* 1b. Preservar magic de Multiboot (eax) e info pointer (ebx) ANTES de que rep stosd destruya edi/eax */
    mov [mb_magic], eax
    mov [mb_info],  ebx

    /* 2. Configurar tablas de páginas para Modo Largo (Identity Mapping de los primeros 4 GB) */
    /* Limpiar tablas de páginas (PML4, PDPT, 4*PD = 6 * 4096 bytes = 6144 dwords) */
    mov edi, offset pml4_table
    xor eax, eax
    mov ecx, 6144
    rep stosd

    /* PML4[0] -> PDPT (Presente 0x01 | Lectura/Escritura 0x02) */
    mov edi, offset pml4_table
    mov eax, offset pdpt_table
    or eax, 0x03
    mov [edi], eax

    /* PDPT[0..3] -> 4 tablas PD de 1GB cada una (Total: 4 GB) */
    mov edi, offset pdpt_table
    mov eax, offset pd_tables
    or eax, 0x03
    mov [edi], eax
    add eax, 4096
    mov [edi + 8], eax
    add eax, 4096
    mov [edi + 16], eax
    add eax, 4096
    mov [edi + 24], eax

    /* Mapear 2048 páginas enormes de 2MB (4 tablas * 512 entradas = 4096 MB = 4 GB) */
    mov edi, offset pd_tables
    mov ebx, 0x83 /* Presente 0x01 | RW 0x02 | Huge Page 2MB 0x80 */
    mov ecx, 2048
1:
    mov [edi], ebx
    add ebx, 0x200000 /* Siguiente página física de 2MB */
    add edi, 8        /* Entrada de 64 bits (8 bytes) */
    loop 1b

    /* 3. Habilitar PAE (Physical Address Extension) en CR4 (Bit 5) */
    mov eax, cr4
    or eax, (1 << 5)
    mov cr4, eax

    /* 4. Cargar dirección base de PML4 en CR3 */
    mov eax, offset pml4_table
    mov cr3, eax

    /* 5. Habilitar Long Mode en el MSR IA32_EFER (0xC0000080, Bit 8 = LME) */
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8)
    wrmsr

    /* 6. Habilitar Paginación y Protección en CR0 (Bit 31 = PG, Bit 0 = PE) */
    mov eax, cr0
    or eax, 0x80000001
    mov cr0, eax

    /* 7. Cargar GDT de 64 bits */
    lgdt [gdt64_ptr]

    /* 8. Salto largo (far return) al segmento de código de 64 bits (Selector 0x08) */
    push 0x08
    mov eax, offset long_mode_start
    push eax
    retf



.code64
long_mode_start:
    /* Cargar selectores de datos de 64 bits (Selector 0x10) */
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    /* Configurar puntero de pila de 64 bits */
    mov rsp, offset stack_top

    /* Invocar el punto de entrada principal del kernel Beaver en Rust */
    /* Pasar magic (rdi) e info-pointer (rsi) como argumentos System V AMD64 ABI */
    mov edi, [mb_magic]
    mov esi, [mb_info]
    call kernel_main

    /* Si kernel_main retorna, detener la CPU en reposo */
    cli
halt_loop:
    hlt
    jmp halt_loop

.global uefi_entry
.type uefi_entry, @function
uefi_entry:
    cli
    lgdt [gdt64_ptr64]
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov rsp, offset stack_top

    /* Configurar tablas de paginas de 64 bits propias del kernel (Identity Mapping de 4 GB) */
    mov rdi, offset pml4_table
    xor eax, eax
    mov ecx, 6144
    rep stosd

    mov rdi, offset pml4_table
    mov rax, offset pdpt_table
    or rax, 0x03
    mov [rdi], rax

    mov rdi, offset pdpt_table
    mov rax, offset pd_tables
    or rax, 0x03
    mov [rdi], rax
    add rax, 4096
    mov [rdi + 8], rax
    add rax, 4096
    mov [rdi + 16], rax
    add rax, 4096
    mov [rdi + 24], rax

    mov rdi, offset pd_tables
    mov rbx, 0x83 /* Presente 0x01 | RW 0x02 | Huge Page 2MB 0x80 */
    mov ecx, 2048
2:
    mov [rdi], rbx
    add rbx, 0x200000
    add rdi, 8
    loop 2b

    mov rax, offset pml4_table
    mov cr3, rax

    /* Pasar UEFI boot magic (rdi) y direccion de UefiBootInfo en 0x9000 (rsi) */
    mov edi, 0xBEA0B007
    mov esi, 0x9000
    call kernel_main
    cli
uefi_halt:
    hlt
    jmp uefi_halt

.section .rodata
.align 8
gdt64:    .quad 0x0000000000000000 /* 0x00: Descriptor Nulo */
    .quad 0x00209a0000000000 /* 0x08: Descriptor de Código 64-bit (Ring 0, Exec/Read, L=1) */
    .quad 0x0000920000000000 /* 0x10: Descriptor de Datos 64-bit (Ring 0, Read/Write) */
gdt64_end:

gdt64_ptr:
    .short gdt64_end - gdt64 - 1
    .long gdt64

gdt64_ptr64:
    .short gdt64_end - gdt64 - 1
    .quad gdt64


.section .bss
.align 4096
pml4_table:
    .skip 4096
pdpt_table:
    .skip 4096
pd_tables:
    .skip 16384 /* 4 tablas PD para cubrir 4 GB (4 x 512 entradas de 2MB) */

.align 16
stack_bottom:
    .skip 16384 /* Pila de arranque de 16 KB */
stack_top:

/* Variables para preservar magic e info pointer de Multiboot antes de rep stosd */
.section .data
.align 4
.global mb_magic
mb_magic:
    .long 0
.global mb_info
mb_info:
    .long 0
