use crate::error::Error;
use crate::parser::{Attribute, Enum, EnumVariantData, Parser};
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_enum()?;

    let mut output = String::new();
    generate_error_impl(&mut output, &parsed)?;
    generate_from_impls(&mut output, &parsed)?;

    Ok(output.parse().unwrap())
}

fn generate_error_impl(output: &mut String, parsed: &Enum) -> Result<(), Error> {
    output.push_str(&format!("impl std::error::Error for {} {{", parsed.name));

    output.push_str("fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {");
    output.push_str("match self {");
    for variant in &parsed.variants {
        let mut source = None;
        output.push_str(&format!("Self::{}", variant.name));
        match &variant.data {
            EnumVariantData::None => {}
            EnumVariantData::TupleLike(fields) => {
                output.push_str("(");
                for (idx, field) in fields.iter().enumerate() {
                    let name = format!("field{}", idx);
                    output.push_str(&name);
                    output.push_str(",");
                    maybe_set_source(&mut source, &name, &field.attrs)?;
                }
                output.push_str(")");
            }
            EnumVariantData::StructLike(fields) => {
                output.push_str("{");
                for field in fields {
                    output.push_str(&field.name);
                    output.push_str(",");
                    maybe_set_source(&mut source, &field.name, &field.attrs)?;
                }
                output.push_str("}");
            }
        }
        output.push_str(" => ");
        match source {
            Some(source) => output.push_str(&format!("Some({source})")),
            None => output.push_str("None"),
        }
        output.push_str(",");
    }
    output.push_str("}");
    output.push_str("}");

    output.push_str("}");
    Ok(())
}

fn maybe_set_source(
    source: &mut Option<String>,
    name: &str,
    attrs: &[Attribute],
) -> Result<(), Error> {
    if let Some(attr) = attrs
        .iter()
        .find(|a| a.value == "from" || a.value == "source")
    {
        if source.is_some() {
            Err(Error::new("multiple sources for the same variant").span(attr.span))
        } else {
            *source = Some(name.into());
            Ok(())
        }
    } else {
        Ok(())
    }
}

fn generate_from_impls(output: &mut String, parsed: &Enum) -> Result<(), Error> {
    for variant in &parsed.variants {
        let mut fields = match &variant.data {
            EnumVariantData::None => continue,
            EnumVariantData::TupleLike(fields) => fields
                .iter()
                .map(|f| FromImplField {
                    attrs: &f.attrs,
                    field: "value",
                    ty: &f.ty,
                    open_assign: "(",
                    close_assign: ")",
                })
                .collect::<Vec<_>>(),
            EnumVariantData::StructLike(fields) => fields
                .iter()
                .map(|f| FromImplField {
                    attrs: &f.attrs,
                    field: &f.name,
                    ty: &f.ty,
                    open_assign: "{",
                    close_assign: "}",
                })
                .collect::<Vec<_>>(),
        };

        let has_from = fields
            .iter()
            .any(|f| f.attrs.iter().any(|a| a.value == "from"));
        if !has_from {
            continue;
        } else if fields.len() > 1 {
            return Err(Error::new("#[from] in variant with multiple fields").span(variant.span));
        }

        generate_from_impl(output, &parsed.name, &variant.name, fields.pop().unwrap());
    }
    Ok(())
}

fn generate_from_impl(
    output: &mut String,
    enum_name: &str,
    variant_name: &str,
    field: FromImplField<'_>,
) {
    output.push_str(&format!("impl From<{}> for {enum_name} {{", field.ty));
    output.push_str(&format!(
        "fn from({}: {}) -> Self {{",
        field.field, field.ty
    ));
    output.push_str(&format!(
        "{enum_name}::{variant_name}{}{}{}",
        field.open_assign, field.field, field.close_assign
    ));
    output.push_str("}}");
}

struct FromImplField<'a> {
    attrs: &'a [Attribute],
    field: &'a str,
    ty: &'a str,
    open_assign: &'a str,
    close_assign: &'a str,
}
