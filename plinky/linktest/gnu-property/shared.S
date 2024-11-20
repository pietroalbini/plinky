/* Elf constants */
#define NT_GNU_PROPERTY_TYPE_0 5
#define GNU_PROPERTY_X86_FEATURES_2_USED 0xc0010001
#define GNU_PROPERTY_X86_ISA_1_USED 0xc0010002

#if(__x86_64__)
#define _PROPERTY_ALIGN 8
#else
#define _PROPERTY_ALIGN 4
#endif

.macro GNU_PROPERTY_UINT32 type, value
    .section .note.gnu.property,"a",@note
        /* SHT_NOTE header */
        .long note_name_end_\@ - note_name_start_\@
        .long note_value_end_\@ - note_value_start_\@
        .long NT_GNU_PROPERTY_TYPE_0

        /* SHT_NOTE name */
        note_name_start_\@:
        .asciz "GNU"
        note_name_end_\@:
        .balign 4

        /* SHT_NOTE value */
        note_value_start_\@:
            /* GNU property header */
            .long \type
            .long gnu_property_value_end_\@ - gnu_property_value_start_\@

            /* GNU property value */
            gnu_property_value_start_\@:
            .long \value
            gnu_property_value_end_\@:

            /* Padding (the amount depends on the arch) */
            .balign _PROPERTY_ALIGN
        note_value_end_\@:
        .balign 4
.endm

