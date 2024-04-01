global _start

section .text
_start:
    jmp sample

section .text.foo
sample:
    mov eax, name

section .text.bar
excluded:
    mov eax, surname

section .rodata.name
name:
    db "Pietro",0

section .rodata.surname
surname:
    db "Albini",0

section .comment merge strings noalloc noexec nowrite
    db "Sample comment",0
