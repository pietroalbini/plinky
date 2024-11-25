.intel_syntax noprefix
.file "syscalls.write.x86_64.S"

.global write

.section .text
write:
    push rax
    push rcx /* Register clobbered by the kernel */
    push r11 /* Register clobbered by the kernel */

    mov rax, 1   /* Syscall number (1 = write) */
 /* mov rdi, rdi    First argument (fd) */
 /* mov rsi, rsi    Second argument (string pointer) */
 /* mov rdx, rdx    Third argument (len) */
    syscall

    pop r11
    pop rcx
    pop rax
    ret
