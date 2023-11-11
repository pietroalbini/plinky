use crate::error::Error;
use crate::parser::{Attribute, EnumVariantData, Item, Parser, StructFields};
use proc_macro::{Span, TokenStream};

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let item = Parser::new(tokens).parse_item()?;

    let mut output = String::new();
    generate_error_impl(&mut output, &item)?;
    generate_from_impls(&mut output, &item)?;

    Ok(output.parse().unwrap())
}

fn generate_error_impl(output: &mut String, item: &Item) -> Result<(), Error> {
    output.push_str(&format!("impl std::error::Error for {} {{", item.name()));

    output.push_str("fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {");
    match item {
        Item::Struct(struct_) => {
            let mut source = None;
            match &struct_.fields {
                StructFields::None => {}
                StructFields::TupleLike(fields) => {
                    for (idx, field) in fields.iter().enumerate() {
                        maybe_set_source(&mut source, &idx.to_string(), &field.attrs)?;
                    }
                }
                StructFields::StructLike(fields) => {
                    for field in fields {
                        maybe_set_source(&mut source, &field.name, &field.attrs)?;
                    }
                }
            }
            match source {
                Some(source) => output.push_str(&format!("Some(&self.{source})")),
                None => output.push_str("None"),
            }
        }
        Item::Enum(enum_) => {
            output.push_str("match self {");
            for variant in &enum_.variants {
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
        }
    }
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
            Err(Error::new("multiple sources for a single error").span(attr.span))
        } else {
            *source = Some(name.into());
            Ok(())
        }
    } else {
        Ok(())
    }
}

fn generate_from_impls(output: &mut String, item: &Item) -> Result<(), Error> {
    match item {
        Item::Struct(struct_) => {
            let fields = match &struct_.fields {
                StructFields::None => return Ok(()),
                StructFields::TupleLike(fields) => fields
                    .iter()
                    .map(|f| FromImplField {
                        attrs: &f.attrs,
                        field: "value",
                        ty: &f.ty,
                        open_assign: "(",
                        close_assign: ")",
                    })
                    .collect::<Vec<_>>(),
                StructFields::StructLike(fields) => fields
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

            generate_from_impl(output, &struct_.name, &struct_.name, struct_.span, &fields)?;
        }
        Item::Enum(enum_) => {
            for variant in &enum_.variants {
                let fields = match &variant.data {
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

                generate_from_impl(
                    output,
                    &enum_.name,
                    &format!("{}::{}", enum_.name, variant.name),
                    variant.span,
                    &fields,
                )?;
            }
        }
    }
    Ok(())
}

fn generate_from_impl(
    output: &mut String,
    item_name: &str,
    constructor: &str,
    span: Span,
    fields: &[FromImplField<'_>],
) -> Result<(), Error> {
    let has_from = fields
        .iter()
        .any(|f| f.attrs.iter().any(|a| a.value == "from"));
    if !has_from {
        return Ok(());
    } else if fields.len() > 1 {
        return Err(Error::new("#[from] in error with multiple fields").span(span));
    }
    let field = fields.last().unwrap();

    output.push_str(&format!("impl From<{}> for {item_name} {{", field.ty));
    output.push_str(&format!(
        "fn from({}: {}) -> Self {{",
        field.field, field.ty
    ));
    output.push_str(&format!(
        "{constructor}{}{}{}",
        field.open_assign, field.field, field.close_assign
    ));
    output.push_str("}}");
    Ok(())
}

struct FromImplField<'a> {
    attrs: &'a [Attribute],
    field: &'a str,
    ty: &'a str,
    open_assign: &'a str,
    close_assign: &'a str,
}
