.intel_syntax noprefix
.file "goodbye.S"

.global goodbye

.section .rodata
    msg:
    .ascii "Goodbye world!\n"
    .equ len, $ - msg

.section .text
goodbye:
    /* write(1, "Goodbye world\n", $len) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET msg
    mov edx, len
    int 0x80

    /* exit(0) */
    mov al, 1
    mov ebx, 0
    int 0x80
