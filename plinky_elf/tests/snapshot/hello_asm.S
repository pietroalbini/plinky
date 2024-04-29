.intel_syntax noprefix
.file "hello_asm.S"

.global _start

.section .data
    msg_hello:
    .ascii "Hello world!\n"
    .equ len_hello, $ - msg_hello

    msg_goodbye:
    .ascii "Goodbye world!\n"
    .equ len_goodbye, $ - msg_goodbye

.section .bss
    .lcomm uninit,8

.section .text
_start:
    /* write(1, "Hello world\n", $len_goodbye) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET msg_hello
    mov edx, len_hello
    int 0x80

    /* write(1, "Goodbye world\n", $len_goodbye) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET msg_goodbye
    mov edx, len_goodbye
    int 0x80

    /* exit(0) */
    mov al, 1
    mov ebx, 0
    int 0x80
