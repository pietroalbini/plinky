#![cfg_attr(not(test), expect(unused))]

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
}

impl X86Codegen {
    fn new(arch: X86Arch) -> Self {
        Self { arch, buf: Vec::new(), relocations: Vec::new() }
    }

    fn encode(&mut self, instruction: X86Instruction) {
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
    }

    fn encode_reference(&mut self, reference: X86Reference, modrm_opcode: u8) {
        match reference {
            // rip-relative displacement is only available in x86-64, and is represented by a
            // ModR/M byte for disp32, which on x86-64 is interpreted as `[rip] + disp32` instead
            // of, you know, `disp32`.
            X86Reference::RipRelativeDisplacement(disp32) => match self.arch {
                X86Arch::X86 => panic!("rip-relative displacement is not available on x86"),
                X86Arch::X86_64 => {
                    self.buf.push(modrm(0b00, modrm_opcode, 0b101));
                    self.encode_value(disp32);
                }
            },
            X86Reference::Displacement(disp32) => {
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
                self.encode_value(disp32);
            }
        }
    }

    fn encode_value(&mut self, value: X86Value) {
        let bytes = match value {
            X86Value::Known(value) => value.to_le_bytes(),
            X86Value::Relocation { type_, symbol, addend } => {
                self.relocations.push(Relocation {
                    type_,
                    symbol,
                    offset: Offset::from(
                        i64::try_from(self.buf.len()).expect("generated x86 too large"),
                    ),
                    addend: Some(addend),
                });
                0i32.to_le_bytes()
            }
        };
        self.buf.extend_from_slice(&bytes);
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
    }

    #[test]
    fn test_encode_nop() {
        encode_all(X86Instruction::Nop, [0x90]);
    }
}
