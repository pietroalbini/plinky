global _start

section .bss
    hello: resb 6
    hello_len: equ $-hello

section .text
_start:
    mov byte [hello], 'h'
    mov byte [hello + 1], 'e'
    mov byte [hello + 2], 'l'
    mov byte [hello + 3], 'l'
    mov byte [hello + 4], 'o'
    mov byte [hello + 5], 0x0A

    ; write syscall
    mov eax, 4
    mov ebx, 1
    mov ecx, hello
    mov edx, hello_len
    int 0x80

    ; exit syscall
    mov eax, 1
    mov ebx, 0
    int 0x80
