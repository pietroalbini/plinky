use crate::repr::object::{GnuProperties, Object};
use crate::repr::sections::NotesSection;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use plinky_elf::{ElfGnuProperty, ElfNote, ElfPermissions};
use std::ops::BitOr;

pub(crate) fn run(object: &mut Object) {
    // There are multiple algorithms to merge properties, depending on the numeric representation
    // of the property type:
    //
    // | Low        | High       | Mode          |
    // |------------|------------|---------------|
    // | 0xc0000002 | 0xc0007fff | UINT32_AND    |
    // | 0xc0008000 | 0xc000ffff | UINT32_OR     |
    // | 0xc0010000 | 0xc0017fff | UINT32_OR_AND |
    //
    // Documentation for how to perform the merging is available at:
    //
    //     https://gitlab.com/x86-psABIs/x86-64-ABI/-/merge_requests/13
    //
    let mut merged_x86_isa_used = AndOrState::Initial;
    let mut merged_x86_features2_used = AndOrState::Initial;

    for input in &object.inputs {
        let GnuProperties { x86_isa_used, x86_features_2_used } = &input.gnu_properties;

        merged_x86_isa_used.merge(*x86_isa_used);
        merged_x86_features2_used.merge(*x86_features_2_used);
    }

    let mut properties = Vec::new();
    if let Some(val) = merged_x86_isa_used.prepare_for_adding() {
        properties.push(ElfGnuProperty::X86IsaUsed(val));
    }
    if let Some(val) = merged_x86_features2_used.prepare_for_adding() {
        properties.push(ElfGnuProperty::X86Features2Used(val));
    }

    if !properties.is_empty() {
        let section = NotesSection::new(vec![ElfNote::GnuProperties(properties)]);
        let align = section.alignment(object.env.class);
        let id = object.sections.builder(".note.gnu.property", section).create();

        object.segments.add(Segment {
            align,
            type_: SegmentType::GnuProperty,
            perms: ElfPermissions::R,
            content: vec![SegmentContent::Section(id)],
        });
    }
}

enum AndOrState<T> {
    Initial,
    Present(T),
    Missing,
}

impl<T: BitOr<T, Output = T> + Copy> AndOrState<T> {
    fn merge(&mut self, other: Option<T>) {
        match (&self, other) {
            (_, None) | (AndOrState::Missing, _) => *self = AndOrState::Missing,
            (AndOrState::Initial, Some(initial)) => *self = AndOrState::Present(initial),
            (AndOrState::Present(old), Some(new)) => *self = AndOrState::Present(*old | new),
        }
    }

    fn prepare_for_adding(self) -> Option<T> {
        match self {
            AndOrState::Initial => panic!("adding a note in the initial state"),
            AndOrState::Present(val) => Some(val),
            AndOrState::Missing => None,
        }
    }
}
