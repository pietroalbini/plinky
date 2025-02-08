use crate::error::Error;
use crate::parser::{Attributes, EnumVariantData, Item, Parser, StructFields, Type};
use crate::utils::{generate_for_each_variant, generate_impl_for, ident, UnifiedField};
use plinky_macros_quote::quote;
use proc_macro::{TokenStream, TokenTree};

pub(crate) fn derive(tokens: TokenStream) -> Result<TokenStream, Error> {
    let item = Parser::new(tokens).parse_item()?;

    Ok(quote! {
        #{ generate_error_impl(&item)? }
        #{ generate_from_impls(&item)? }
    })
}

fn generate_error_impl(item: &Item) -> Result<TokenStream, Error> {
    Ok(generate_impl_for(
        item,
        Some("std::error::Error"),
        quote! {
            #{ generate_error_source(item)? }
            #{ generate_error_provide(item)? }
        },
    ))
}

fn generate_error_source(item: &Item) -> Result<TokenStream, Error> {
    let body = generate_for_each_variant(item, |_span, attrs, fields| {
        if let Some(attr) = attrs.get("transparent")? {
            attr.must_be_empty()?;

            match fields {
                [field] => {
                    for attr_name in ["from", "source"] {
                        if field.attrs.get(attr_name)?.is_some() {
                            return Err(Error::new(format!(
                                "#[transparent] is incompatible with #[{attr_name}]"
                            ))
                            .span(attr.span));
                        }
                    }
                    Ok(quote! {
                        <#{ &field.ty } as std::error::Error>::source(#{ &field.access_ref })
                    })
                }
                _ => {
                    Err(Error::new("#[transparent] items must have exactly one field")
                        .span(attr.span))
                }
            }
        } else {
            let mut source = None;
            for field in fields {
                for attr_name in ["from", "source"] {
                    if let Some(attr) = field.attrs.get(attr_name)? {
                        attr.must_be_empty()?;
                        match source {
                            None => source = Some(field),
                            Some(_) => {
                                return Err(Error::new("multiple error sources").span(attr.span));
                            }
                        }
                    }
                }
            }

            if let Some(source) = source {
                Ok(quote! { Some(#{ &source.access_ref }) })
            } else {
                Ok(quote! { None })
            }
        }
    })?;

    Ok(quote! {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            #body
        }
    })
}

fn generate_error_provide(item: &Item) -> Result<TokenStream, Error> {
    let body = generate_for_each_variant(item, |_span, attrs, fields| {
        let get_attribute = |name| -> Result<Option<&UnifiedField>, Error> {
            let mut result = None;
            for field in fields {
                if let Some(attr) = field.attrs.get(name)? {
                    attr.must_be_empty()?;

                    if attrs.get("transparent")?.is_some() {
                        return Err(Error::new(format!(
                            "#[transparent] and #[{name}] are not compatible"
                        ))
                        .span(attr.span));
                    } else if result.is_some() {
                        return Err(Error::new(format!("multiple #[{name}] are not supported"))
                            .span(attr.span));
                    }
                    result = Some(field);
                }
            }
            Ok(result)
        };

        let diagnostic = get_attribute("diagnostic")?;
        let diagnostic_context = get_attribute("diagnostic_context")?;

        if let Some(attr) = attrs.get("transparent")? {
            let [field] = fields else {
                return Err(Error::new("#[transparent] is only supported with exactly one field")
                    .span(attr.span));
            };
            Ok(quote!(#{ &field.access_ref }.provide(request)))
        } else {
            let mut provided = Vec::new();
            if let Some(diagnostic) = diagnostic {
                provided.push(quote! {
                    request.provide_ref::<dyn plinky_diagnostics::DiagnosticBuilder>(
                        #{ &diagnostic.access_ref } as &dyn plinky_diagnostics::DiagnosticBuilder
                    );
                });
            }
            if let Some(diagnostic_context) = diagnostic_context {
                provided.push(quote! {
                    request.provide_ref::<dyn plinky_diagnostics::DiagnosticContext>(
                        #{ &diagnostic_context.access_ref } as &dyn plinky_diagnostics::DiagnosticContext
                    );
                });
            }
            Ok(quote!({ #provided }))
        }
    })?;

    Ok(quote! {
        fn provide<'a>(&'a self, request: &mut std::error::Request<'a>) {
            #body
        }
    })
}

fn generate_from_impls(item: &Item) -> Result<Vec<TokenStream>, Error> {
    fn should_generate<'a>(
        container_attrs: &'a Attributes,
        fields_attrs: impl Iterator<Item = &'a Attributes>,
    ) -> Result<bool, Error> {
        let mut generate = None;

        if let Some(attr) = container_attrs.get("transparent")? {
            attr.must_be_empty()?;
            generate = Some(attr.span);
        }

        let mut fields_count = 0;
        for field_attrs in fields_attrs {
            fields_count += 1;
            if let Some(attr) = field_attrs.get("from")? {
                attr.must_be_empty()?;
                match generate {
                    None => generate = Some(attr.span),
                    Some(_) => {
                        return Err(
                            Error::new("multiple attributes to generate From impl").span(attr.span)
                        );
                    }
                }
            }
        }

        if let Some(span) = generate {
            if fields_count == 1 {
                Ok(true)
            } else {
                Err(Error::new("From impl can be generated only with one field").span(span))
            }
        } else {
            Ok(false)
        }
    }

    fn render<F>(item: &Item, ty: &Type, setter: F) -> TokenStream
    where
        F: FnOnce(TokenTree) -> TokenStream,
    {
        let variable = ident("__value__");
        generate_impl_for(
            item,
            Some(&format!("From<{}>", ty.0)),
            quote! {
                fn from(#variable: #ty) -> Self {
                    #{ setter(variable) }
                }
            },
        )
    }

    let mut generated = Vec::new();
    match item {
        Item::Struct(struct_) => match &struct_.fields {
            StructFields::None => {
                assert!(!should_generate(&struct_.attrs, std::iter::empty())?);
            }
            StructFields::TupleLike(fields) => {
                if should_generate(&struct_.attrs, fields.iter().map(|f| &f.attrs))? {
                    let field = &fields[0];
                    generated.push(render(item, &field.ty, |var| quote! { Self(#var) }));
                }
            }
            StructFields::StructLike(fields) => {
                if should_generate(&struct_.attrs, fields.iter().map(|f| &f.attrs))? {
                    let field = &fields[0];
                    generated.push(render(
                        item,
                        &field.ty,
                        |var| quote! { Self { #{ &field.name }: #var } },
                    ));
                }
            }
        },
        Item::Enum(enum_) => {
            for variant in &enum_.variants {
                match &variant.data {
                    EnumVariantData::None => {
                        assert!(!should_generate(&variant.attrs, std::iter::empty())?);
                    }
                    EnumVariantData::TupleLike(fields) => {
                        if should_generate(&variant.attrs, fields.iter().map(|f| &f.attrs))? {
                            let field = &fields[0];
                            generated.push(render(
                                item,
                                &field.ty,
                                |var| quote! { Self::#{ &variant.name }(#var) },
                            ));
                        }
                    }
                    EnumVariantData::StructLike(fields) => {
                        if should_generate(&variant.attrs, fields.iter().map(|f| &f.attrs))? {
                            let field = &fields[0];
                            generated.push(render(
                                item,
                                &field.ty,
                                |var| quote! { Self::#{ &variant.name } { #{ &field.name }: #var } },
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(generated)
}
