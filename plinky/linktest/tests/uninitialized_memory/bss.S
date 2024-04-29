.intel_syntax noprefix
.file "bss.S"

.global _start

.section .bss
    .lcomm hello,6

.section .text
_start:
    movb [hello], 'h'
    movb [hello + 1], 'e'
    movb [hello + 2], 'l'
    movb [hello + 3], 'l'
    movb [hello + 4], 'o'
    movb [hello + 5], 0x0A

    /* write syscall */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET hello
    mov edx, 6
    int 0x80

    /* exit syscall */
    mov eax, 1
    mov ebx, 0
    int 0x80
