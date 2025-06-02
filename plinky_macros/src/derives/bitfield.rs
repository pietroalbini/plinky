use crate::error::Error;
use crate::parser::{Attributes, Ident, Item, Parser, Struct, StructFields};
use crate::utils::{generate_impl_for, ident, literal};
use plinky_macros_quote::quote;
use plinky_utils::quote::Quote;
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_struct()?;

    let rwfields = generate_rwfields(&parsed)?;
    let fields = generate_fields(&parsed);
    let mut impls = Vec::new();

    impls.push(generate_impl_for(
        &Item::Struct(parsed.clone()),
        Some("plinky_utils::bitfields::Bitfield"),
        quote! {
            #{ bitfield_type_repr(&parsed)? }
            #{ bitfield_fn_read(&rwfields) }
            #{ bitfield_fn_write(&rwfields) }
            #{ bitfield_fn_empty(&fields) }
            #{ bitfield_fn_is_empty(&fields)}
            #{ bitfield_fn_binop(&fields, quote!(or), quote!(||))}
            #{ bitfield_fn_binop(&fields, quote!(and), quote!(&&))}
        },
    ));

    if let Some(attr) = parsed.attrs.get("bitfield_display_comma_separated")? {
        attr.must_be_empty()?;

        impls.push(generate_impl_for(
            &Item::Struct(parsed.clone()),
            Some("std::fmt::Display"),
            display_fn_fmt(&rwfields),
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

fn bitfield_fn_read(fields: &RWFields) -> TokenStream {
    let result = match fields {
        RWFields::None => quote!(Self),
        RWFields::TupleLike(fields) => {
            let mut parts = Vec::new();
            for bit in fields {
                parts.push(quote!(reader.bit(#bit),));
            }
            quote!(Self(#parts))
        }
        RWFields::StructLike(fields) => {
            let mut parts = Vec::new();
            for (name, bit) in fields {
                parts.push(quote!(#name: reader.bit(#bit),));
            }
            quote!(Self { #parts })
        }
    };

    quote! {
        fn read(raw: Self::Repr, ctx: plinky_utils::bitfields::BitfieldContext) -> Result<Self, plinky_utils::bitfields::BitfieldReadError> {
            let mut reader = plinky_utils::bitfields::BitfieldReader::new(raw);
            let result = #result;
            reader.check_for_unknown_bits()?;
            Ok(result)
        }
    }
}

fn bitfield_fn_write(fields: &RWFields) -> TokenStream {
    let mut setters = Vec::new();
    match fields {
        RWFields::None => {}
        RWFields::TupleLike(fields) => {
            for (idx, bit) in fields.iter().enumerate() {
                setters.push(quote! { writer.set_bit(#bit, self.#{ literal(idx) }); });
            }
        }
        RWFields::StructLike(fields) => {
            for (name, bit) in fields.iter() {
                setters.push(quote! { writer.set_bit(#bit, self.#name); });
            }
        }
    }

    quote! {
        fn write(&self, ctx: plinky_utils::bitfields::BitfieldContext) -> Self::Repr {
            let mut writer = plinky_utils::bitfields::BitfieldWriter::new();
            #setters
            writer.value()
        }
    }
}

fn bitfield_fn_empty(fields: &Fields) -> TokenStream {
    quote! {
        fn empty() -> Self {
            #{setter(fields, |_| quote!(false))}
        }
    }
}

fn bitfield_fn_is_empty(fields: &Fields) -> TokenStream {
    let mut comparisons = Vec::new();
    match fields {
        Fields::None => {}
        Fields::TupleLike(items) | Fields::StructLike(items) => {
            for item in items {
                comparisons.push(quote!(&& !self.#item));
            }
        }
    }

    quote! {
        fn is_empty(&self) -> bool {
            true #comparisons
        }
    }
}

fn bitfield_fn_binop(fields: &Fields, method: TokenStream, op: TokenStream) -> TokenStream {
    quote! {
        fn #method(&self, other: &Self) -> Self {
            #{ setter(fields, |name| quote!(self.#name #op other.#name)) }
        }
    }
}

fn display_fn_fmt(fields: &RWFields) -> TokenStream {
    let mut writes = Vec::new();
    match fields {
        RWFields::None => {}
        RWFields::TupleLike(fields) => {
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
        RWFields::StructLike(fields) => {
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

fn generate_rwfields(struct_: &Struct) -> Result<RWFields, Error> {
    let mut calculator = BitCalculator::new();

    Ok(match &struct_.fields {
        StructFields::None => RWFields::None,
        StructFields::TupleLike(fields) => RWFields::TupleLike(
            fields
                .iter()
                .enumerate()
                .map(|(idx, field)| calculator.index_of(&field.attrs, idx))
                .collect::<Result<_, _>>()?,
        ),
        StructFields::StructLike(fields) => RWFields::StructLike(
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

enum RWFields {
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

fn generate_fields(struct_: &Struct) -> Fields {
    match &struct_.fields {
        StructFields::None => Fields::None,
        StructFields::TupleLike(fields) => {
            Fields::TupleLike((0..fields.len()).map(|idx| quote!(#{ literal(idx) })).collect())
        }
        StructFields::StructLike(fields) => {
            Fields::StructLike(fields.iter().map(|f| quote!(#{&f.name})).collect())
        }
    }
}

enum Fields {
    None,
    TupleLike(Vec<TokenStream>),
    StructLike(Vec<TokenStream>),
}

fn setter<F>(fields: &Fields, f: F) -> TokenStream
where
    F: Fn(&TokenStream) -> TokenStream,
{
    let value = match fields {
        Fields::None => quote!(),
        Fields::TupleLike(items) => {
            let mut parts = Vec::new();
            for item in items {
                parts.push(quote!(#{f(item)},));
            }
            quote!((#parts))
        }
        Fields::StructLike(items) => {
            let mut parts = Vec::new();
            for item in items {
                parts.push(quote!(#item: #{f(item)},));
            }
            quote!({#parts})
        }
    };
    quote!(Self #value)
}
