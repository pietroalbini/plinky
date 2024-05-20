.intel_syntax noprefix
.file "foo.S"

.global _start
.global sample_symbol
.hidden sample_symbol

.section .text.sample_group,"axG",@progbits,sample_group,comdat
sample_symbol:
    nop
    nop
    nop

.section .text
_start:
    nop
