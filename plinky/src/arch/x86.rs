use crate::passes::analyze_relocations::ResolvedAt;
use crate::passes::generate_got::Got;
use crate::passes::generate_plt::GeneratePltArchOutput;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationMode, RelocationType};
use crate::repr::symbols::SymbolId;
use crate::utils::x86_codegen::{
    X86Arch, X86Codegen, X86Instruction::*, X86Reference::*, X86Value,
};
use plinky_elf::raw::{RawRel, RawRela};
use plinky_utils::ints::ExtractNumber;
use plinky_utils::raw_types::RawType;
use std::collections::BTreeMap;

pub(crate) fn generate_plt(
    object: &Object,
    got_plt: &Got,
    plt_symbol: SymbolId,
) -> GeneratePltArchOutput {
    let mut codegen = X86Codegen::new(X86Arch::X86);

    let plt_reloc = X86Value::Relocation {
        type_: RelocationType::Relative32,
        symbol: plt_symbol,
        addend: 0i64.into(),
    };

    codegen.encode(PushReference(EbxPlus(v(0x4))));
    codegen.encode(JumpReference(EbxPlus(v(0x8))));
    for _ in 0..4 {
        codegen.encode(Nop);
    }

    // Ensure alignment.
    debug_assert!(codegen.len() % 16 == 0);

    let reloc_size: i32 = match object.relocation_mode() {
        RelocationMode::Rel => RawRel::size(object.env.class.into()) as _,
        RelocationMode::Rela => RawRela::size(object.env.class.into()) as _,
    };

    let mut extra_got_plt_relocations = Vec::new();
    let mut offsets = BTreeMap::new();
    for (symbol, got_entry) in got_plt.entries.iter() {
        offsets.insert(*symbol, codegen.current_offset());

        codegen.encode(JumpReference(EbxPlus(v(got_entry.offset.extract().try_into().unwrap()))));

        match got_entry.resolved_at {
            ResolvedAt::RunTime => {
                let reloc_idx: i32 = got_entry
                    .dynamic_relocation_index
                    .expect("no dynamic relocation index for a runtime got entry")
                    .try_into()
                    .expect("too many got entries");

                let lazy_jump_target = codegen.current_offset();
                codegen.encode(PushImmediate(X86Value::Known(reloc_idx * reloc_size)));
                codegen.encode(JumpRelative(plt_reloc));

                // When relocations are resolved at runtime (and the dynamic linker is involved),
                // it's possible that relocations will be resolved lazily, which requires the first
                // instruction not to jump, letting the rest of the instructions execute.
                //
                // In order to do that though, we need to ensure the placeholder value of the
                // .got.plt for this slot is the address of the second instruction. If eager
                // resolution is enabled, the placeholder will be overridden at startup, while if
                // lazy resolution is enabled it will allow executing the rest of the PLT slot.
                extra_got_plt_relocations.push(Relocation {
                    type_: RelocationType::Absolute32,
                    symbol: plt_symbol,
                    offset: got_entry.offset,
                    addend: lazy_jump_target.into(),
                });
            }
            ResolvedAt::LinkTime => {
                for _ in 0..10 {
                    codegen.encode(Nop);
                }
            }
        }

        // Ensure alignment.
        debug_assert!(codegen.len() % 16 == 0);
    }

    let (content, relocations) = codegen.finish();
    GeneratePltArchOutput { content, relocations, extra_got_plt_relocations, offsets }
}

fn v(value: i32) -> X86Value {
    X86Value::Known(value)
}
