.intel_syntax noprefix
.file "syscalls.32bit.S"

.global write
.global exit

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

exit:
    mov eax, 1         /* Syscall number (1 = exit) */
    mov ebx, [esp + 4] /* First argument (code) */
    int 0x80
