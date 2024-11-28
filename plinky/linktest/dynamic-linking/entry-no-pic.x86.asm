global _start

extern message
extern message_len
extern exit_code
extern write
extern exit

section .text
_start:
    ; Note that the push order is inverted
    push DWORD [message_len]
    push message
    push 1 ; File descriptor
    call write

    push DWORD [exit_code]
    call exit
