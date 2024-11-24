.intel_syntax noprefix
.file "syscalls.exit.x86.S"

.global exit

.section .text
exit:
    mov eax, 1         /* Syscall number (1 = exit) */
    mov ebx, [esp + 4] /* First argument (code) */
    int 0x80
