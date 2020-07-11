use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

mod common;
mod expand_impl;
mod expand_trait;

#[proc_macro_attribute]
pub fn act_zero(_attr: TokenStream, item: TokenStream) -> TokenStream {
    match act_zero_impl(item) {
        Ok(tokens) => tokens,
        Err(e) => e.to_compile_error(),
    }
    .into()
}

fn act_zero_impl(item: TokenStream) -> syn::Result<TokenStream2> {
    let item: syn::Item = syn::parse(item)?;
    Ok(match item {
        syn::Item::Trait(trait_item) => expand_trait::expand(trait_item)?,
        syn::Item::Impl(impl_item) => expand_impl::expand(impl_item)?,
        _ => {
            return Err(syn::Error::new_spanned(
                item,
                "Expected a trait or a trait implementation",
            ))
        }
    })
}
