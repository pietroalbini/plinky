.intel_syntax noprefix
.file "foo.S"

.global foo
.global bar

.section .rodata
foo:
    .asciz "Hello world"

.section .text
bar:
    nop
