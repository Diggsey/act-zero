use heck::CamelCase;
use quote::{format_ident, ToTokens};

pub fn assert_none<T: ToTokens, M: std::fmt::Display>(t: &Option<T>, msg: M) -> syn::Result<()> {
    if let Some(t) = t {
        Err(syn::Error::new_spanned(t, msg))
    } else {
        Ok(())
    }
}

pub fn internal_trait_ident(ident: &syn::Ident) -> syn::Ident {
    format_ident!("__{}Internal", ident)
}

pub fn message_enum_ident(ident: &syn::Ident) -> syn::Ident {
    format_ident!("__{}Msg", ident)
}

pub fn ext_trait_ident(ident: &syn::Ident) -> syn::Ident {
    format_ident!("{}Ext", ident)
}

pub fn camel_case_ident(ident: &syn::Ident) -> syn::Ident {
    syn::Ident::new(&ident.to_string().to_camel_case(), ident.span())
}
