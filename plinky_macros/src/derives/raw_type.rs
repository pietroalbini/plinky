use crate::error::Error;
use crate::parser::{Item, Parser, Struct, StructFields};
use crate::utils::generate_impl_for;
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_struct()?;

    let fields32 = prepare_field_list(&parsed, true)?;
    let fields64 = prepare_field_list(&parsed, false)?;

    let mut output = String::new();
    generate_impl_for(
        &mut output,
        &Item::Struct(parsed.clone()),
        "plinky_utils::raw_types::RawType",
        |output| {
            fn_zero(output, &fields32);
            fn_size(output, &fields32);
            fn_read(output, &fields32, &fields64);
            fn_write(output, &fields32, &fields64);
        },
    );

    Ok(output.parse().unwrap())
}

fn fn_zero(output: &mut String, fields: &[Field<'_>]) {
    output.push_str("fn zero() -> Self {");
    output.push_str("Self {");
    for field in fields {
        output.push_str(&format!(
            "{}: <{} as plinky_utils::raw_types::{}>::zero(),",
            field.name, field.field_ty, field.trait_ty
        ));
    }
    output.push_str("}}");
}

fn fn_size(output: &mut String, fields: &[Field<'_>]) {
    output.push_str("fn size(bits: impl Into<plinky_utils::Bits>) -> usize {\n");
    output.push_str("let bits = bits.into();");
    output.push('0');
    for field in fields {
        output.push_str(&format!(
            " + <{} as plinky_utils::raw_types::{}>::size(bits)",
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
                "{}: plinky_utils::raw_types::RawReadError::wrap_field::<Self, _>(stringify!({}), <{} as plinky_utils::raw_types::{}>::read(bits, endian, reader))?,",
                field.name, field.name, field.field_ty, field.trait_ty
            ));
        }
        output.push_str("})");
    }

    output.push_str("fn read(bits: impl Into<plinky_utils::Bits>, endian: impl Into<plinky_utils::Endian>, reader: &mut dyn std::io::Read) -> Result<Self, plinky_utils::raw_types::RawReadError> {");
    output.push_str("let bits = bits.into();");
    output.push_str("let endian = endian.into();");
    if fields32 != fields64 {
        output.push_str("match bits {");
        for (bits, fields) in [("Bits32", fields32), ("Bits64", fields64)] {
            output.push_str(&format!("plinky_utils::Bits::{bits} => {{"));
            render(output, fields);
            output.push('}');
        }
        output.push('}');
    } else {
        render(output, fields32);
    }
    output.push('}');
}

fn fn_write(output: &mut String, fields32: &[Field<'_>], fields64: &[Field<'_>]) {
    fn render(output: &mut String, fields: &[Field<'_>]) {
        for field in fields {
            output.push_str(&format!(
                "plinky_utils::raw_types::RawWriteError::wrap_field::<Self, _>(stringify!({}), <{} as plinky_utils::raw_types::{}>::write(&self.{}, bits, endian, writer))?;",
                field.name, field.field_ty, field.trait_ty, field.name
            ));
        }
    }

    output.push_str(
        "fn write(&self, bits: impl Into<plinky_utils::Bits>, endian: impl Into<plinky_utils::Endian>, writer: &mut dyn std::io::Write) -> Result<(), plinky_utils::raw_types::RawWriteError> {",
    );
    output.push_str("let bits = bits.into();");
    output.push_str("let endian = endian.into();");
    if fields32 != fields64 {
        output.push_str("match bits {");
        for (bits, fields) in [("Bits32", fields32), ("Bits64", fields64)] {
            output.push_str(&format!("plinky_utils::Bits::{bits} => {{"));
            render(output, fields);
            output.push('}');
        }
        output.push('}');
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

        if let Some(attr) = field.attrs.get("pointer_size")? {
            attr.must_be_empty()?;
            trait_ty = "RawTypeAsPointerSize";
        }
        if let Some(attr) = field.attrs.get("placed_on_elf32_after")? {
            let after = attr.get_equals_to_str()?;
            if is_elf32 {
                insert_at = fields.iter().position(|i| i.name == after).ok_or_else(|| {
                    Error::new(format!("could not find field called {after}")).span(attr.span)
                })? + 1;
            }
        }
        if let Some(attr) = field.attrs.get("placed_on_elf64_after")? {
            let after = attr.get_equals_to_str()?;
            if !is_elf32 {
                insert_at = fields.iter().position(|i| i.name == after).ok_or_else(|| {
                    Error::new(format!("could not find field called {after}")).span(attr.span)
                })? + 1;
            }
        }

        fields.insert(insert_at, Field { name: &field.name, field_ty: &field.ty, trait_ty });
    }

    Ok(fields)
}

#[derive(PartialEq, Eq)]
struct Field<'a> {
    name: &'a str,
    field_ty: &'a str,
    trait_ty: &'static str,
}
