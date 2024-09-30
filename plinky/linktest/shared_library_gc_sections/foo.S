.intel_syntax noprefix
.file "foo.S"

.global foo
.global bar

.section .text.foo
foo:
    nop

.section .text.bar
bar:
    nop

.section .text.baz
baz:
    nop
