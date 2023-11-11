extern crate proc_macro;

mod derives;
mod error;
mod parser;

use proc_macro::TokenStream;

#[proc_macro_derive(RawType, attributes(pointer_size, placed_on_elf32_after))]
pub fn derive_raw_type(item: TokenStream) -> TokenStream {
    error::emit_compiler_error(derives::raw_type::derive(item))
}
