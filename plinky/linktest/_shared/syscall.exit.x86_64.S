.intel_syntax noprefix
.file "syscalls.exit.x86_64.S"

.global exit

.section .text
exit:
    mov rax, 60  /* Syscall number (60 = exit) */
    mov rbx, rdi /* First argument (code) */
    syscall
