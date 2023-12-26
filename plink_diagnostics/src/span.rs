use std::path::PathBuf;

#[derive(Clone)]
pub struct ObjectSpan(ObjectSpanInner);

impl ObjectSpan {
    pub fn new_file(path: impl Into<PathBuf>) -> Self {
        ObjectSpan(ObjectSpanInner::File(path.into()))
    }

    pub fn new_archive_member(archive: impl Into<PathBuf>, member: impl Into<String>) -> Self {
        ObjectSpan(ObjectSpanInner::ArchiveMember {
            archive: archive.into(),
            member: member.into(),
        })
    }

    #[must_use]
    pub fn merge(&self, other: &ObjectSpan) -> ObjectSpan {
        let mut new_mix = Vec::new();
        match (&self.0, &other.0) {
            (ObjectSpanInner::Mix(mix1), ObjectSpanInner::Mix(mix2)) => {
                new_mix.extend(mix1.iter().cloned());
                new_mix.extend(mix2.iter().cloned());
            }
            (ObjectSpanInner::Mix(mix), non_mix) | (non_mix, ObjectSpanInner::Mix(mix)) => {
                new_mix.extend(mix.iter().cloned());
                new_mix.push(ObjectSpan(non_mix.clone()));
            }
            (non_mix1, non_mix2) => {
                new_mix.push(ObjectSpan(non_mix1.clone()));
                new_mix.push(ObjectSpan(non_mix2.clone()));
            }
        }
        ObjectSpan(ObjectSpanInner::Mix(new_mix))
    }
}

impl std::fmt::Display for ObjectSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            ObjectSpanInner::File(file) => write!(f, "{}", file.display()),
            ObjectSpanInner::ArchiveMember { archive, member } => {
                write!(f, "{member} inside archive {}", archive.display())
            }
            ObjectSpanInner::Mix(items) => {
                f.write_str("mix of ")?;
                for (idx, item) in items.iter().enumerate() {
                    <ObjectSpan as std::fmt::Display>::fmt(item, f)?;
                    if idx < items.len() - 2 {
                        f.write_str(", ")?;
                    } else if idx == items.len() - 2 {
                        f.write_str(" and ")?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::fmt::Debug for ObjectSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Display>::fmt(self, f)
    }
}

#[derive(Clone)]
enum ObjectSpanInner {
    File(PathBuf),
    ArchiveMember { archive: PathBuf, member: String },
    Mix(Vec<ObjectSpan>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repr() {
        let span1 = ObjectSpan::new_file("foo.o");
        let span2 = ObjectSpan::new_archive_member("libutils.a", "bar.o");
        let span3 = ObjectSpan::new_file("baz.o");
        let span4 = ObjectSpan::new_file("quux.o");

        assert_eq!("foo.o", span1.to_string());
        assert_eq!("bar.o inside archive libutils.a", span2.to_string());
        assert_eq!(
            "mix of foo.o and bar.o inside archive libutils.a",
            span1.merge(&span2).to_string()
        );
        assert_eq!(
            "mix of foo.o, bar.o inside archive libutils.a and baz.o",
            span1.merge(&span2).merge(&span3).to_string()
        );
        assert_eq!(
            "mix of foo.o, bar.o inside archive libutils.a, baz.o and quux.o",
            span1.merge(&span2).merge(&span3).merge(&span4).to_string()
        );
    }
}
