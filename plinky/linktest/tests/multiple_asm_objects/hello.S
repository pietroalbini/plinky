global _start
extern goodbye

section .rodata
    msg: db "Hello world!",0x0A
    len: equ $-msg

section .text
_start:
    ; write(1, "Hello world\n", $len)
    mov eax, 4
    mov ebx, 1
    mov ecx, msg
    mov edx, len
    int 0x80

    jmp goodbye
