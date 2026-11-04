// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver OS C Library Implementation (beaver-libc)
// ============================================================================

#include "../include/beaver/unistd.h"
#include "../include/beaver/stdio.h"

int64_t read(int fd, void *buf, size_t count) {
    return beaver_syscall3(SYS_read, fd, (int64_t)buf, count);
}

int64_t write(int fd, const void *buf, size_t count) {
    return beaver_syscall3(SYS_write, fd, (int64_t)buf, count);
}

int open(const char *pathname, int flags) {
    return (int)beaver_syscall2(SYS_open, (int64_t)pathname, flags);
}

int close(int fd) {
    return (int)beaver_syscall1(SYS_close, fd);
}

int64_t lseek(int fd, int64_t offset, int whence) {
    return beaver_syscall3(SYS_lseek, fd, offset, whence);
}

int64_t getpid(void) {
    return beaver_syscall0(SYS_getpid);
}

int64_t uptime(void) {
    return beaver_syscall0(SYS_uptime);
}

void sleep(uint64_t ticks) {
    beaver_syscall1(SYS_sleep, ticks);
}

void exit(int status) {
    beaver_syscall1(SYS_exit, status);
    for (;;) {}
}

void sched_yield(void) {
    beaver_syscall0(SYS_yield);
}

int putchar(int c) {
    char ch = (char)c;
    if (write(STDOUT_FILENO, &ch, 1) == 1) {
        return c;
    }
    return -1;
}

int puts(const char *s) {
    size_t len = 0;
    while (s[len]) len++;
    write(STDOUT_FILENO, s, len);
    putchar('\n');
    return 0;
}

int getchar(void) {
    char ch = 0;
    if (read(STDIN_FILENO, &ch, 1) == 1) {
        return (int)ch;
    }
    return -1;
}

