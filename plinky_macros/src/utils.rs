use crate::parser::{
    Attributes, EnumVariantData, GenericParam, GenericParamConst, GenericParamNormal, Item,
    StructFields, Type,
};
use plinky_macros_quote::quote;
use proc_macro::{Ident, Span, TokenStream, TokenTree};
use std::fmt::Display;

pub(crate) fn literal(value: impl Display) -> TokenTree {
    TokenTree::Literal(value.to_string().parse().expect("invalid literal"))
}

pub(crate) fn ident(ident: impl AsRef<str>) -> TokenTree {
    TokenTree::Ident(Ident::new(ident.as_ref(), Span::call_site()))
}

pub(crate) fn generate_impl_for(
    item: &Item,
    trait_: Option<&str>,
    body: TokenStream,
) -> TokenStream {
    let trait_for = match trait_ {
        Some(trait_) => {
            let trait_: TokenStream = trait_.parse().unwrap();
            quote! { #trait_ for }
        }
        None => TokenStream::new(),
    };
    let (name, generics) = match item {
        Item::Struct(s) => (&s.name, &s.generics),
        Item::Enum(e) => (&e.name, &e.generics),
    };

    let mut generics_left = Vec::new();
    let mut generics_right = Vec::new();
    for generic in generics {
        match generic {
            GenericParam::Normal(GenericParamNormal { name, bound }) => {
                generics_left.push(quote!( #name: #bound, ));
                generics_right.push(quote!( #name, ));
            }
            GenericParam::Const(GenericParamConst { name, type_, .. }) => {
                generics_left.push(quote!( const #name: #type_, ));
                generics_right.push(quote!( #name, ));
            }
        }
    }

    quote! {
        impl<#generics_left> #trait_for #name<#generics_right> {
            #body
        }
    }
}

pub(crate) fn generate_for_each_variant<F, E>(item: &Item, mut f: F) -> Result<TokenStream, E>
where
    F: FnMut(Span, &Attributes, &[UnifiedField<'_>]) -> Result<TokenStream, E>,
{
    match item {
        Item::Struct(struct_) => {
            let mut declarations = Vec::new();
            let mut unified = Vec::new();
            match &struct_.fields {
                StructFields::None => {}
                StructFields::TupleLike(fields) => {
                    for (idx, field) in fields.iter().enumerate() {
                        let field_name = ident(&format!("f{idx}"));
                        declarations.push(quote! {
                            #[allow(unused)]
                            let #field_name = &self.#{ literal(idx) };
                        });
                        unified.push(UnifiedField {
                            attrs: &field.attrs,
                            ty: &field.ty,
                            access_ref: quote!(#field_name),
                        });
                    }
                }
                StructFields::StructLike(fields) => {
                    for field in fields {
                        declarations.push(quote! {
                            #[allow(unused)]
                            let #{ &field.name } = &self.#{ &field.name };
                        });
                        unified.push(UnifiedField {
                            attrs: &field.attrs,
                            ty: &field.ty,
                            access_ref: quote!(#{ &field.name }),
                        });
                    }
                }
            }
            Ok(quote! {
                #declarations
                #{ f(struct_.span, &struct_.attrs, &unified)? }
            })
        }
        Item::Enum(enum_) => {
            let mut match_arms = Vec::new();
            for variant in &enum_.variants {
                let mut unified = Vec::new();
                let name = &variant.name;
                let lhs = match &variant.data {
                    EnumVariantData::None => quote!(Self::#name),
                    EnumVariantData::TupleLike(fields) => {
                        let mut declarations = Vec::new();
                        for (idx, field) in fields.iter().enumerate() {
                            let field_name = ident(&format!("f{idx}"));
                            declarations.push(quote!(#field_name,));
                            unified.push(UnifiedField {
                                attrs: &field.attrs,
                                ty: &field.ty,
                                access_ref: quote!(#field_name),
                            });
                        }
                        quote!(Self::#name(#declarations))
                    }
                    EnumVariantData::StructLike(fields) => {
                        let mut declarations = Vec::new();
                        for field in fields {
                            declarations.push(quote!(#{ &field.name },));
                            unified.push(UnifiedField {
                                attrs: &field.attrs,
                                ty: &field.ty,
                                access_ref: quote!(#{ &field.name }),
                            });
                        }
                        quote!(Self::#name { #declarations })
                    }
                };
                match_arms.push(quote! {
                    #[allow(unused)]
                    #lhs => #{ f(variant.span, &variant.attrs, &unified)? },
                });
            }

            Ok(quote! {
                match self {
                    #match_arms
                }
            })
        }
    }
}

pub(crate) struct UnifiedField<'a> {
    pub(crate) attrs: &'a Attributes,
    pub(crate) ty: &'a Type,
    pub(crate) access_ref: TokenStream,
}
