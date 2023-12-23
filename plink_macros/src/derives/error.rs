use crate::error::Error;
use crate::parser::{Attributes, EnumVariantData, Item, Parser, StructFields};
use crate::utils::generate_impl_for;
use proc_macro::{Span, TokenStream};

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let item = Parser::new(tokens).parse_item()?;

    let mut output = String::new();
    generate_error_impl(&mut output, &item)?;
    generate_from_impls(&mut output, &item)?;

    Ok(output.parse().unwrap())
}

fn generate_error_impl(output: &mut String, item: &Item) -> Result<(), Error> {
    generate_impl_for(output, item, "std::error::Error", |output| {
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
                            output.push('(');
                            for (idx, field) in fields.iter().enumerate() {
                                let name = format!("field{}", idx);
                                output.push_str(&name);
                                output.push(',');
                                maybe_set_source(&mut source, &name, &field.attrs)?;
                            }
                            output.push(')');
                        }
                        EnumVariantData::StructLike(fields) => {
                            output.push('{');
                            for field in fields {
                                output.push_str(&field.name);
                                output.push(',');
                                maybe_set_source(&mut source, &field.name, &field.attrs)?;
                            }
                            output.push('}');
                        }
                    }
                    output.push_str(" => ");
                    match source {
                        Some(source) => output.push_str(&format!("Some({source})")),
                        None => output.push_str("None"),
                    }
                    output.push(',');
                }
                output.push('}');
            }
        }
        output.push('}');
        Ok(())
    })
}

fn maybe_set_source(
    source: &mut Option<String>,
    name: &str,
    attrs: &Attributes,
) -> Result<(), Error> {
    for attr_name in ["from", "source"] {
        if let Some(attr) = attrs.get(attr_name)? {
            attr.must_be_empty()?;
            if source.is_some() {
                return Err(Error::new("multiple sources for a single error").span(attr.span));
            } else {
                *source = Some(name.into());
            }
        }
    }
    Ok(())
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

            generate_from_impl(output, item, &struct_.name, struct_.span, &fields)?;
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
                    item,
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
    item: &Item,
    constructor: &str,
    span: Span,
    fields: &[FromImplField<'_>],
) -> Result<(), Error> {
    let field = if let [field] = fields {
        field
    } else {
        for field in fields {
            if field.attrs.get("from")?.is_some() {
                return Err(Error::new("#[from] in error with multiple fields").span(span));
            }
        }
        return Ok(());
    };
    if let Some(attr) = field.attrs.get("from")? {
        attr.must_be_empty()?;
    } else {
        return Ok(());
    }

    generate_impl_for(output, item, &format!("From<{}>", field.ty), |output| {
        output.push_str(&format!("fn from({}: {}) -> Self {{", field.field, field.ty));
        output.push_str(&format!(
            "{constructor}{}{}{}",
            field.open_assign, field.field, field.close_assign
        ));
        output.push('}');
        Ok(())
    })
}

struct FromImplField<'a> {
    attrs: &'a Attributes,
    field: &'a str,
    ty: &'a str,
    open_assign: &'a str,
    close_assign: &'a str,
}
