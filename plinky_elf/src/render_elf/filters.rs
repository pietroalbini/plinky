use plinky_macros::{Display, Error};
use plinky_utils::filters_parser::{FilterParseError, FilterPart, StringFilter};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RenderElfFilters {
    pub(super) meta: bool,
    sections: SectionsFilter,
    pub(super) segments: bool,
}

impl RenderElfFilters {
    pub fn all() -> Self {
        RenderElfFilters { meta: true, sections: SectionsFilter::All, segments: true }
    }

    pub fn parse(raw: &str) -> Result<Self, RenderElfFiltersParseError> {
        let mut filter =
            RenderElfFilters { meta: false, sections: SectionsFilter::None, segments: false };

        for part in FilterPart::parse_iter(raw) {
            match part? {
                FilterPart::Special("meta") => filter.meta = true,
                FilterPart::Special("segments") => filter.segments = true,
                FilterPart::Special("sections") => match &filter.sections {
                    SectionsFilter::Some(_) => {
                        return Err(RenderElfFiltersParseError::CantMixSectionFilters)
                    }
                    SectionsFilter::None | SectionsFilter::All => {
                        filter.sections = SectionsFilter::All;
                    }
                },
                FilterPart::Special(other) => {
                    return Err(RenderElfFiltersParseError::InvalidSpecialFilter(other.into()))
                }
                FilterPart::StringFilter(section_filter) => match &mut filter.sections {
                    SectionsFilter::Some(section_filters) => section_filters.push(section_filter),
                    SectionsFilter::All => {
                        return Err(RenderElfFiltersParseError::CantMixSectionFilters)
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
pub enum RenderElfFiltersParseError {
    #[display("invalid special filter: {f0}")]
    InvalidSpecialFilter(String),
    #[display("can't mix @sections and individual section filters")]
    CantMixSectionFilters,
    #[transparent]
    Utils(FilterParseError),
}
