global write
global exit

section .text
write:
    push eax
    push ebx
    push ecx

    mov eax, 4   ; Syscall number (4 = write)
    mov ebx, edi ; First argument (fd)
    mov ecx, esi ; Second argument (string pointer)
  ; mov rdx, rdx ; Third argument (len)
    int 0x80

    pop ecx
    pop ebx
    pop eax
    ret

exit:
    mov eax, 1   ; Syscall number (1 = exit)
    mov ebx, edi ; First argument (code)
    int 0x80
