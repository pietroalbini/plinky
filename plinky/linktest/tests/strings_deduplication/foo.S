.intel_syntax noprefix
.file "foo.S"

.global _start

.section .custom_messages,"aMS",@progbits,1
    hello: .asciz "Hello"
    .equ hello_len, $ - hello - 1

    /* The end message here should get deduplicated. */
    foo_end: .asciz " world!\n"
    .equ foo_end_len, $ - foo_end - 1

.section .text
_start:
    /* write(1, hello, hello_len) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET hello
    mov edx, hello_len
    int 0x80

    /* write(1, end, end_len) */
    mov eax, 4
    mov ebx, 1
    mov ecx, OFFSET foo_end
    mov edx, foo_end_len
    int 0x80

    jmp bar
