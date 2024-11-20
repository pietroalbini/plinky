.intel_syntax noprefix
.file "test.S"

#include "shared.S"

.global _start

.section .text
_start:
    nop

GNU_PROPERTY_UINT32 GNU_PROPERTY_X86_FEATURES_2_USED, 0b1
GNU_PROPERTY_UINT32 GNU_PROPERTY_X86_FEATURES_2_USED, 0b01
