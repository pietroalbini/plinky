.intel_syntax noprefix
.file "syscalls.write.x86.S"

.global write

.section .text
write:
    push eax
    push ebx
    push ecx

    mov eax, 4          /* Syscall number (4 = write) */
    mov ebx, [esp + 16] /* First argument (fd) */
    mov ecx, [esp + 20] /* Second argument (string pointer) */
    mov edx, [esp + 24] /* Third argument (len) */
    int 0x80

    pop ecx
    pop ebx
    pop eax
    ret
