.intel_syntax noprefix
.file "impl.x86_64.S"

.global impl

.section .text
impl:
    /* Exit */
    mov rax, 60
    mov rbx, 0
    syscall
