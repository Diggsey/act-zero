use heck::CamelCase;
use quote::{format_ident, ToTokens};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;

use crate::respan::respan;

pub fn assert_none<T: ToTokens, M: std::fmt::Display>(t: &Option<T>, msg: M) -> syn::Result<()> {
    if let Some(t) = t {
        Err(syn::Error::new_spanned(t, msg))
    } else {
        Ok(())
    }
}

pub fn internal_trait_ident(ident: &syn::Ident) -> syn::Ident {
    format_ident!("{}Impl", ident)
}

pub fn message_enum_ident(ident: &syn::Ident) -> syn::Ident {
    format_ident!("{}Msg", ident)
}

pub fn ext_trait_ident(ident: &syn::Ident) -> syn::Ident {
    format_ident!("{}Ext", ident)
}

pub fn camel_case_ident(ident: &syn::Ident) -> syn::Ident {
    syn::Ident::new(&ident.to_string().to_camel_case(), ident.span())
}

const RESERVED_IDENTS: &[&str] = &["_self", "tx", "rx", "inner"];

pub fn sanitize_ident(ident: &syn::Ident) -> syn::Ident {
    if RESERVED_IDENTS.iter().any(|r| ident == r) {
        format_ident!("{}_", ident)
    } else {
        ident.clone()
    }
}

pub struct PathReplacer {
    pub old: syn::Path,
    pub new: syn::Path,
}

impl VisitMut for PathReplacer {
    fn visit_path_mut(&mut self, p: &mut syn::Path) {
        if *p == self.old {
            let span = p.span();
            *p = respan(&self.new, span);
        } else {
            syn::visit_mut::visit_path_mut(self, p);
        }
    }
}
