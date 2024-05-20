.intel_syntax noprefix
.file "test.S"

.global _start

.section .comment,"MS",@progbits,1
    .asciz "Hello world"

.section .text
_start:
    nop
