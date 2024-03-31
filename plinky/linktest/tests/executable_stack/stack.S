; To test whether the stack is executable, the _start symbol copies the payload
; into the stack and then jumps to it.

global _start

section .text
_start:
    ; On x86 the stack grows downward, so we need to copy the payload from end
    ; to start (rather than from start to end). Every loop iteration subtracts
    ; 4 from the current pointer, and then pushes the 32bit value to the stack.
    mov ebx,end_payload
    loop:
        sub ebx,4
        mov eax,[ebx]
        push eax

        cmp ebx,start_payload
        jne loop

    ; Finally jump to the stack.
    jmp esp

section .text.payload alloc nowrite noexec
start_payload:
    ; write(1, $msg, $msg_len)
    mov eax, 4
    mov ebx, 1
    mov ecx, msg
    mov edx, msg_len
    int 0x80

    ; exit(0)
    mov eax, 1
    mov ebx, 0
    int 0x80
    align 4
end_payload:

section .rodata
    msg: db "Hello world!",0x0A
    msg_len: equ $-msg
