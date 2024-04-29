.intel_syntax noprefix
.file "goodbye.S"

.global goodbye
.global goodbye_len

.section .data
    goodbye:
    .ascii "Goodbye world!\n"
    .equ goodbye_len, $ - goodbye
