use crate::passes::analyze_relocations::ResolvedAt;
use crate::passes::generate_got::Got;
use crate::passes::generate_plt::GeneratePltArchOutput;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::symbols::SymbolId;
use crate::utils::x86_codegen::{
    X86Arch, X86Codegen, X86Instruction::*, X86Reference::*, X86Value,
};
use plinky_utils::ints::ExtractNumber;
use std::collections::BTreeMap;

pub(crate) fn generate_plt(got_plt: &Got, plt_symbol: SymbolId) -> GeneratePltArchOutput {
    let got_plt_symbol = got_plt.symbol.expect(".got.plt without the symbol");
    let mut codegen = X86Codegen::new(X86Arch::X86_64);

    let got_plt_reloc = |addend: i64| -> _ {
        X86Value::Relocation {
            type_: RelocationType::Relative32,
            symbol: got_plt_symbol,
            addend: addend.into(),
        }
    };

    let plt_reloc = X86Value::Relocation {
        type_: RelocationType::Relative32,
        symbol: plt_symbol,
        addend: 0i64.into(),
    };

    codegen.encode(PushReference(RipRelativeDisplacement(got_plt_reloc(0x08))));
    codegen.encode(JumpReference(RipRelativeDisplacement(got_plt_reloc(0x10))));
    for _ in 0..4 {
        codegen.encode(Nop);
    }

    // Ensure alignment.
    debug_assert!(codegen.len() % 16 == 0);

    let mut extra_got_plt_relocations = Vec::new();
    let mut offsets = BTreeMap::new();
    for (symbol, got_entry) in got_plt.entries.iter() {
        offsets.insert(*symbol, codegen.current_offset());

        codegen.encode(JumpReference(RipRelativeDisplacement(got_plt_reloc(
            got_entry.offset.extract(),
        ))));

        match got_entry.resolved_at {
            ResolvedAt::RunTime => {
                let lazy_jump_target = codegen.current_offset();
                codegen.encode(PushImmediate(X86Value::Known(
                    got_entry
                        .dynamic_relocation_index
                        .expect("no dynamic relocation index for a runtime got entry")
                        .try_into()
                        .expect("too many got entries"),
                )));
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
