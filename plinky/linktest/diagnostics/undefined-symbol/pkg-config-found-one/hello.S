.intel_syntax noprefix
.file "hello.S"

.global _start

.section .text
_start:
    call libexample_init
