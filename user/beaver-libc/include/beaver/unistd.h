// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver OS Standard Symbolic Constants and Types (POSIX unistd.h)
// ============================================================================

#ifndef _BEAVER_UNISTD_H
#define _BEAVER_UNISTD_H

#include "syscall.h"
#include <stddef.h>

#define STDIN_FILENO  0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

int64_t read(int fd, void *buf, size_t count);
int64_t write(int fd, const void *buf, size_t count);
int open(const char *pathname, int flags);
int close(int fd);
int64_t lseek(int fd, int64_t offset, int whence);
int64_t getpid(void);
int64_t uptime(void);
void sleep(uint64_t ticks);
void exit(int status) __attribute__((noreturn));
void sched_yield(void);

#endif // _BEAVER_UNISTD_H

