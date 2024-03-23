use proc_macro::{TokenStream, TokenTree};

pub trait Quote {
    fn to_token_stream(&self) -> TokenStream;
}

impl Quote for TokenStream {
    fn to_token_stream(&self) -> TokenStream {
        self.clone()
    }
}

impl Quote for TokenTree {
    fn to_token_stream(&self) -> TokenStream {
        self.clone().into()
    }
}

impl<T: Quote> Quote for &T {
    fn to_token_stream(&self) -> TokenStream {
        (*self).to_token_stream()
    }
}

impl<T: Quote + Clone> Quote for Vec<T> {
    fn to_token_stream(&self) -> TokenStream {
        self.iter().cloned().map(|v| v.to_token_stream()).collect()
    }
}

impl<T: Quote + Clone> Quote for [T] {
    fn to_token_stream(&self) -> TokenStream {
        self.iter().cloned().map(|v| v.to_token_stream()).collect()
    }
}

