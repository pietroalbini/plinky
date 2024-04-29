.intel_syntax noprefix
.file "syscalls.64bit.S"

.global write
.global exit

.section .text
write:
    push rax
    push rbx
    push rcx

    mov rax, 4   /* Syscall number (4 = write) */
    mov rbx, rdi /* First argument (fd) */
    mov rcx, rsi /* Second argument (string pointer) */
 /* mov rdx, rdx    Third argument (len) */
    int 0x80

    pop rcx
    pop rbx
    pop rax
    ret

exit:
    mov rax, 1   /* Syscall number (1 = exit) */
    mov rbx, rdi /* First argument (code) */
    int 0x80
