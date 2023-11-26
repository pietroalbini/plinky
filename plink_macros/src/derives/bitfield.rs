use crate::error::Error;
use crate::parser::{Item, Parser, Struct, StructFields};
use crate::utils::generate_impl_for;
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_struct()?;
    let mut output = String::new();

    generate_impl_for(
        &mut output,
        &Item::Struct(parsed.clone()),
        "plink_rawutils::bitfields::Bitfield",
        |output| {
            type_repr(output, &parsed)?;
            fn_read(output, &parsed);
            fn_write(output, &parsed);
            Ok(())
        },
    )?;

    Ok(output.parse().unwrap())
}

fn type_repr(output: &mut String, struct_: &Struct) -> Result<(), Error> {
    let mut repr = None;
    for attribute in &struct_.attrs {
        let parsed = attribute
            .value
            .strip_prefix("bitfield_repr(")
            .and_then(|s| s.strip_suffix(")"));
        match (parsed, &repr) {
            (None, _) => {}
            (Some(parsed), None) => repr = Some(parsed),
            (Some(_), Some(_)) => {
                return Err(Error::new("duplicate attribute").span(attribute.span));
            }
        }
    }
    let Some(repr) = repr else {
        return Err(Error::new("missing attribute bitfield_repr"));
    };

    output.push_str("type Repr = ");
    output.push_str(repr);
    output.push(';');

    Ok(())
}

fn fn_read(output: &mut String, struct_: &Struct) {
    output.push_str(
        "fn read(raw: Self::Repr) -> Result<Self, plink_rawutils::bitfields::BitfieldReadError> {",
    );
    output.push_str("let mut reader = plink_rawutils::bitfields::BitfieldReader::new(raw);");

    output.push_str("let result = Self ");
    match &struct_.fields {
        StructFields::None => {}
        StructFields::TupleLike(fields) => {
            output.push('(');
            for idx in 0..fields.len() {
                output.push_str(&format!("reader.bit({idx}),"));
            }
            output.push(')');
        }
        StructFields::StructLike(fields) => {
            output.push('{');
            for (idx, field) in fields.iter().enumerate() {
                output.push_str(&format!("{}: reader.bit({idx}),", field.name));
            }
            output.push('}');
        }
    }
    output.push_str(";");

    output.push_str("reader.check_for_unknown_bits()?;");
    output.push_str("Ok(result)");
    output.push_str("}");
}

fn fn_write(output: &mut String, struct_: &Struct) {
    output.push_str("fn write(&self) -> Self::Repr {");
    output.push_str("let mut writer = plink_rawutils::bitfields::BitfieldWriter::new();");

    match &struct_.fields {
        StructFields::None => {}
        StructFields::TupleLike(fields) => {
            for idx in 0..fields.len() {
                output.push_str(&format!("writer.set_bit({idx}, self.{idx});"));
            }
        }
        StructFields::StructLike(fields) => {
            for (idx, field) in fields.iter().enumerate() {
                output.push_str(&format!("writer.set_bit({idx}, self.{});", field.name));
            }
        }
    }

    output.push_str("writer.value()");
    output.push_str("}");
}
