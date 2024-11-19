.intel_syntax noprefix
.file "foo.S"

.global _start

.section .text,"ax"
_start:
    jmp sample

.section .text.foo,"ax"
sample:
    mov eax, name

.section .text.bar,"ax"
excluded:
    mov eax, surname

.section .rodata.name,"a"
name:
    .asciz "Pietro"

.section .rodata.surname,"a"
surname:
    .asciz "Albini"

.section .comment,"MS",@progbits,1
    .asciz "Sample comment"
