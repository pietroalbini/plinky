.intel_syntax noprefix
.file "syscalls.write.x86_64.S"

.global write

.section .text
write:
    push rax
    push rbx
    push rcx

    mov rax, 1   /* Syscall number (1 = write) */
    mov rbx, rdi /* First argument (fd) */
    mov rcx, rsi /* Second argument (string pointer) */
 /* mov rdx, rdx    Third argument (len) */
    syscall

    pop rcx
    pop rbx
    pop rax
    ret
