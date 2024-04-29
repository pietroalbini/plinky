.intel_syntax noprefix
.file "hello.S"

.global custom_entry

.section .data
    msg:
    .ascii "Hello world!\n"
    .equ len, $ - msg

.section .text
custom_entry:
    /* write(1, "Hello world\n", $len) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET msg
    mov edx, len
    int 0x80

    /* exit(0) */
    mov al, 1
    mov ebx, 0
    int 0x80

