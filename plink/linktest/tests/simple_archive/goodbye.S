global goodbye
global goodbye_len

section .data
    goodbye: db "Goodbye world!",0x0A
    goodbye_len: equ $-goodbye
