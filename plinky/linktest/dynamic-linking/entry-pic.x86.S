.intel_syntax noprefix
.file "entry-pic.x86.S"

.global _start
.global main

.section .text
_start:
    call __x86.get_pc_thunk.bx
    add ebx, OFFSET _GLOBAL_OFFSET_TABLE_

    jmp main@PLT

.section .text.main
main:
    /* Note that the push order is inverted */
    mov eax, [message_len@GOT + ebx]
    push [eax]
    push [message@GOT + ebx]
    push 1 /* File descriptor */
    call write@PLT

    mov eax, [exit_code@GOT + ebx]
    push [eax]
    call exit@PLT

/* Move the instruction pointer to ebx */
.section .text.__x86.get_pc_thunk.bx,"axG",@progbits,__x86.get_pc_thunk.bx,comdat
__x86.get_pc_thunk.bx:
    mov ebx,[esp]
    ret
