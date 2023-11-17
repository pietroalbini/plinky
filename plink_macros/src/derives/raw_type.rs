use crate::error::Error;
use crate::parser::{Parser, Struct, StructFields};
use proc_macro::{Span, TokenStream};

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_struct()?;

    let fields32 = prepare_field_list(&parsed, true)?;
    let fields64 = prepare_field_list(&parsed, false)?;

    let mut output = String::new();
    output.push_str(&format!("impl RawType for {} {{\n", parsed.name));
    fn_zero(&mut output, &fields32);
    fn_size(&mut output, &fields32);
    fn_read(&mut output, &fields32, &fields64);
    fn_write(&mut output, &fields32, &fields64);
    output.push_str("}\n");

    Ok(output.parse().unwrap())
}

fn fn_zero(output: &mut String, fields: &[Field<'_>]) {
    output.push_str("fn zero() -> Self {");
    output.push_str("Self {");
    for field in fields {
        output.push_str(&format!(
            "{}: <{} as {}>::zero(),",
            field.name, field.field_ty, field.trait_ty
        ));
    }
    output.push_str("}}");
}

fn fn_size(output: &mut String, fields: &[Field<'_>]) {
    output.push_str("fn size(class: ElfClass) -> usize {\n");
    output.push_str("0");
    for field in fields {
        output.push_str(&format!(
            " + <{} as {}>::size(class)",
            field.field_ty, field.trait_ty
        ));
    }
    output.push_str("\n}\n");
}

fn fn_read(output: &mut String, fields32: &[Field<'_>], fields64: &[Field<'_>]) {
    fn render(output: &mut String, fields: &[Field<'_>]) {
        output.push_str("Ok(Self {");
        for field in fields {
            output.push_str(&format!(
                "{}: <{} as {}>::read(cursor)?,",
                field.name, field.field_ty, field.trait_ty
            ));
        }
        output.push_str("})");
    }

    output.push_str("fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {");
    if fields32 != fields64 {
        output.push_str("match cursor.class {");
        for (class, fields) in [("Elf32", fields32), ("Elf64", fields64)] {
            output.push_str(&format!("Some(ElfClass::{class}) => {{"));
            render(output, fields);
            output.push_str("}");
        }
        output.push_str(r#"None => panic!("elf class not configured yet"),"#);
        output.push_str("}");
    } else {
        render(output, fields32);
    }
    output.push_str("}");
}

fn fn_write(output: &mut String, fields32: &[Field<'_>], fields64: &[Field<'_>]) {
    fn render(output: &mut String, fields: &[Field<'_>]) {
        for field in fields {
            output.push_str(&format!(
                "<{} as {}>::write(&self.{}, cursor)?;",
                field.field_ty, field.trait_ty, field.name
            ));
        }
    }

    output.push_str("fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError> {");
    if fields32 != fields64 {
        output.push_str("match cursor.class {");
        for (class, fields) in [("Elf32", fields32), ("Elf64", fields64)] {
            output.push_str(&format!("ElfClass::{class} => {{"));
            render(output, fields);
            output.push_str("}");
        }
        output.push_str("}");
    } else {
        render(output, fields32);
    }
    output.push_str("Ok(()) }");
}

fn prepare_field_list(parsed: &Struct, is_elf32: bool) -> Result<Vec<Field>, Error> {
    let mut fields: Vec<Field> = Vec::new();

    let parsed_fields = match &parsed.fields {
        StructFields::StructLike(struct_like) => struct_like,
        _ => return Err(Error::new("only struct-like fields are supported")),
    };

    for field in parsed_fields {
        let mut trait_ty = "RawType";
        let mut insert_at = fields.len();
        for attribute in &field.attrs {
            let (name, value) = match attribute.value.split_once('=') {
                Some((n, v)) => (n.trim(), Some(unquote(v.trim(), attribute.span)?)),
                None => (&*attribute.value, None),
            };
            match (name, value) {
                ("pointer_size", None) => trait_ty = "RawTypeAsPointerSize",
                ("placed_on_elf32_after", Some(after)) => {
                    if is_elf32 {
                        insert_at =
                            fields.iter().position(|i| i.name == after).ok_or_else(|| {
                                Error::new(format!("could not find field called {after}"))
                                    .span(attribute.span)
                            })? + 1;
                    }
                }
                ("placed_on_elf64_after", Some(after)) => {
                    if !is_elf32 {
                        insert_at =
                            fields.iter().position(|i| i.name == after).ok_or_else(|| {
                                Error::new(format!("could not find field called {after}"))
                                    .span(attribute.span)
                            })? + 1;
                    }
                }
                _ => return Err(Error::new("unknown attribute").span(attribute.span)),
            }
        }
        fields.insert(
            insert_at,
            Field {
                name: &field.name,
                field_ty: &field.ty,
                trait_ty,
            },
        );
    }

    Ok(fields)
}

fn unquote(input: &str, span: Span) -> Result<&str, Error> {
    input
        .strip_prefix('"')
        .and_then(|i| i.strip_suffix('"'))
        .ok_or_else(|| Error::new("attribute value must be quoted").span(span))
}

#[derive(PartialEq, Eq)]
struct Field<'a> {
    name: &'a str,
    field_ty: &'a str,
    trait_ty: &'static str,
}
