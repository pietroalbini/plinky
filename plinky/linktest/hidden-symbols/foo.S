.intel_syntax noprefix
.file "foo.S"

.global foo
.hidden foo

.global _start

.section .text
foo:
    nop
_start:
    nop
