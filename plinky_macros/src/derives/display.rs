use crate::error::Error;
use crate::parser::{Item, Parser};
use crate::utils::{UnifiedField, generate_for_each_variant, generate_impl_for, literal};
use plinky_macros_quote::quote;
use proc_macro::TokenStream;

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let item = Parser::new(tokens).parse_item()?;

    generate_impl(&item)
}

fn generate_impl(item: &Item) -> Result<TokenStream, Error> {
    let body = generate_for_each_variant(item, |span, attrs, fields| {
        if let Some(attr) = attrs.get("transparent")? {
            attr.must_be_empty()?;

            if let [UnifiedField { ty, access_ref, .. }] = fields {
                Ok(quote! {
                    <#ty as std::fmt::Display>::fmt(#access_ref, f)
                })
            } else {
                Err(Error::new("#[transparent] only supports one item").span(attr.span))
            }
        } else {
            let format_str = if let Some(attr) = attrs.get("display")? {
                attr.get_parenthesis_one_str()?
            } else {
                return Err(Error::new("missing #[display] attribute").span(span));
            };
            Ok(quote! {
                write!(f, #{ literal(format!("\"{format_str}\"")) })
            })
        }
    })?;

    Ok(generate_impl_for(
        item,
        Some("std::fmt::Display"),
        quote! {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                #body
            }
        },
    ))
}
