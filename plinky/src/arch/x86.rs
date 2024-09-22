use crate::passes::generate_got::GOT;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::utils::x86_codegen::{
    X86Arch, X86Codegen, X86Instruction::*, X86Reference::*, X86Value,
};
use plinky_elf::ids::serial::SymbolId;
use plinky_utils::ints::{ExtractNumber, Offset};
use std::collections::BTreeMap;

pub(crate) fn generate_plt(
    got_plt: &GOT,
    plt_symbol: SymbolId,
) -> (Vec<u8>, Vec<Relocation>, BTreeMap<SymbolId, Offset>) {
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

    let mut offsets = BTreeMap::new();
    for (idx, (symbol, got_offset)) in got_plt.offsets.iter().enumerate() {
        offsets.insert(*symbol, i64::try_from(codegen.len()).unwrap().into());

        codegen.encode(JumpReference(EbxPlus(v(got_offset.extract().try_into().unwrap()))));
        codegen.encode(PushImmediate(X86Value::Known(idx as _)));
        codegen.encode(JumpRelative(plt_reloc));

        // Ensure alignment.
        debug_assert!(codegen.len() % 16 == 0);
    }

    let (content, relocations) = codegen.finish();
    (content, relocations, offsets)
}

fn v(value: i32) -> X86Value {
    X86Value::Known(value)
}
