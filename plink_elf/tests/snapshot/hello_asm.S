global _start

section .data
    msg_hello: db "Hello world!",0x0A
    len_hello: equ $-msg_hello
    msg_goodbye: db "Goodbye world!",0x0A
    len_goodbye: equ $-msg_goodbye

section .text
_start:
    ; write(1, "Hello world\n", $len_goodbye)
    mov eax, 4
    mov ebx, 1
    mov ecx, msg_hello
    mov edx, len_hello
    int 0x80

    ; write(1, "Goodbye world\n", $len_goodbye)
    mov eax, 4
    mov ebx, 1
    mov ecx, msg_goodbye
    mov edx, len_goodbye
    int 0x80

    ; exit(0)
    mov al, 1
    mov ebx, 0
    int 0x80
