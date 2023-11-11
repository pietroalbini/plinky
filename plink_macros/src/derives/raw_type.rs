use crate::error::Error;
use crate::parser::{Parser, Struct};
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_struct()?;

    let mut output = String::new();
    output.push_str(&format!("impl RawType for {} {{\n", parsed.name));
    fn_size(&mut output, &parsed);
    fn_read(&mut output, &parsed);
    fn_write(&mut output, &parsed);
    output.push_str("}\n");

    Ok(output.parse().unwrap())
}

fn fn_size(output: &mut String, parsed: &Struct) {
    output.push_str("fn size() -> usize {\n");

    let mut first = true;
    for field in &parsed.fields {
        if first {
            first = false;
        } else {
            output.push_str(" + ");
        }
        output.push_str(&format!("<{} as RawType>::size()", field.ty));
    }
    output.push_str("\n");

    output.push_str("}\n");
}

fn fn_read(output: &mut String, parsed: &Struct) {
    output.push_str("fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {\n");
    output.push_str("    Ok(Self {\n");
    for field in &parsed.fields {
        let as_ty = if field.attrs.iter().any(|attr| attr == "pointer_size") {
            "RawTypeAsPointerSize"
        } else {
            "RawType"
        };
        output.push_str(&format!(
            "{}: <{} as {as_ty}>::read(cursor)?,",
            field.name, field.ty
        ));
    }
    output.push_str("    })\n");
    output.push_str("}\n");
}

fn fn_write(output: &mut String, parsed: &Struct) {
    output.push_str("fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError> {\n");
    for field in &parsed.fields {
        let as_ty = if field.attrs.iter().any(|attr| attr == "pointer_size") {
            "RawTypeAsPointerSize"
        } else {
            "RawType"
        };
        output.push_str(&format!(
            "<{} as {as_ty}>::write(&self.{}, cursor)?;",
            field.ty, field.name
        ));
    }
    output.push_str("    Ok(())\n");
    output.push_str("}\n");
}
