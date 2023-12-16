use proc_macro::{Group, Span, TokenStream, TokenTree};

pub(crate) struct Error {
    message: String,
    span: Option<Span>,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), span: None }
    }

    pub(crate) fn span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

pub(crate) fn emit_compiler_error(result: Result<TokenStream, Error>) -> TokenStream {
    match result {
        Ok(stream) => stream,
        Err(err) => {
            let message = format!("derive macro error: {}", err.message);
            let mut compile_error: TokenStream =
                format!("compile_error!(r#\"{message}\"#);").parse().unwrap();

            if let Some(span) = err.span {
                compile_error = set_span(span, compile_error);
            }

            compile_error
        }
    }
}

fn set_span(span: Span, stream: TokenStream) -> TokenStream {
    stream
        .into_iter()
        .map(|mut tree| {
            match &mut tree {
                TokenTree::Group(group) => {
                    *group = Group::new(group.delimiter(), set_span(span, group.stream()));
                    group.set_span(span);
                }
                TokenTree::Ident(ident) => ident.set_span(span),
                TokenTree::Punct(punct) => punct.set_span(span),
                TokenTree::Literal(literal) => literal.set_span(span),
            }
            tree
        })
        .collect()
}
