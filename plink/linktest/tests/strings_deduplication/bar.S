global bar

section .custom_messages strings byte merge
    goodbye: db "Goodbye",0
    goodbye_len: equ $-goodbye-1
    ; The end message here should get deduplicated.
    bar_end: db " world!",0x0a,0
    bar_end_len: equ $-bar_end-1

section .text
bar:
    ; write(1, goodbye, goodbye_len)
    mov eax, 4
    mov ebx, 1
    mov ecx, goodbye
    mov edx, goodbye_len
    int 0x80

    ; write(1, bar_end, bar_end_len)
    mov eax, 4
    mov ebx, 1
    mov ecx, bar_end
    mov edx, bar_end_len
    int 0x80

    ; exit(0)
    mov eax, 1
    mov ebx, 0
    int 0x80
