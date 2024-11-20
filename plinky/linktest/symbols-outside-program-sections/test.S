.intel_syntax noprefix
.file "test.S"

.global _start

.section .text
_start:
    nop

.section .strtab.custom,"a",@3  /* 3 is SHT_STRTAB */
    hello:
    .asciz "Hello"
    world:
    .asciz "World"
