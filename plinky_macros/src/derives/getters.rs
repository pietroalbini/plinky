use crate::error::Error;
use crate::parser::{Item, Parser, StructFields};
use crate::utils::generate_impl_for;
use plinky_macros_quote::quote;
use proc_macro::TokenStream;

pub(crate) fn derive(item: TokenStream) -> Result<TokenStream, Error> {
    let item = Parser::new(item).parse_item()?;
    let Item::Struct(struct_) = &item else {
        return Err(Error::new("#[derive(Getters)] is only supported on structs"));
    };
    let StructFields::StructLike(fields) = &struct_.fields else {
        return Err(Error::new("#[derive(Getters)] only supports structs with named fields"));
    };

    let mut getters = Vec::new();
    for field in fields {
        if let Some(attr) = field.attrs.get("get")? {
            attr.must_be_empty()?;

            getters.push(quote! {
                #[allow(unused)]
                pub fn #{ &field.name }(&self) -> #{ &field.ty } { self.#{ &field.name } }
            });
        }

        if let Some(attr) = field.attrs.get("get_ref")? {
            attr.must_be_empty()?;

            getters.push(quote! {
                #[allow(unused)]
                pub fn #{ &field.name }(&self) -> &#{ &field.ty } { &self.#{ &field.name } }
            });
        }
    }

    Ok(generate_impl_for(&item, None, quote! { #getters }))
}
