.intel_syntax noprefix
.file "bar.S"

.global bar

.section .custom_messages,"aMS",@progbits,1
    goodbye: .asciz "Goodbye"
    .equ goodbye_len, $ - goodbye - 1

    /* The end message here should get deduplicated. */
    bar_end: .asciz " world!\n"
    .equ bar_end_len, $ - bar_end - 1

.section .text
bar:
    /* write(1, goodbye, goodbye_len) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET goodbye
    mov edx, goodbye_len
    int 0x80

    /* write(1, bar_end, bar_end_len) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET bar_end
    mov edx, bar_end_len
    int 0x80

    /* exit(0) */
    mov eax, 1
    mov ebx, 0
    int 0x80
