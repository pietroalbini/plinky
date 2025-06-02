use crate::error::Error;
use crate::parser::{Attributes, Ident, Item, Parser, Struct, StructFields};
use crate::utils::{generate_impl_for, ident, literal};
use plinky_macros_quote::quote;
use plinky_utils::quote::Quote;
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_struct()?;

    let fields = generate_fields(&parsed)?;
    let mut impls = Vec::new();

    impls.push(generate_impl_for(
        &Item::Struct(parsed.clone()),
        Some("plinky_utils::bitfields::Bitfield"),
        quote! {
            #{ bitfield_type_repr(&parsed)? }
            #{ bitfield_fn_read(&fields) }
            #{ bitfield_fn_write(&fields) }
        },
    ));

    if let Some(attr) = parsed.attrs.get("bitfield_display_comma_separated")? {
        attr.must_be_empty()?;

        impls.push(generate_impl_for(
            &Item::Struct(parsed.clone()),
            Some("std::fmt::Display"),
            display_fn_fmt(&fields),
        ));
    };

    Ok(quote! { #impls })
}

fn bitfield_type_repr(struct_: &Struct) -> Result<TokenStream, Error> {
    let repr = if let Some(attr) = struct_.attrs.get("bitfield_repr")? {
        attr.get_parenthesis_one_expr()?
    } else {
        return Err(Error::new("missing attribute bitfield_repr"));
    };

    Ok(quote! { type Repr = #{ ident(repr) }; })
}

fn bitfield_fn_read(fields: &Fields) -> TokenStream {
    let result = match fields {
        Fields::None => quote!(Self),
        Fields::TupleLike(fields) => {
            let mut parts = Vec::new();
            for bit in fields {
                parts.push(quote!(reader.bit(#bit),));
            }
            quote!(Self(#parts))
        }
        Fields::StructLike(fields) => {
            let mut parts = Vec::new();
            for (name, bit) in fields {
                parts.push(quote!(#name: reader.bit(#bit),));
            }
            quote!(Self { #parts })
        }
    };

    quote! {
        fn read(raw: Self::Repr) -> Result<Self, plinky_utils::bitfields::BitfieldReadError> {
            let mut reader = plinky_utils::bitfields::BitfieldReader::new(raw);
            let result = #result;
            reader.check_for_unknown_bits()?;
            Ok(result)
        }
    }
}

fn bitfield_fn_write(fields: &Fields) -> TokenStream {
    let mut setters = Vec::new();
    match fields {
        Fields::None => {}
        Fields::TupleLike(fields) => {
            for (idx, bit) in fields.iter().enumerate() {
                setters.push(quote! { writer.set_bit(#bit, self.#{ literal(idx) }); });
            }
        }
        Fields::StructLike(fields) => {
            for (name, bit) in fields.iter() {
                setters.push(quote! { writer.set_bit(#bit, self.#name); });
            }
        }
    }

    quote! {
        fn write(&self) -> Self::Repr {
            let mut writer = plinky_utils::bitfields::BitfieldWriter::new();
            #setters
            writer.value()
        }
    }
}

fn display_fn_fmt(fields: &Fields) -> TokenStream {
    let mut writes = Vec::new();
    match fields {
        Fields::None => {}
        Fields::TupleLike(fields) => {
            for (idx, bit) in fields.iter().enumerate() {
                writes.push(quote! {
                    if self.#{ literal(idx) } {
                        if !first {
                            f.write_str(", ")?;
                        }
                        first = false;
                        f.write_str(stringify!(#bit))?;
                    }
                });
            }
        }
        Fields::StructLike(fields) => {
            for (name, _) in fields.iter() {
                writes.push(quote! {
                    if self.#name {
                        if !first {
                            f.write_str(", ")?;
                        }
                        first = false;
                        f.write_str(stringify!(#name))?;
                    }
                })
            }
        }
    }

    quote! {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            let mut first = true;
            #writes
            Ok(())
        }
    }
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
        Self { has_custom_bit: false }
    }

    fn index_of(&mut self, attrs: &Attributes, position: usize) -> Result<BitIndex, Error> {
        match self.find_bit_attribute(attrs)? {
            Some(bit) => {
                self.has_custom_bit = true;
                Ok(BitIndex(bit))
            }
            None => {
                if self.has_custom_bit {
                    return Err(Error::new("bit attribute required after other bit attributes"));
                }
                Ok(BitIndex(position))
            }
        }
    }

    fn find_bit_attribute(&self, attrs: &Attributes) -> Result<Option<usize>, Error> {
        if let Some(attr) = attrs.get("bit")? {
            Ok(Some(
                attr.get_parenthesis_one_expr()?
                    .parse()
                    .map_err(|_| Error::new("failed to parse bit").span(attr.span))?,
            ))
        } else {
            Ok(None)
        }
    }
}

enum Fields {
    None,
    TupleLike(Vec<BitIndex>),
    StructLike(Vec<(Ident, BitIndex)>),
}

#[derive(Clone)]
struct BitIndex(usize);

impl Quote for BitIndex {
    fn to_token_stream(&self) -> TokenStream {
        literal(self.0).into()
    }
}
