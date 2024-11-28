use crate::template::Template;
use serde::Deserialize;
use serde::de::Visitor;

struct TemplateVisitor;

impl<'de> Visitor<'de> for TemplateVisitor {
    type Value = Template;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a template")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Template::parse(v).map_err(|e| E::custom(e.to_string()))
    }
}

impl<'de> Deserialize<'de> for Template {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TemplateVisitor)
    }
}
