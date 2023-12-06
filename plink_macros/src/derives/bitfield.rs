use crate::error::Error;
use crate::parser::{Attribute, Item, Parser, Struct, StructFields};
use crate::utils::generate_impl_for;
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_struct()?;
    let mut output = String::new();

    let fields = generate_fields(&parsed)?;

    generate_impl_for(
        &mut output,
        &Item::Struct(parsed.clone()),
        "plink_rawutils::bitfields::Bitfield",
        |output| {
            type_repr(output, &parsed)?;
            fn_read(output, &fields);
            fn_write(output, &fields);
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

fn fn_read(output: &mut String, fields: &Fields) {
    output.push_str(
        "fn read(raw: Self::Repr) -> Result<Self, plink_rawutils::bitfields::BitfieldReadError> {",
    );
    output.push_str("let mut reader = plink_rawutils::bitfields::BitfieldReader::new(raw);");

    output.push_str("let result = Self ");
    match fields {
        Fields::None => {}
        Fields::TupleLike(fields) => {
            output.push('(');
            for bit in fields {
                output.push_str(&format!("reader.bit({}),", bit.0));
            }
            output.push(')');
        }
        Fields::StructLike(fields) => {
            output.push('{');
            for (name, bit) in fields {
                output.push_str(&format!("{}: reader.bit({}),", name, bit.0));
            }
            output.push('}');
        }
    }
    output.push_str(";");

    output.push_str("reader.check_for_unknown_bits()?;");
    output.push_str("Ok(result)");
    output.push_str("}");
}

fn fn_write(output: &mut String, fields: &Fields) {
    output.push_str("fn write(&self) -> Self::Repr {");
    output.push_str("let mut writer = plink_rawutils::bitfields::BitfieldWriter::new();");

    match fields {
        Fields::None => {}
        Fields::TupleLike(fields) => {
            for (idx, bit) in fields.iter().enumerate() {
                output.push_str(&format!("writer.set_bit({}, self.{});", bit.0, idx));
            }
        }
        Fields::StructLike(fields) => {
            for (name, bit) in fields.iter() {
                output.push_str(&format!("writer.set_bit({}, self.{});", bit.0, name));
            }
        }
    }

    output.push_str("writer.value()");
    output.push_str("}");
}

fn generate_fields(struct_: &Struct) -> Result<Fields, Error> {
    let mut calculator = BitCalculator::new();

    Ok(match &struct_.fields {
        StructFields::None => Fields::None,
        StructFields::TupleLike(fields) => Fields::TupleLike(
            fields
                .iter()
                .enumerate()
                .map(|(idx, field)| calculator.index_of(&field.attrs, idx))
                .collect::<Result<_, _>>()?,
        ),
        StructFields::StructLike(fields) => Fields::StructLike(
            fields
                .iter()
                .enumerate()
                .map(|(idx, field)| {
                    Ok((field.name.clone(), calculator.index_of(&field.attrs, idx)?))
                })
                .collect::<Result<_, _>>()?,
        ),
    })
}

struct BitCalculator {
    has_custom_bit: bool,
}

impl BitCalculator {
    fn new() -> Self {
        Self {
            has_custom_bit: false,
        }
    }

    fn index_of(&mut self, attrs: &[Attribute], position: usize) -> Result<BitIndex, Error> {
        match self.find_bit_attribute(attrs)? {
            Some(bit) => {
                self.has_custom_bit = true;
                Ok(BitIndex(bit))
            }
            None => {
                if self.has_custom_bit {
                    return Err(Error::new(
                        "bit attribute required after other bit attributes",
                    ));
                }
                Ok(BitIndex(position))
            }
        }
    }

    fn find_bit_attribute(&self, attrs: &[Attribute]) -> Result<Option<usize>, Error> {
        let mut found = None;
        for attr in attrs {
            match (&found, self.parse_bit_attribute(attr)?) {
                (None, Some(value)) => found = Some(value),
                (Some(_), Some(_)) => {
                    return Err(Error::new("duplicate bit attribute").span(attr.span))
                }
                _ => {}
            }
        }
        Ok(found)
    }

    fn parse_bit_attribute(&self, attr: &Attribute) -> Result<Option<usize>, Error> {
        attr.value
            .strip_prefix("bit(")
            .and_then(|s| s.strip_suffix(")"))
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| Error::new("failed to parse bit").span(attr.span))
            })
            .transpose()
    }
}

enum Fields {
    None,
    TupleLike(Vec<BitIndex>),
    StructLike(Vec<(String, BitIndex)>),
}

struct BitIndex(usize);
