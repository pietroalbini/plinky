; For this test, `nasm` is used instead of `as` because `as` stubbornly (and correctly) emits PLT
; relocations when a symbol we call is defined externally, since it could be defined in a shared
; object. In this test though we explicitly want to test how plinky behaves with non-PIC relocations
; pointing to shared objects, and `nasm` lets us emit those relocations.

global _start

extern message
extern message_len
extern exit_code
extern write
extern exit

section .text
_start:
    mov rdi, 1 ; File descriptor
    mov rsi, message
    mov edx, [message_len]
    call write

    mov rdi, [exit_code]
    call exit
