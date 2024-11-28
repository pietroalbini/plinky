use plinky_macros::{Display, Error};
use plinky_utils::filters_parser::{FilterParseError, FilterPart, StringFilter};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ObjectsFilter {
    pub(super) env: bool,
    sections: SectionsFilter,
    pub(super) symbols: bool,
    pub(super) dynamic: bool,
    pub(super) inputs: bool,
}

impl ObjectsFilter {
    pub(crate) fn all() -> Self {
        ObjectsFilter {
            env: true,
            sections: SectionsFilter::All,
            symbols: true,
            dynamic: true,
            inputs: true,
        }
    }

    pub(crate) fn parse(raw: &str) -> Result<Self, ObjectsFilterParseError> {
        let mut filter = ObjectsFilter {
            env: false,
            sections: SectionsFilter::None,
            symbols: false,
            dynamic: false,
            inputs: false,
        };

        for part in FilterPart::parse_iter(raw) {
            match part? {
                FilterPart::Special("env") => filter.env = true,
                FilterPart::Special("symbols") => filter.symbols = true,
                FilterPart::Special("dynamic") => filter.dynamic = true,
                FilterPart::Special("inputs") => filter.inputs = true,
                FilterPart::Special("sections") => match &filter.sections {
                    SectionsFilter::Some(_) => {
                        return Err(ObjectsFilterParseError::CantMixSectionFilters);
                    }
                    SectionsFilter::None | SectionsFilter::All => {
                        filter.sections = SectionsFilter::All;
                    }
                },
                FilterPart::Special(other) => {
                    return Err(ObjectsFilterParseError::InvalidSpecialFilter(other.into()));
                }
                FilterPart::StringFilter(section_filter) => match &mut filter.sections {
                    SectionsFilter::Some(section_filters) => section_filters.push(section_filter),
                    SectionsFilter::All => {
                        return Err(ObjectsFilterParseError::CantMixSectionFilters);
                    }
                    SectionsFilter::None => {
                        filter.sections = SectionsFilter::Some(vec![section_filter])
                    }
                },
            }
        }

        Ok(filter)
    }

    pub(super) fn section(&self, name: &str) -> bool {
        match &self.sections {
            SectionsFilter::None => false,
            SectionsFilter::All => true,
            SectionsFilter::Some(filters) => filters.iter().any(|filter| filter.matches(name)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SectionsFilter {
    None,
    All,
    Some(Vec<StringFilter>),
}

#[derive(Debug, Display, Error, PartialEq, Eq)]
pub(crate) enum ObjectsFilterParseError {
    #[display("invalid special filter: {f0}")]
    InvalidSpecialFilter(String),
    #[display("can't mix @sections and individual section filters")]
    CantMixSectionFilters,
    #[transparent]
    Utils(FilterParseError),
}
