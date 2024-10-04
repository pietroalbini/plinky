#[macro_export]
macro_rules! new_serial_ids {
    (
        $ids_vis:vis $ids_ty:ident {
            type SectionId = $section_vis:vis $section_ty:ident;
            type SymbolId = $symbol_vis:vis $symbol_ty:ident;
            type StringId = $string_vis:vis $string_ty:ident;
        }
    ) => {
        use $crate::ids::convert::IdConversionMap;
        use $crate::ids::convert::ConvertibleElfIds;
        use $crate::ids::{ElfIds, ReprIdGetters, StringIdGetters};
        use $crate::{ElfObject, ElfSectionContent};

        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $section_vis struct $section_ty(usize);

        impl std::fmt::Debug for $section_ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "section#{}", self.0)
            }
        }

        impl ReprIdGetters for $section_ty {
            fn repr_id(&self) -> String {
                format!("{}", self.0)
            }
        }

        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $symbol_vis struct $symbol_ty(usize);

        impl std::fmt::Debug for $symbol_ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "symbol#{}", self.0)
            }
        }

        impl ReprIdGetters for $symbol_ty {
            fn repr_id(&self) -> String {
                format!("{}", self.0)
            }
        }

        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $string_vis struct $string_ty($section_ty, u32);

        impl $string_ty {
            $string_vis fn new(section: $section_ty, offset: u32) -> Self {
                Self(section, offset)
            }
        }

        impl std::fmt::Debug for $string_ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}:string#{}", self.0, self.1)
            }
        }

        impl StringIdGetters<$ids_ty> for $string_ty {
            fn section(&self) -> &$section_ty {
                &self.0
            }

            fn offset(&self) -> u32 {
                self.1
            }
        }

        #[derive(Debug)]
        $ids_vis struct $ids_ty {
            next_section_id: usize,
            next_symbol_id: usize,
        }

        impl ElfIds for $ids_ty {
            type SectionId = $section_ty;
            type SymbolId = $symbol_ty;
            type StringId = $string_ty;
        }

        impl<F> ConvertibleElfIds<F> for $ids_ty
        where
            F: ElfIds,
            F::StringId: StringIdGetters<F>,
        {
            fn create_conversion_map(
                &mut self,
                object: &ElfObject<F>,
                string_ids: &[F::StringId],
            ) -> IdConversionMap<F, Self> {
                let mut map = IdConversionMap::<F, Self>::new();

                for (old_id, section) in &object.sections {
                    map.section_ids.insert(old_id.clone(), self.allocate_section_id());

                    if let ElfSectionContent::SymbolTable(table) = &section.content {
                        for id in table.symbols.keys() {
                            map.symbol_ids.insert(id.clone(), self.allocate_symbol_id());
                        }
                    }
                }

                for string_id in string_ids {
                    map.string_ids.insert(
                        string_id.clone(),
                        $string_ty::new(
                            *map.section_ids.get(string_id.section()).expect("missing section"),
                            string_id.offset(),
                        ),
                    );
                }

                map
            }
        }

        impl $ids_ty {
            $ids_vis fn new() -> Self {
                Self { next_section_id: 0, next_symbol_id: 0 }
            }

            $ids_vis fn allocate_section_id(&mut self) -> $section_ty {
                let id = $section_ty(self.next_section_id);
                self.next_section_id += 1;
                id
            }

            $ids_vis fn allocate_symbol_id(&mut self) -> $symbol_ty {
                let id = $symbol_ty(self.next_symbol_id);
                self.next_symbol_id += 1;
                id
            }
        }
    };
}

new_serial_ids! {
    pub SerialIds {
        type SectionId = pub SerialSectionId;
        type SymbolId = pub SerialSymbolId;
        type StringId = pub SerialStringId;
    }
}
