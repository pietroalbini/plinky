.intel_syntax noprefix
.file "foo.S"

.global _start

.section .bad,"aSM",@progbits,1
    _start:
    .asciz "Hello world"
