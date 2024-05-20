#include <stddef.h>

// Defined in assembly files.
void exit(int code);
int write(int fd, char* str, size_t len);

void _start() {
    write(1, "Hello world\n", 12);
    exit(0);
}
