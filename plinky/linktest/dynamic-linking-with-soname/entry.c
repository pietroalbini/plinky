#include <stddef.h>

void exit(size_t code);
void write(size_t fd, char* message, size_t message_len);

void _start() {
    write(1, "Hello world\n", 12);
    exit(0);
}
