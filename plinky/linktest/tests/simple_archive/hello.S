.intel_syntax noprefix
.file "hello.S"

.global hello
.global hello_len

.section .data
    hello:
    .ascii "Hello world!\n"
    .equ hello_len, $ - hello
