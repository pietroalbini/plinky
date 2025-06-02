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

.section .rodata.retained,"aR"
    .asciz "I want to be retained"

.section .rodata.retained_same_name,"a",%progbits
    .asciz "I also happen to be retained because I have the same name"

.section .rodata.retained_same_name,"aR",%progbits
    .asciz "I really want to be retained"

.section .comment,"MS",@progbits,1
    .asciz "Sample comment"
