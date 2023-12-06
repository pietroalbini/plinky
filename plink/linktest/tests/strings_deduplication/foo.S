global _start
extern bar

section .custom_messages strings byte merge
    hello: db "Hello",0
    hello_len: equ $-hello-1
    ; The end message here should get deduplicated.
    foo_end: db " world!",0x0a,0
    foo_end_len: equ $-foo_end-1

section .text
_start:
    ; write(1, hello, hello_len)
    mov eax, 4
    mov ebx, 1
    mov ecx, hello
    mov edx, hello_len
    int 0x80

    ; write(1, end, end_len)
    mov eax, 4
    mov ebx, 1
    mov ecx, foo_end
    mov edx, foo_end_len
    int 0x80

    jmp bar
