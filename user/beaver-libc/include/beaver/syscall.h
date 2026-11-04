// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver OS Standard C Syscall Definitions & Macros (x86_64)
// ============================================================================

#ifndef _BEAVER_SYSCALL_H
#define _BEAVER_SYSCALL_H

#include <stdint.h>

// Números de Syscall POSIX estándar soportados por el kernel de Beaver OS
#define SYS_read     0
#define SYS_write    1
#define SYS_open     2
#define SYS_close    3
#define SYS_stat     4
#define SYS_lseek    8
#define SYS_yield   24
#define SYS_getpid  39
#define SYS_execve  59
#define SYS_exit    60
#define SYS_uptime  99
#define SYS_sleep  102

// IPC y sincronización
#define SYS_ipc_send     110
#define SYS_ipc_recv     111
#define SYS_futex_wait   112
#define SYS_futex_wake   113
#define SYS_mutex_lock   114
#define SYS_mutex_unlock 115
#define SYS_sem_wait     116
#define SYS_sem_post     117

// Macros de invocación de syscall x86_64 usando instrucción `syscall`
static inline int64_t beaver_syscall0(int64_t n) {
    int64_t ret;
    __asm__ __volatile__(
        "syscall"
        : "=a"(ret)
        : "a"(n)
        : "rcx", "r11", "memory"
    );
    return ret;
}

static inline int64_t beaver_syscall1(int64_t n, int64_t a1) {
    int64_t ret;
    __asm__ __volatile__(
        "syscall"
        : "=a"(ret)
        : "a"(n), "D"(a1)
        : "rcx", "r11", "memory"
    );
    return ret;
}

static inline int64_t beaver_syscall2(int64_t n, int64_t a1, int64_t a2) {
    int64_t ret;
    __asm__ __volatile__(
        "syscall"
        : "=a"(ret)
        : "a"(n), "D"(a1), "S"(a2)
        : "rcx", "r11", "memory"
    );
    return ret;
}

static inline int64_t beaver_syscall3(int64_t n, int64_t a1, int64_t a2, int64_t a3) {
    int64_t ret;
    __asm__ __volatile__(
        "syscall"
        : "=a"(ret)
        : "a"(n), "D"(a1), "S"(a2), "d"(a3)
        : "rcx", "r11", "memory"
    );
    return ret;
}

#endif // _BEAVER_SYSCALL_H

