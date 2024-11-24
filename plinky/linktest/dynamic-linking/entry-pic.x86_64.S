.intel_syntax noprefix
.file "entry-pic.x86_64.S"

.global _start

.section .text
_start:
    mov rdi, 1 /* File descriptor */
    mov rsi, [message@GOTPCREL + rip]
    mov rax, [message_len@GOTPCREL + rip]
    mov edx, [rax]
    call write@PLT

    mov rax, [exit_code@GOTPCREL + rip]
    mov rdi, [rax]
    call exit@PLT
