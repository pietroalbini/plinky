.intel_syntax noprefix
.file "data.S"

.global exit_code
.global message
.global message_len

.section .rodata
    exit_code:
    .long 0
    message:
    .ascii "Hello world!\n"
    message_len:
    .long $ - message
