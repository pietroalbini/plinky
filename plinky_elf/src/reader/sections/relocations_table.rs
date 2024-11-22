use crate::errors::LoadError;
use crate::ids::ElfSymbolId;
use crate::raw::{RawRel, RawRela};
use crate::reader::sections::reader::{SectionMetadata, SectionReader};
use crate::{ElfClass, ElfRel, ElfRelTable, ElfRela, ElfRelaTable, ElfRelocationType};

pub(super) fn read_rel(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfRelTable, LoadError> {
    let mut relocations = Vec::new();
    for mut cursor in reader.entries()? {
        let raw: RawRel = cursor.read_raw()?;
        let (symbol, relocation_type) = symbol_and_relocation_type(reader, meta, raw.info);
        relocations.push(ElfRel { offset: raw.offset, symbol, relocation_type });
    }

    Ok(ElfRelTable {
        symbol_table: meta.section_link(),
        applies_to_section: meta.section_info(),
        relocations,
    })
}

pub(super) fn read_rela(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfRelaTable, LoadError> {
    let mut relocations = Vec::new();
    for mut cursor in reader.entries()? {
        let raw: RawRela = cursor.read_raw()?;
        let (symbol, relocation_type) = symbol_and_relocation_type(reader, meta, raw.info);
        relocations.push(ElfRela {
            offset: raw.offset,
            symbol,
            relocation_type,
            addend: raw.addend,
        });
    }

    Ok(ElfRelaTable {
        symbol_table: meta.section_link(),
        applies_to_section: meta.section_info(),
        relocations,
    })
}

fn symbol_and_relocation_type(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
    info: u64,
) -> (ElfSymbolId, ElfRelocationType) {
    let (index, type_) = match reader.parent_cursor.class {
        ElfClass::Elf32 => (
            (info >> 8) as u32,
            match info & 0xF {
                0 => ElfRelocationType::X86_None,
                1 => ElfRelocationType::X86_32,
                2 => ElfRelocationType::X86_PC32,
                3 => ElfRelocationType::X86_GOT32,
                4 => ElfRelocationType::X86_PLT32,
                5 => ElfRelocationType::X86_COPY,
                6 => ElfRelocationType::X86_GlobDat,
                7 => ElfRelocationType::X86_JumpSlot,
                8 => ElfRelocationType::X86_Relative,
                9 => ElfRelocationType::X86_GOTOff,
                10 => ElfRelocationType::X86_GOTPC,
                11 => ElfRelocationType::X86_GOT32X,
                other => ElfRelocationType::Unknown(other as _),
            },
        ),
        ElfClass::Elf64 => (
            (info >> 32) as u32,
            match info & 0xFFFF_FFFF {
                0 => ElfRelocationType::X86_64_None,
                1 => ElfRelocationType::X86_64_64,
                2 => ElfRelocationType::X86_64_PC32,
                3 => ElfRelocationType::X86_64_GOT32,
                4 => ElfRelocationType::X86_64_PLT32,
                5 => ElfRelocationType::X86_64_Copy,
                6 => ElfRelocationType::X86_64_GlobDat,
                7 => ElfRelocationType::X86_64_JumpSlot,
                8 => ElfRelocationType::X86_64_Relative,
                9 => ElfRelocationType::X86_64_GOTPCRel,
                10 => ElfRelocationType::X86_64_32,
                11 => ElfRelocationType::X86_64_32S,
                12 => ElfRelocationType::X86_64_16,
                13 => ElfRelocationType::X86_64_PC16,
                14 => ElfRelocationType::X86_64_8,
                15 => ElfRelocationType::X86_64_PC8,
                16 => ElfRelocationType::X86_64_DTPMod64,
                17 => ElfRelocationType::X86_64_DTPOff64,
                18 => ElfRelocationType::X86_64_TPOff64,
                19 => ElfRelocationType::X86_64_TLSGD,
                20 => ElfRelocationType::X86_64_TLSLD,
                21 => ElfRelocationType::X86_64_DTPOff32,
                22 => ElfRelocationType::X86_64_GOTTPOff,
                23 => ElfRelocationType::X86_64_TPOff32,
                24 => ElfRelocationType::X86_64_PC64,
                25 => ElfRelocationType::X86_64_GOTOff64,
                26 => ElfRelocationType::X86_64_GOTPC32,
                32 => ElfRelocationType::X86_64_Size32,
                33 => ElfRelocationType::X86_64_Size64,
                34 => ElfRelocationType::X86_64_GOTPC32_TLSDesc,
                35 => ElfRelocationType::X86_64_TLSDescCall,
                36 => ElfRelocationType::X86_64_TLSDesc,
                37 => ElfRelocationType::X86_64_IRelative,
                38 => ElfRelocationType::X86_64_IRelative64,
                41 => ElfRelocationType::X86_64_GOTPCRelX,
                42 => ElfRelocationType::X86_64_Rex_GOTPCRelX,
                43 => ElfRelocationType::X86_64_Code_4_GOTPCRelX,
                44 => ElfRelocationType::X86_64_Code_4_GOTPCOff,
                45 => ElfRelocationType::X86_64_Code_4_GOTPC32_TLSDesc,
                46 => ElfRelocationType::X86_64_Code_5_GOTPCRelX,
                47 => ElfRelocationType::X86_64_Code_5_GOTPCOff,
                48 => ElfRelocationType::X86_64_Code_5_GOTPC32_TLSDesc,
                49 => ElfRelocationType::X86_64_Code_6_GOTPCRelX,
                50 => ElfRelocationType::X86_64_Code_6_GOTPCOff,
                51 => ElfRelocationType::X86_64_Code_6_GOTPC32_TLSDesc,
                other => ElfRelocationType::Unknown(other as _),
            },
        ),
    };
    (ElfSymbolId { section: meta.section_link(), index }, type_)
}
