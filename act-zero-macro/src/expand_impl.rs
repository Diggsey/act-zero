use proc_macro2::TokenStream as TokenStream2;

use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{parse_quote, punctuated::Punctuated, token};

use crate::common::*;
use crate::receiver::ReplaceReceiver;
use crate::respan::respan;

struct ActorTraitImpl {
    unsafety: Option<token::Unsafe>,
    generics: syn::Generics,
    items: Vec<ActorTraitImplItem>,
    self_ty: Box<syn::Type>,

    // Derived state
    internal_trait_path: syn::Path,

    // Span data
    original_impl: syn::ItemImpl,
}

impl ActorTraitImpl {
    fn impl_internal(&self) -> syn::ItemImpl {
        syn::ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: self.unsafety,
            impl_token: self.original_impl.impl_token,
            generics: self.generics.clone(),
            trait_: Some((
                None,
                self.internal_trait_path.clone(),
                self.original_impl.trait_.as_ref().unwrap().2,
            )),
            self_ty: self.self_ty.clone(),
            brace_token: self.original_impl.brace_token,
            items: self
                .items
                .iter()
                .map(|item| item.impl_internal(&self.generics, &self.self_ty))
                .collect(),
        }
    }
}

enum SelfTy {
    Mut,
    Ref,
    Other(Box<syn::Type>),
}

struct ActorTraitImplItem {
    unsafety: Option<token::Unsafe>,
    asyncness: Option<token::Async>,
    ident: syn::Ident,
    generics: syn::Generics,
    inputs: Vec<syn::PatType>,
    output: syn::ReturnType,
    self_ty: SelfTy,
    block: syn::Block,

    // Derived state
    safe_input_names: Punctuated<syn::Ident, token::Comma>,
    safe_input_args: Punctuated<syn::FnArg, token::Comma>,

    // Span data
    original_item: syn::ImplItemMethod,
    receiver_span: proc_macro2::Span,
}

fn combine_generics(a: &syn::Generics, b: &syn::Generics) -> syn::Generics {
    syn::Generics {
        lt_token: a.lt_token.or(b.lt_token),
        params: a.params.iter().chain(b.params.iter()).cloned().collect(),
        gt_token: a.gt_token.or(b.gt_token),
        where_clause: if let (Some(aw), Some(bw)) = (&a.where_clause, &b.where_clause) {
            Some(syn::WhereClause {
                where_token: aw.where_token,
                predicates: aw
                    .predicates
                    .iter()
                    .chain(bw.predicates.iter())
                    .cloned()
                    .collect(),
            })
        } else {
            a.where_clause.clone().or_else(|| b.where_clause.clone())
        },
    }
}

impl ActorTraitImplItem {
    fn impl_internal(&self, impl_generics: &syn::Generics, self_ty: &syn::Type) -> syn::ImplItem {
        let mut inputs = Punctuated::new();
        let self_arg = quote_spanned!(self.receiver_span => _self: &::act_zero::Local<Self>);
        inputs.push(parse_quote!(#self_arg));
        inputs.extend(self.safe_input_args.iter().cloned());

        let mut tuple_args = self.safe_input_names.clone();
        if !tuple_args.empty_or_trailing() {
            tuple_args.push_punct(Default::default());
        }

        let mut tuple_pats: Punctuated<_, token::Comma> =
            self.inputs.iter().map(|pat| &pat.pat).collect();
        if !tuple_pats.empty_or_trailing() {
            tuple_pats.push_punct(Default::default());
        }
        let mut tuple_tys: Punctuated<_, token::Comma> =
            self.inputs.iter().map(|pat| &pat.ty).collect();
        if !tuple_tys.empty_or_trailing() {
            tuple_tys.push_punct(Default::default());
        }

        let mut inner_inputs = Punctuated::new();
        inner_inputs.push(respan(
            &match &self.self_ty {
                SelfTy::Mut => parse_quote!(_self: &mut #self_ty),
                SelfTy::Ref => parse_quote!(_self: &#self_ty),
                SelfTy::Other(t) => parse_quote!(_self: #t),
            },
            self.receiver_span,
        ));

        let last_input = quote_spanned!(self.ident.span() => (#tuple_pats): (#tuple_tys));
        inner_inputs.push(parse_quote!(#last_input));
        let mut block = self.block.clone();
        ReplaceReceiver::with(self_ty.clone()).visit_block_mut(&mut block);

        let combined_generics = combine_generics(impl_generics, &self.generics);
        let ty_generics = combined_generics.split_for_impl().1;
        let turbofish = ty_generics.as_turbofish();

        let inner_fn = syn::ItemFn {
            attrs: Vec::new(),
            vis: syn::Visibility::Inherited,
            sig: syn::Signature {
                constness: None,
                asyncness: self.asyncness,
                unsafety: None,
                abi: None,
                fn_token: self.original_item.sig.fn_token,
                ident: respan(&format_ident!("inner"), self.original_item.sig.ident.span()),
                generics: combined_generics.clone(),
                paren_token: self.original_item.sig.paren_token,
                inputs: inner_inputs,
                variadic: None,
                output: self.output.clone(),
            },
            block: Box::new(block),
        };

        syn::ImplItem::Method(syn::ImplItemMethod {
            attrs: Vec::new(),
            vis: syn::Visibility::Inherited,
            defaultness: None,
            sig: syn::Signature {
                constness: None,
                asyncness: None,
                unsafety: self.unsafety,
                abi: None,
                fn_token: self.original_item.sig.fn_token,
                ident: self.ident.clone(),
                generics: self.generics.clone(),
                paren_token: self.original_item.sig.paren_token,
                inputs,
                variadic: None,
                output: syn::ReturnType::Default,
            },
            block: {
                let span = self.ident.span();
                let glue = match self.self_ty {
                    SelfTy::Mut => {
                        quote_spanned!(span => _self.send_mut(::act_zero::async_fn::Closure::new(inner #turbofish, (#tuple_args))))
                    }
                    SelfTy::Ref => {
                        quote_spanned!(span => _self.send(::act_zero::async_fn::Closure::new(inner #turbofish, (#tuple_args))))
                    }
                    SelfTy::Other(_) => {
                        quote_spanned!(span => _self.send_fut(inner #turbofish(_self.addr(), (#tuple_args))))
                    }
                };
                parse_quote!({
                    #inner_fn
                    #glue
                })
            },
        })
    }
}

fn is_valid_receiver(receiver: &syn::Receiver) -> bool {
    if let Some((_, lifetime)) = &receiver.reference {
        lifetime.is_none()
    } else {
        false
    }
}

fn parse(item_impl: &syn::ItemImpl) -> syn::Result<ActorTraitImpl> {
    assert_none(
        &item_impl.defaultness,
        "Default actor trait implementations are not supported",
    )?;

    let mut items = Vec::new();
    for item in item_impl.items.iter() {
        if let syn::ImplItem::Method(method) = item {
            assert_none(
                &method.defaultness,
                "Default actor trait implementations are not supported",
            )?;
            assert_none(&method.sig.constness, "Actor trait methods cannot be const")?;
            assert_none(
                &method.sig.abi,
                "Actor trait methods must use the default ABI",
            )?;
            assert_none(
                &method.sig.variadic,
                "Actor trait methods cannot be variadic",
            )?;

            let (self_ty, receiver_span) = match method.sig.inputs.first() {
                Some(syn::FnArg::Receiver(recv)) if is_valid_receiver(recv) => (
                    if recv.mutability.is_some() {
                        SelfTy::Mut
                    } else {
                        SelfTy::Ref
                    },
                    recv.self_token.span,
                ),
                Some(syn::FnArg::Typed(p)) if *p.pat == parse_quote!(self) => {
                    (SelfTy::Other(p.ty.clone()), p.span())
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        &method.sig,
                        "Actor trait methods must have `&self` as the receiver",
                    ))
                }
            };
            let inputs: Vec<_> = method
                .sig
                .inputs
                .iter()
                .skip(1)
                .map(|input| match input {
                    syn::FnArg::Receiver(recv) => Err(syn::Error::new_spanned(
                        recv,
                        "Unexpected receiver argument",
                    )),
                    syn::FnArg::Typed(p) => Ok(p.clone()),
                })
                .collect::<syn::Result<_>>()?;

            let ident = method.sig.ident.clone();
            let safe_input_names: Punctuated<_, _> = inputs
                .iter()
                .enumerate()
                .map(|(index, input)| {
                    if let syn::Pat::Ident(name) = &*input.pat {
                        sanitize_ident(&name.ident)
                    } else {
                        format_ident!("arg{}", index)
                    }
                })
                .collect();
            let safe_input_args = inputs
                .iter()
                .zip(&safe_input_names)
                .map(|(input, name)| {
                    syn::FnArg::Typed(syn::PatType {
                        attrs: Vec::new(),
                        pat: parse_quote!(#name),
                        colon_token: Default::default(),
                        ty: input.ty.clone(),
                    })
                })
                .collect();

            items.push(ActorTraitImplItem {
                unsafety: method.sig.unsafety,
                asyncness: method.sig.asyncness,
                ident,
                generics: method.sig.generics.clone(),
                inputs,
                output: method.sig.output.clone(),
                self_ty,
                block: method.block.clone(),

                // Derived state
                safe_input_names,
                safe_input_args,

                // Span data
                original_item: method.clone(),
                receiver_span,
            })
        }
    }

    let internal_trait_path = if let Some((_, trait_path, _)) = &item_impl.trait_ {
        let mut internal_trait_path = trait_path.clone();
        if let Some(part) = internal_trait_path.segments.last_mut() {
            let internal_trait_ident = internal_trait_ident(&part.ident);
            part.ident = internal_trait_ident;
            internal_trait_path
        } else {
            return Err(syn::Error::new_spanned(trait_path, "Invalid trait path"));
        }
    } else {
        return Err(syn::Error::new_spanned(
            &item_impl,
            "Must be an implementation of a trait",
        ));
    };

    Ok(ActorTraitImpl {
        unsafety: item_impl.unsafety,
        generics: item_impl.generics.clone(),
        self_ty: item_impl.self_ty.clone(),
        items,
        internal_trait_path,

        // Span data
        original_impl: item_impl.clone(),
    })
}

pub fn expand(item_impl: syn::ItemImpl) -> syn::Result<TokenStream2> {
    let spec = parse(&item_impl)?;

    let impl_internal = spec.impl_internal();

    Ok(quote! {
        #impl_internal
    })
}
