.intel_syntax noprefix
.file "groups.S"

.section .rodata.group1,"aG",@progbits,first_group
    .ascii "First group"

.section .rodata.group2_1,"aG",@progbits,second_group,comdat
    .ascii "First part of second group"

.section .rodata.group2_2,"aG",@progbits,second_group,comdat
    .ascii "Second part of second group"

.section .rodata.group3,"aG",@progbits,third_group
    .ascii "Third group"
