.intel_syntax noprefix
.file "hello.S"

.global _start

.section .rodata
    msg:
    .ascii "Hello world!\n"
    .equ len, $ - msg

.section .text
_start:
    /* write(1, "Hello world\n", $len) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET msg
    mov edx, len
    int 0x80

    jmp goodbye
