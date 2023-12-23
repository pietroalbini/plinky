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
        StructFields::TupleLike(fields) => {
            (0..fields.len()).map(|idx| (format!("f{idx}"), format!("&self.{idx}"))).collect()
        }
        StructFields::StructLike(fields) => {
            fields.iter().map(|f| (f.name.clone(), format!("&self.{}", f.name))).collect()
        }
    };

    generate_impl_for(output, item, "std::fmt::Display", |output| {
        output.push_str("fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {");

        for (name, value) in &args {
            output.push_str(&format!("let {name} = {value};"));
        }
        generate_write(output, &struct_.attrs, struct_.span)?;

        output.push('}');
        Ok(())
    })
}

fn generate_enum_impl(output: &mut String, item: &Item, enum_: &Enum) -> Result<(), Error> {
    let mut match_arms = Vec::new();
    for variant in &enum_.variants {
        let name = &variant.name;
        match &variant.data {
            EnumVariantData::None => {
                match_arms.push((name.to_string(), &variant.attrs, variant.span))
            }
            EnumVariantData::TupleLike(fields) => {
                let fields =
                    (0..fields.len()).map(|idx| format!("f{idx}")).collect::<Vec<_>>().join(", ");
                match_arms.push((format!("{name}({fields})"), &variant.attrs, variant.span));
            }
            EnumVariantData::StructLike(fields) => {
                let fields = fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
                match_arms.push((format!("{name} {{ {fields} }}"), &variant.attrs, variant.span));
            }
        }
    }

    generate_impl_for(output, item, "std::fmt::Display", |output| {
        output.push_str("fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {");
        output.push_str("match self {");
        for (variant, attrs, span) in match_arms {
            output.push_str(&enum_.name);
            output.push_str("::");
            output.push_str(&variant);
            output.push_str(" => ");
            generate_write(output, attrs, span)?;
            output.push(',');
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
