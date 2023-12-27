use crate::error::Error;
use crate::parser::{Attributes, Enum, EnumVariantData, Item, Parser, Struct, StructFields};
use crate::utils::generate_impl_for;
use proc_macro::{Span, TokenStream};

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let item = Parser::new(tokens).parse_item()?;
    let mut output = String::new();

    match &item {
        Item::Struct(struct_) => generate_struct_impl(&mut output, &item, struct_)?,
        Item::Enum(enum_) => generate_enum_impl(&mut output, &item, enum_)?,
    }

    Ok(output.parse().unwrap())
}

fn generate_struct_impl(output: &mut String, item: &Item, struct_: &Struct) -> Result<(), Error> {
    let args = match &struct_.fields {
        StructFields::None => Vec::new(),
        StructFields::TupleLike(fields) => fields
            .iter()
            .enumerate()
            .map(|(idx, f)| (format!("f{idx}"), format!("&self.{idx}"), &f.ty))
            .collect(),
        StructFields::StructLike(fields) => {
            fields.iter().map(|f| (f.name.clone(), format!("&self.{}", f.name), &f.ty)).collect()
        }
    };

    generate_impl_for(output, item, "std::fmt::Display", |output| {
        output.push_str("fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {");

        if let Some(attr) = struct_.attrs.get("transparent")? {
            attr.must_be_empty()?;
            match args.as_slice() {
                [(_name, value, ty)] => {
                    output.push_str(&format!("<{ty} as std::fmt::Display>::fmt({value}, f)"));
                }
                _ => {
                    return Err(
                        Error::new("#[transparent] structs must have one field").span(attr.span)
                    );
                }
            }
        } else {
            for (name, value, _ty) in &args {
                output.push_str(&format!("let {name} = {value};"));
            }
            generate_write(output, &struct_.attrs, struct_.span)?;
        }

        output.push('}');
        Ok(())
    })
}

fn generate_enum_impl(output: &mut String, item: &Item, enum_: &Enum) -> Result<(), Error> {
    enum MatchArm<'a> {
        Variant { variant: String, attrs: &'a Attributes, span: Span },
        Transparent { variant: String, ty: &'a str, field: &'a str },
    }

    let mut match_arms = Vec::new();
    for variant in &enum_.variants {
        let name = &variant.name;
        match &variant.data {
            EnumVariantData::None => {
                if let Some(attr) = variant.attrs.get("transparent")? {
                    attr.must_be_empty()?;
                    return Err(
                        Error::new("#[transparent] on a variant with no fields").span(attr.span)
                    );
                }
                match_arms.push(MatchArm::Variant {
                    variant: name.into(),
                    attrs: &variant.attrs,
                    span: variant.span,
                });
            }
            EnumVariantData::TupleLike(fields) => {
                if let Some(attr) = variant.attrs.get("transparent")? {
                    attr.must_be_empty()?;
                    match fields.as_slice() {
                        [field] => {
                            match_arms.push(MatchArm::Transparent {
                                variant: format!("{name}(field)"),
                                ty: &field.ty,
                                field: "field",
                            });
                        }
                        _ => {
                            return Err(Error::new(
                                "#[transparent] only supported on variants with one field",
                            )
                            .span(attr.span))
                        }
                    }
                } else {
                    let fields = (0..fields.len())
                        .map(|idx| format!("f{idx}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    match_arms.push(MatchArm::Variant {
                        variant: format!("{name}({fields})"),
                        attrs: &variant.attrs,
                        span: variant.span,
                    });
                }
            }
            EnumVariantData::StructLike(fields) => {
                if let Some(attr) = variant.attrs.get("transparent")? {
                    attr.must_be_empty()?;
                    match fields.as_slice() {
                        [field] => {
                            match_arms.push(MatchArm::Transparent {
                                variant: format!("{name} {{ {} }}", field.name),
                                ty: &field.ty,
                                field: &field.name,
                            });
                        }
                        _ => {
                            return Err(Error::new(
                                "#[transparent] only supported on variants with one field",
                            )
                            .span(attr.span))
                        }
                    }
                } else {
                    let fields =
                        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
                    match_arms.push(MatchArm::Variant {
                        variant: format!("{name} {{ {fields} }}"),
                        attrs: &variant.attrs,
                        span: variant.span,
                    });
                }
            }
        }
    }

    generate_impl_for(output, item, "std::fmt::Display", |output| {
        output.push_str("fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {");
        output.push_str("match self {");
        for match_arm in match_arms {
            match match_arm {
                MatchArm::Variant { variant, attrs, span } => {
                    output.push_str(&enum_.name);
                    output.push_str("::");
                    output.push_str(&variant);
                    output.push_str(" => ");
                    generate_write(output, attrs, span)?;
                    output.push(',');
                }
                MatchArm::Transparent { variant, ty, field } => {
                    output.push_str(&enum_.name);
                    output.push_str("::");
                    output.push_str(&variant);
                    output.push_str(" => ");
                    output.push_str(&format!("<{ty} as std::fmt::Display>::fmt({field}, f)"));
                    output.push(',');
                }
            }
        }
        output.push_str("}}");

        Ok(())
    })
}

fn generate_write(output: &mut String, attrs: &Attributes, span: Span) -> Result<(), Error> {
    let format_str = if let Some(attr) = attrs.get("display")? {
        attr.get_parenthesis_one_str()?
    } else {
        return Err(Error::new("missing #[display] attribute").span(span));
    };

    output.push_str(&format!("write!(f, \"{format_str}\")"));
    Ok(())
}
