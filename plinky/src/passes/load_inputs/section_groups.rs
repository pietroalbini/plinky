use crate::interner::{intern, Interned};
use crate::passes::load_inputs::strings::{MissingStringError, Strings};
use plinky_elf::ids::serial::{SectionId, SerialIds, SerialSymbolId, StringId};
use plinky_elf::{ElfGroup, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable};
use plinky_macros::{Display, Error};
use std::collections::BTreeSet;

pub(super) struct SectionGroups {
    loaded_groups: BTreeSet<Interned<String>>,
    group_section_ids: BTreeSet<SectionId>,
}

impl SectionGroups {
    pub(super) fn new() -> Self {
        Self { loaded_groups: BTreeSet::new(), group_section_ids: BTreeSet::new() }
    }

    pub(super) fn for_object(&mut self) -> SectionGroupsForObject<'_> {
        SectionGroupsForObject { parent: self, remove_sections: BTreeSet::new() }
    }

    pub(super) fn is_section_a_group_definition(&self, id: SectionId) -> bool {
        self.group_section_ids.contains(&id)
    }
}

pub(super) struct SectionGroupsForObject<'a> {
    parent: &'a mut SectionGroups,
    remove_sections: BTreeSet<SectionId>,
}

impl SectionGroupsForObject<'_> {
    pub(super) fn add_group(
        &mut self,
        strings: &Strings,
        symbol_tables: &[(StringId, ElfSymbolTable<SerialIds>)],
        id: SectionId,
        group: ElfGroup<SerialIds>,
    ) -> Result<(), SectionGroupsError> {
        // Right now we are only implementing section groups for the x86 snippet of code
        // used to determine the current instruction pointer (__x86.get_pc_thunk.bx). That
        // group is COMDAT with a single section, so we reject anything that is not that.
        //
        // Note that supporting section groups with more than a section will require
        // updating gc_sections to keep other sections in the group if any section is
        // marked to be kept.
        if !group.comdat {
            return Err(SectionGroupsError::NoCOMDAT);
        }
        let section_id = if let [section] = group.sections.as_slice() {
            *section
        } else {
            return Err(SectionGroupsError::MultipleSections);
        };

        let signature = intern(
            strings
                .get(
                    symbol_tables
                        .iter()
                        .filter_map(|(_, table)| table.symbols.get(&group.signature))
                        .next()
                        .ok_or(SectionGroupsError::MissingSignatureSymbol(group.signature))?
                        .name,
                )
                .map_err(SectionGroupsError::MissingSignatureString)?,
        );

        if !self.parent.loaded_groups.insert(signature) {
            self.remove_sections.insert(section_id);
        }

        self.parent.group_section_ids.insert(id);

        Ok(())
    }

    pub(super) fn filter_symbol_table(
        &self,
        table: &mut ElfSymbolTable<SerialIds>,
    ) -> Result<(), SectionGroupsError> {
        // If the group is already loaded in another object file, mark each symbol pointing to it
        // as undefined, so that the rest of the linker will resolve the symbol to the retained
        // instance of the group.
        for (id, symbol) in table.symbols.iter_mut() {
            let ElfSymbolDefinition::Section(section_id) = &symbol.definition else { continue };
            if !self.remove_sections.contains(section_id) {
                continue;
            }

            // Having a non-global symbol pointing into the group is likely going to cause
            // problems (as the symbols from deleted sections won't be merged with the retained
            // sections). To avoid confusing errors later down the line we error out early.
            if let ElfSymbolBinding::Local = symbol.binding {
                return Err(SectionGroupsError::NonGlobalSymbolInGroup(*id));
            }

            symbol.definition = ElfSymbolDefinition::Undefined;
        }

        Ok(())
    }

    pub(super) fn should_skip_section(&self, id: SectionId) -> bool {
        self.remove_sections.contains(&id)
    }
}

#[derive(Debug, Error, Display)]
pub(crate) enum SectionGroupsError {
    #[display("only COMDAT section groups are currently supported")]
    NoCOMDAT,
    #[display("only section groups with a single section are supported")]
    MultipleSections,
    #[display("group signature {f0:?} is not present in any symbol table")]
    MissingSignatureSymbol(SerialSymbolId),
    #[display("symbol {f0:?} points inside a section group but is not global")]
    NonGlobalSymbolInGroup(SerialSymbolId),
    #[display("missing group signature")]
    MissingSignatureString(#[source] MissingStringError),
}
