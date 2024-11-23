.intel_syntax noprefix
.file "impl.x86.S"

.global impl

.section .text
impl:
    /* Exit */
    mov eax, 1
    mov ebx,  0
    int 0x80
