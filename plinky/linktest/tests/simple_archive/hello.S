global hello
global hello_len

section .data
    hello: db "Hello world!",0x0A
    hello_len: equ $-hello
