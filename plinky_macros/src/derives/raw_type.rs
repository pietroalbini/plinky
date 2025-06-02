use crate::error::Error;
use crate::parser::{Ident, Item, Parser, Struct, StructFields, Type};
use crate::utils::generate_impl_for;
use plinky_macros_quote::quote;
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let parsed = Parser::new(tokens).parse_struct()?;

    let fields32 = prepare_field_list(&parsed, true)?;
    let fields64 = prepare_field_list(&parsed, false)?;

    Ok(generate_impl_for(
        &Item::Struct(parsed.clone()),
        Some("plinky_utils::raw_types::RawType"),
        quote! {
            #{ fn_zero(&fields32) }
            #{ fn_size(&fields32) }
            #{ fn_read(&fields32, &fields64) }
            #{ fn_write(&fields32, &fields64) }
        },
    ))
}

fn fn_zero(fields: &[Field<'_>]) -> TokenStream {
    let mut initializers = Vec::new();
    for Field { name, field_ty, trait_ty } in fields {
        initializers.push(quote! { #name: <#field_ty as #trait_ty>::zero(), });
    }
    quote! {
        fn zero() -> Self {
            Self { #initializers }
        }
    }
}

fn fn_size(fields: &[Field<'_>]) -> TokenStream {
    let mut addends = Vec::new();
    for Field { field_ty, trait_ty, .. } in fields {
        addends.push(quote! { + <#field_ty as #trait_ty>::size(bits) });
    }

    quote! {
        fn size(bits: plinky_utils::Bits) -> usize {
            0 #addends
        }
    }
}

fn fn_read(fields32: &[Field<'_>], fields64: &[Field<'_>]) -> TokenStream {
    fn render(fields: &[Field<'_>]) -> TokenStream {
        let mut setters = Vec::new();
        for Field { name, field_ty, trait_ty } in fields {
            setters.push(quote! {
                #name: plinky_utils::raw_types::RawReadError::wrap_field::<Self, _>(
                    stringify!(#name),
                    <#field_ty as #trait_ty>::read(ctx, reader)
                )?,
            });
        }
        quote! {
            Ok(Self { #setters })
        }
    }

    quote! {
        fn read(
            ctx: plinky_utils::raw_types::RawTypeContext,
            reader: &mut dyn std::io::Read,
        ) -> Result<Self, plinky_utils::raw_types::RawReadError> {
            match ctx.bits {
                plinky_utils::Bits::Bits32 => #{ render(fields32) },
                plinky_utils::Bits::Bits64 => #{ render(fields64) },
            }
        }
    }
}

fn fn_write(fields32: &[Field<'_>], fields64: &[Field<'_>]) -> TokenStream {
    fn render(fields: &[Field<'_>]) -> TokenStream {
        let mut writes = Vec::new();
        for Field { name, field_ty, trait_ty } in fields {
            writes.push(quote! {
                plinky_utils::raw_types::RawWriteError::wrap_field::<Self, _>(
                    stringify!(#name),
                    <#field_ty as #trait_ty>::write(&self.#name, ctx, writer)
                )?;
            });
        }
        quote! { { #writes } }
    }

    quote! {
        fn write(
            &self,
            ctx: plinky_utils::raw_types::RawTypeContext,
            writer: &mut dyn std::io::Write,
        ) -> Result<(), plinky_utils::raw_types::RawWriteError> {
            match ctx.bits {
                plinky_utils::Bits::Bits32 => #{ render(fields32) },
                plinky_utils::Bits::Bits64 => #{ render(fields64) },
            }
            Ok(())
        }
    }
}

fn prepare_field_list(parsed: &Struct, is_elf32: bool) -> Result<Vec<Field>, Error> {
    let trait_ty_base = Type("plinky_utils::raw_types::RawType".parse().unwrap());
    let trait_ty_pointers = Type("plinky_utils::raw_types::RawTypeAsPointerSize".parse().unwrap());

    let mut fields: Vec<Field> = Vec::new();

    let parsed_fields = match &parsed.fields {
        StructFields::StructLike(struct_like) => struct_like,
        _ => return Err(Error::new("only struct-like fields are supported")),
    };

    for field in parsed_fields {
        let mut trait_ty = &trait_ty_base;
        let mut insert_at = fields.len();

        if let Some(attr) = field.attrs.get("pointer_size")? {
            attr.must_be_empty()?;
            trait_ty = &trait_ty_pointers;
        }
        if let Some(attr) = field.attrs.get("placed_on_elf32_after")? {
            let after = attr.get_equals_to_str()?;
            if is_elf32 {
                insert_at = fields.iter().position(|i| i.name.name == after).ok_or_else(|| {
                    Error::new(format!("could not find field called {after}")).span(attr.span)
                })? + 1;
            }
        }
        if let Some(attr) = field.attrs.get("placed_on_elf64_after")? {
            let after = attr.get_equals_to_str()?;
            if !is_elf32 {
                insert_at = fields.iter().position(|i| i.name.name == after).ok_or_else(|| {
                    Error::new(format!("could not find field called {after}")).span(attr.span)
                })? + 1;
            }
        }

        fields.insert(
            insert_at,
            Field { name: &field.name, field_ty: &field.ty, trait_ty: trait_ty.clone() },
        );
    }

    Ok(fields)
}

struct Field<'a> {
    name: &'a Ident,
    field_ty: &'a Type,
    trait_ty: Type,
}
