#[derive(Debug, PartialEq, Eq)]
pub enum FilterPart<'a> {
    Special(&'a str),
    StringFilter(StringFilter),
}

impl FilterPart<'_> {
    pub fn parse_iter<'a>(
        raw: &'a str,
    ) -> impl Iterator<Item = Result<FilterPart<'a>, FilterParseError>> + 'a {
        raw.split(',').map(|part| {
            if part.is_empty() {
                return Err(FilterParseError::EmptyFilter);
            }

            let part = part.trim();
            if let Some(special) = part.strip_prefix('@') {
                if special.is_empty() {
                    return Err(FilterParseError::EmptyFilter);
                }
                Ok(FilterPart::Special(special))
            } else {
                Ok(FilterPart::StringFilter(StringFilter::parse(part)?))
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StringFilter {
    Exact(String),
    Wildcard { before: String, after: String },
}

impl StringFilter {
    pub fn parse(raw: &str) -> Result<Self, FilterParseError> {
        let mut iter = raw.split('*');
        let Some(first) = iter.next() else {
            return Ok(StringFilter::Exact(String::new()));
        };
        let Some(second) = iter.next() else {
            return Ok(StringFilter::Exact(first.into()));
        };
        if iter.next().is_none() {
            Ok(StringFilter::Wildcard { before: first.into(), after: second.into() })
        } else {
            Err(FilterParseError::MoreThanOneWildcard(raw.into()))
        }
    }

    pub fn matches(&self, input: &str) -> bool {
        match self {
            StringFilter::Exact(pattern) => input == pattern,
            StringFilter::Wildcard { before, after } => {
                input.starts_with(before) && input.ends_with(after)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FilterParseError {
    MoreThanOneWildcard(String),
    EmptyFilter,
}

impl std::error::Error for FilterParseError {}

impl std::fmt::Display for FilterParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterParseError::MoreThanOneWildcard(wildcard) => {
                write!(f, "more than one wildcard used in filter: {wildcard}")
            }
            FilterParseError::EmptyFilter => f.write_str("empty filter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filter_iter() {
        let parse = |raw| FilterPart::parse_iter(raw).collect::<Result<Vec<_>, _>>();

        assert!(matches!(parse(""), Err(FilterParseError::EmptyFilter)));
        assert!(matches!(parse("foo,,bar"), Err(FilterParseError::EmptyFilter)));
        assert!(matches!(parse("@"), Err(FilterParseError::EmptyFilter)));
        assert_eq!(vec![FilterPart::Special("foo")], parse("@foo").unwrap());
        assert_eq!(
            vec![FilterPart::Special("foo"), FilterPart::Special("bar")],
            parse("@foo,@bar").unwrap()
        );
        assert_eq!(
            vec![
                FilterPart::StringFilter(StringFilter::Exact("foo".into())),
                FilterPart::StringFilter(StringFilter::Wildcard {
                    before: "bar".into(),
                    after: "".into()
                }),
            ],
            parse("foo,bar*").unwrap()
        );
        assert_eq!(
            vec![
                FilterPart::StringFilter(StringFilter::Exact("foo".into())),
                FilterPart::Special("bar"),
                FilterPart::StringFilter(StringFilter::Wildcard {
                    before: "baz".into(),
                    after: "".into()
                }),
            ],
            parse("foo,@bar,baz*").unwrap()
        );
    }

    #[test]
    fn test_parse_string_filter() {
        assert_eq!(StringFilter::Exact("".into()), StringFilter::parse("").unwrap());
        assert_eq!(StringFilter::Exact("foo".into()), StringFilter::parse("foo").unwrap());
        assert_eq!(
            StringFilter::Wildcard { before: "".into(), after: "foo".into() },
            StringFilter::parse("*foo").unwrap()
        );
        assert_eq!(
            StringFilter::Wildcard { before: "foo".into(), after: "".into() },
            StringFilter::parse("foo*").unwrap()
        );
        assert_eq!(
            StringFilter::Wildcard { before: "foo".into(), after: "bar".into() },
            StringFilter::parse("foo*bar").unwrap()
        );
        match StringFilter::parse("foo**bar") {
            Err(FilterParseError::MoreThanOneWildcard(bad)) if bad == "foo**bar" => {}
            other => panic!("invalid result: {other:?}"),
        }
    }
}
