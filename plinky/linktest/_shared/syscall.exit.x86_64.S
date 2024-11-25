.intel_syntax noprefix
.file "syscalls.exit.x86_64.S"

.global exit

.section .text
exit:
    /* Registers are not saved since, well, we exit */

    mov rax, 60  /* Syscall number (60 = exit) */
 /* mov rdi, rdi    First argument (code) */
    syscall
