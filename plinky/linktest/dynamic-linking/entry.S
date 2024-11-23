.intel_syntax noprefix
.file "entry.S"

.global _start

.section .text
_start:
    call impl@plt
