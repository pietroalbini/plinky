use crate::repr::relocations::{Relocation, RelocationType};
use plinky_elf::ids::serial::SymbolId;
use plinky_utils::ints::Offset;

#[derive(Debug, Clone, Copy)]
pub(crate) enum X86Instruction {
    PushImmediate(X86Value),
    PushReference(X86Reference),
    JumpRelative(X86Value),
    JumpReference(X86Reference),
    Nop,
}

pub(crate) struct X86Codegen {
    arch: X86Arch,
    buf: Vec<u8>,
    relocations: Vec<Relocation>,
    relocations_to_add: Vec<(Offset, Relocation)>,
}

impl X86Codegen {
    pub(crate) fn new(arch: X86Arch) -> Self {
        Self { arch, buf: Vec::new(), relocations: Vec::new(), relocations_to_add: Vec::new() }
    }

    pub(crate) fn len(&self) -> usize {
        self.buf.len()
    }

    pub(crate) fn finish(self) -> (Vec<u8>, Vec<Relocation>) {
        assert!(self.relocations_to_add.is_empty());
        (self.buf, self.relocations)
    }

    pub(crate) fn encode(&mut self, instruction: X86Instruction) {
        match instruction {
            // Instruction encoding: 68 id
            X86Instruction::PushImmediate(imm) => {
                self.buf.push(0x68);
                self.encode_value(imm);
            }
            // Instruction encoding: FF /6
            X86Instruction::PushReference(reference) => {
                self.buf.push(0xFF);
                self.encode_reference(reference, 6);
            }
            // Instruction encoding: E9 cd
            X86Instruction::JumpRelative(disp) => {
                self.buf.push(0xE9);
                self.encode_value(disp);
            }
            // Instruction encoding: FF /4
            X86Instruction::JumpReference(reference) => {
                self.buf.push(0xFF);
                self.encode_reference(reference, 4);
            }
            // Instruction encoding: 90
            X86Instruction::Nop => self.buf.push(0x90),
        }

        let end_offset = self.offset();
        while let Some((relocation_offset, mut relocation)) = self.relocations_to_add.pop() {
            match relocation.type_ {
                RelocationType::Absolute32 | RelocationType::AbsoluteSigned32 => {
                    self.relocations.push(relocation)
                }

                RelocationType::Relative32 => {
                    // Instructions operating on relative addresses use the current instruction
                    // pointer as the starting point for relative calculations. In x86 and x86_64,
                    // the instruction pointer points to the *next* instruction.
                    //
                    // Linkers on the other hand (including plinky) treat the offset the relocation
                    // is applied to as the starting point for relative calculations, which is
                    // earlier than the instruction pointer will be at runtime.
                    //
                    // To ensure everything works correctly, we have to adjust the addend to
                    // account for the difference in starting points.
                    let addend = relocation.addend.as_mut().expect("addend must be present");
                    *addend = addend.add(relocation_offset.add(end_offset.neg()).unwrap()).unwrap();

                    self.relocations.push(relocation);
                }

                type_ => panic!("unsupported relocation {type_:?} during codegen"),
            }
        }
    }

    fn encode_reference(&mut self, reference: X86Reference, modrm_opcode: u8) {
        match reference {
            // rip-relative displacement is only available in x86-64, and is represented by a
            // ModR/M byte for disp32, which on x86-64 is interpreted as `[rip] + disp32` instead
            // of, you know, `disp32`.
            X86Reference::RipRelativeDisplacement(value) => match self.arch {
                X86Arch::X86 => panic!("rip-relative displacement is not available on x86"),
                X86Arch::X86_64 => {
                    self.buf.push(modrm(0b00, modrm_opcode, 0b101));
                    self.encode_value(value);
                }
            },
            X86Reference::Displacement(value) => {
                match self.arch {
                    X86Arch::X86 => {
                        self.buf.push(modrm(0b00, modrm_opcode, 0b101));
                    }
                    X86Arch::X86_64 => {
                        // On x86-64, just using the ModR/M byte results in the reference being
                        // rip-relative. The Intel manual recommends using the SIB byte on x86-64
                        // to represent a plain disp32.
                        self.buf.push(modrm(0b00, modrm_opcode, 0b100));
                        self.buf.push(sib(0b00, 0b100, 0b101));
                    }
                }
                self.encode_value(value);
            }
            X86Reference::EbxPlus(value) => {
                self.buf.push(modrm(0b10, modrm_opcode, 0b011));
                self.encode_value(value);
            }
        }
    }

    fn encode_value(&mut self, value: X86Value) {
        let bytes = match value {
            X86Value::Known(value) => value.to_le_bytes(),
            X86Value::Relocation { type_, symbol, addend } => {
                // Relocations are added to a temporary list rather than directly into the
                // relocations list because some processing might be needed when finishing to
                // encode the instruction, depending on the relocation type.
                let offset = self.offset();
                self.relocations_to_add
                    .push((offset, Relocation { type_, symbol, offset, addend: Some(addend) }));

                // Placeholder, as it will be relocated later.
                0i32.to_le_bytes()
            }
        };
        self.buf.extend_from_slice(&bytes);
    }

    fn offset(&self) -> Offset {
        Offset::from(i64::try_from(self.buf.len()).expect("generated x86 too large"))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum X86Arch {
    X86,
    X86_64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum X86Value {
    Known(i32),
    Relocation { type_: RelocationType, symbol: SymbolId, addend: Offset },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum X86Reference {
    EbxPlus(X86Value),
    #[cfg_attr(not(test), expect(dead_code))]
    Displacement(X86Value),
    RipRelativeDisplacement(X86Value),
}

fn modrm(mod_: u8, reg_opcode: u8, rm: u8) -> u8 {
    ((mod_ & 0b11) << 6) | ((reg_opcode & 0b111) << 3) | (rm & 0b111)
}

fn sib(scale: u8, index: u8, base: u8) -> u8 {
    ((scale & 0b11) << 6) | ((index & 0b111) << 3) | (base & 0b111)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plinky_elf::ids::serial::SerialIds;

    #[track_caller]
    fn encode_all<const N: usize>(instruction: X86Instruction, expected: [u8; N]) {
        for arch in [X86Arch::X86, X86Arch::X86_64] {
            encode(arch, instruction, expected);
        }
    }

    #[track_caller]
    fn encode<const N: usize>(arch: X86Arch, instruction: X86Instruction, expected: [u8; N]) {
        let mut codegen = X86Codegen::new(arch);
        codegen.encode(instruction);

        assert_eq!(&codegen.buf, &expected, "wrong encoding for arch {arch:?}");
    }

    fn v(known: i32) -> X86Value {
        X86Value::Known(known)
    }

    #[test]
    fn test_modrm_encoding() {
        // Example from the Intel manual.
        assert_eq!(modrm(0b11, 0b001, 0), 0b11001000);
    }

    #[test]
    fn test_sib_encoding() {
        // Example from the Intel manual.
        assert_eq!(sib(0b11, 0b001, 0), 0b11001000);
    }

    #[test]
    fn test_value_encoding() {
        let mut ids = SerialIds::new();
        let symbol = ids.allocate_symbol_id();

        let mut codegen = X86Codegen::new(X86Arch::X86_64);
        codegen.encode(X86Instruction::PushImmediate(X86Value::Known(42)));

        assert_eq!(&[0x68, 42, 0, 0, 0], codegen.buf.as_slice());
        assert!(codegen.relocations.is_empty());

        codegen.encode(X86Instruction::PushImmediate(X86Value::Relocation {
            type_: RelocationType::Absolute32,
            symbol,
            addend: 42_i64.into(),
        }));
        assert_eq!(
            &[/* First */ 0x68, 42, 0, 0, 0, /* Second */ 0x68, 0, 0, 0, 0,],
            codegen.buf.as_slice()
        );
        assert_eq!(
            &[Relocation {
                type_: RelocationType::Absolute32,
                symbol,
                offset: 6_i64.into(),
                addend: Some(42_i64.into())
            }],
            codegen.relocations.as_slice()
        );

        codegen.encode(X86Instruction::PushImmediate(X86Value::Relocation {
            type_: RelocationType::Relative32,
            symbol,
            addend: 42_i64.into(),
        }));
        assert_eq!(
            &[
                /* First */ 0x68, 42, 0, 0, 0, /* Second */ 0x68, 0, 0, 0, 0,
                /* Third */ 0x68, 0, 0, 0, 0
            ],
            codegen.buf.as_slice()
        );
        assert_eq!(
            &[
                Relocation {
                    type_: RelocationType::Absolute32,
                    symbol,
                    offset: 6_i64.into(),
                    addend: Some(42_i64.into())
                },
                Relocation {
                    type_: RelocationType::Relative32,
                    symbol,
                    offset: 11_i64.into(),
                    // This is less than 42 intentionally, as it needs to account for the
                    // instruction pointer being further ahead.
                    addend: Some(38_i64.into()),
                },
            ],
            codegen.relocations.as_slice()
        );
    }

    #[test]
    fn test_encode_push_immediate() {
        encode_all(X86Instruction::PushImmediate(v(42)), [0x68, 42, 0, 0, 0]);
    }

    #[test]
    fn test_encode_push_reference() {
        encode(
            X86Arch::X86_64,
            X86Instruction::PushReference(X86Reference::RipRelativeDisplacement(v(42))),
            [0xFF, 0x35, 42, 0, 0, 0],
        );

        encode(
            X86Arch::X86,
            X86Instruction::PushReference(X86Reference::Displacement(v(42))),
            [0xFF, 0x35, 42, 0, 0, 0],
        );
        encode(
            X86Arch::X86_64,
            X86Instruction::PushReference(X86Reference::Displacement(v(42))),
            [0xFF, 0x34, 0x25, 42, 0, 0, 0],
        );

        encode_all(
            X86Instruction::PushReference(X86Reference::EbxPlus(v(42))),
            [0xff, 0xb3, 42, 0, 0, 0],
        );
    }

    #[test]
    fn test_encode_jump_relative() {
        encode_all(X86Instruction::JumpRelative(v(42)), [0xE9, 42, 0, 0, 0]);
    }

    #[test]
    fn test_encode_jump_reference() {
        encode(
            X86Arch::X86_64,
            X86Instruction::JumpReference(X86Reference::RipRelativeDisplacement(v(42))),
            [0xFF, 0x25, 42, 0, 0, 0],
        );

        encode(
            X86Arch::X86,
            X86Instruction::JumpReference(X86Reference::Displacement(v(42))),
            [0xFF, 0x25, 42, 0, 0, 0],
        );
        encode(
            X86Arch::X86_64,
            X86Instruction::JumpReference(X86Reference::Displacement(v(42))),
            [0xFF, 0x24, 0x25, 42, 0, 0, 0],
        );

        encode_all(
            X86Instruction::JumpReference(X86Reference::EbxPlus(v(42))),
            [0xff, 0xa3, 42, 0, 0, 0],
        );
    }

    #[test]
    fn test_encode_nop() {
        encode_all(X86Instruction::Nop, [0x90]);
    }
}
