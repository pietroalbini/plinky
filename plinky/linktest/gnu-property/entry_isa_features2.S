.intel_syntax noprefix
.file "entry_isa_features2.S"

.global _start

.section .text
_start:
    nop

#include "shared.S"
GNU_PROPERTY_UINT32 GNU_PROPERTY_X86_FEATURES_2_USED 0b1
GNU_PROPERTY_UINT32 GNU_PROPERTY_X86_ISA_1_USED 0b1
