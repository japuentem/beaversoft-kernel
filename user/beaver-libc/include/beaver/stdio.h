// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Beaver OS Standard Input / Output Library (POSIX stdio.h)
// ============================================================================

#ifndef _BEAVER_STDIO_H
#define _BEAVER_STDIO_H

#include "unistd.h"

int puts(const char *s);
int putchar(int c);
int getchar(void);

#endif // _BEAVER_STDIO_H

