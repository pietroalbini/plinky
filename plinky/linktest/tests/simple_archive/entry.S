.intel_syntax noprefix
.file "entry.S"

.global _start

.section .text
_start:
    /* write(1, hello, hello_len) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET hello
    mov edx, OFFSET hello_len
    int 0x80

    /* exit(0) */
    mov al, 1
    mov ebx, 0
    int 0x80
