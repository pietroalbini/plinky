extern crate proc_macro;

mod derives;
mod error;
mod parser;
mod utils;

use proc_macro::TokenStream;

#[proc_macro_derive(
    Bitfield,
    attributes(bitfield_repr, bitfield_display_comma_separated, bitfield_only_on_abi, bit)
)]
pub fn derive_bitfield_repr(item: TokenStream) -> TokenStream {
    error::emit_compiler_error(derives::bitfield::derive(item))
}

#[proc_macro_derive(
    RawType,
    attributes(pointer_size, placed_on_elf32_after, placed_on_elf64_after)
)]
pub fn derive_raw_type(item: TokenStream) -> TokenStream {
    error::emit_compiler_error(derives::raw_type::derive(item))
}

#[proc_macro_derive(Error, attributes(source, from, transparent, diagnostic, diagnostic_context))]
pub fn derive_error(item: TokenStream) -> TokenStream {
    error::emit_compiler_error(derives::error::derive(item))
}

#[proc_macro_derive(Display, attributes(display, transparent))]
pub fn derive_display(item: TokenStream) -> TokenStream {
    error::emit_compiler_error(derives::display::derive(item))
}

#[proc_macro_derive(Getters, attributes(get, get_ref))]
pub fn derive_getters(item: TokenStream) -> TokenStream {
    error::emit_compiler_error(derives::getters::derive(item))
}
