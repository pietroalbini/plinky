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
                            if fields.len() == 1 {
                                maybe_set_source_container(
                                    &mut source,
                                    &idx.to_string(),
                                    &field.ty,
                                    &struct_.attrs,
                                )?;
                            }
                            maybe_set_source_field(&mut source, &idx.to_string(), &field.attrs)?;
                        }
                    }
                    StructFields::StructLike(fields) => {
                        for field in fields {
                            if fields.len() == 1 {
                                maybe_set_source_container(
                                    &mut source,
                                    &field.name,
                                    &field.ty,
                                    &struct_.attrs,
                                )?;
                            }
                            maybe_set_source_field(&mut source, &field.name, &field.attrs)?;
                        }
                    }
                }
                match source {
                    Some(SourceKind::Field(source)) => {
                        output.push_str(&format!("Some(&self.{source})"));
                    }
                    Some(SourceKind::Transparent { ty, field }) => {
                        output.push_str(&format!(
                            "<{ty} as std::error::Error>::source(&self.{field})"
                        ));
                    }
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
                                if fields.len() == 1 {
                                    maybe_set_source_container(
                                        &mut source,
                                        &name,
                                        &field.ty,
                                        &variant.attrs,
                                    )?;
                                }
                                maybe_set_source_field(&mut source, &name, &field.attrs)?;
                            }
                            output.push(')');
                        }
                        EnumVariantData::StructLike(fields) => {
                            output.push('{');
                            for field in fields {
                                output.push_str(&field.name);
                                output.push(',');
                                if fields.len() == 1 {
                                    maybe_set_source_container(
                                        &mut source,
                                        &field.name,
                                        &field.ty,
                                        &variant.attrs,
                                    )?;
                                }
                                maybe_set_source_field(&mut source, &field.name, &field.attrs)?;
                            }
                            output.push('}');
                        }
                    }
                    output.push_str(" => ");
                    match source {
                        Some(SourceKind::Field(source)) => {
                            output.push_str(&format!("Some({source})"));
                        }
                        Some(SourceKind::Transparent { ty, field }) => {
                            output
                                .push_str(&format!("<{ty} as std::error::Error>::source({field})"));
                        }
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

fn maybe_set_source_field(
    source: &mut Option<SourceKind>,
    name: &str,
    attrs: &Attributes,
) -> Result<(), Error> {
    maybe_set_source_inner(source, SourceKind::Field(name.into()), attrs, &["from", "source"])
}

fn maybe_set_source_container(
    source: &mut Option<SourceKind>,
    name: &str,
    ty: &str,
    attrs: &Attributes,
) -> Result<(), Error> {
    maybe_set_source_inner(
        source,
        SourceKind::Transparent { ty: ty.into(), field: name.into() },
        attrs,
        &["transparent"],
    )
}

fn maybe_set_source_inner(
    source: &mut Option<SourceKind>,
    new: SourceKind,
    attrs: &Attributes,
    possible_attrs: &[&str],
) -> Result<(), Error> {
    for attr_name in possible_attrs {
        if let Some(attr) = attrs.get(attr_name)? {
            attr.must_be_empty()?;
            if source.is_some() {
                return Err(Error::new("multiple sources for a single error").span(attr.span));
            } else {
                *source = Some(new.clone());
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
enum SourceKind {
    Field(String),
    Transparent { ty: String, field: String },
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

            generate_from_impl(output, item, &struct_.name, struct_.span, &struct_.attrs, &fields)?;
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
                    &variant.attrs,
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
    container_attrs: &Attributes,
    fields: &[FromImplField<'_>],
) -> Result<(), Error> {
    let field = if let [field] = fields {
        field
    } else {
        if let Some(attr) = container_attrs.get("transparent")? {
            return Err(Error::new("#[transparent] in error with multiple fields").span(attr.span));
        }
        for field in fields {
            if field.attrs.get("from")?.is_some() {
                return Err(Error::new("#[from] in error with multiple fields").span(span));
            }
        }
        return Ok(());
    };

    let from = if let Some(attr) = field.attrs.get("from")? {
        attr.must_be_empty()?;
        true
    } else {
        false
    };
    let transparent = if let Some(attr) = container_attrs.get("transparent")? {
        attr.must_be_empty()?;
        true
    } else {
        false
    };
    match (from, transparent) {
        (true, false) | (false, true) => {}
        (false, false) => return Ok(()),
        (true, true) => {
            return Err(Error::new("both #[transparent] and #[from] present").span(span));
        }
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
