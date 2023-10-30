global _start

section .data
    msg: db "Hello world!",0x0A
    len: equ $-msg

section .text
_start:
    ; write(1, "Hello world", $len)
    mov eax, 4
    mov ebx, 1
    mov ecx, msg
    mov edx, len
    int 0x80

    ; exit(0)
    mov al, 1
    mov ebx, 0
    int 0x80
