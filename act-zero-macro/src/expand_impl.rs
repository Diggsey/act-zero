use proc_macro2::TokenStream as TokenStream2;

use quote::{format_ident, quote, ToTokens};
use syn::{parse_quote, punctuated::Punctuated, token};

use crate::common::*;

struct ActorTraitImpl {
    unsafety: Option<token::Unsafe>,
    generics: syn::Generics,
    items: Vec<ActorTraitImplItem>,
    self_ty: Box<syn::Type>,

    // Derived state
    internal_trait_path: syn::Path,
}

impl ActorTraitImpl {
    fn impl_internal(&self) -> syn::ItemImpl {
        syn::ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: self.unsafety,
            impl_token: Default::default(),
            generics: self.generics.clone(),
            trait_: Some((None, self.internal_trait_path.clone(), Default::default())),
            self_ty: self.self_ty.clone(),
            brace_token: Default::default(),
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
        inputs.push(parse_quote!(this: &::act_zero::Local<Self>));
        inputs.extend(self.safe_input_args.iter().cloned());
        let this_ident = this_ident();

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
        inner_inputs.push(match &self.self_ty {
            SelfTy::Mut => parse_quote!(#this_ident: &mut #self_ty),
            SelfTy::Ref => parse_quote!(#this_ident: &#self_ty),
            SelfTy::Other(t) => parse_quote!(#this_ident: #t),
        });
        inner_inputs.push(parse_quote!((#tuple_pats): (#tuple_tys)));
        let block = self
            .block
            .to_token_stream()
            .replace_ident(&format_ident!("self"), &this_ident);

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
                fn_token: Default::default(),
                ident: parse_quote!(inner),
                generics: combined_generics.clone(),
                paren_token: Default::default(),
                inputs: inner_inputs,
                variadic: None,
                output: self.output.clone(),
            },
            block: parse_quote!(#block),
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
                fn_token: Default::default(),
                ident: self.ident.clone(),
                generics: self.generics.clone(),
                paren_token: Default::default(),
                inputs,
                variadic: None,
                output: syn::ReturnType::Default,
            },
            block: match self.self_ty {
                SelfTy::Mut => parse_quote!({
                    #inner_fn
                    this.send_mut(::act_zero::async_fn::Closure::new(inner #turbofish, (#tuple_args)))
                }),
                SelfTy::Ref => parse_quote!({
                    #inner_fn
                    this.send(::act_zero::async_fn::Closure::new(inner #turbofish, (#tuple_args)))
                }),
                SelfTy::Other(_) => parse_quote!({
                    #inner_fn
                    this.send_fut(inner #turbofish(this.addr(), (#tuple_args)))
                }),
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

            let self_ty = match method.sig.inputs.first() {
                Some(syn::FnArg::Receiver(recv)) if is_valid_receiver(recv) => {
                    if recv.mutability.is_some() {
                        SelfTy::Mut
                    } else {
                        SelfTy::Ref
                    }
                }
                Some(syn::FnArg::Typed(p)) if *p.pat == parse_quote!(self) => {
                    SelfTy::Other(p.ty.clone())
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
    })
}

pub fn expand(item_impl: syn::ItemImpl) -> syn::Result<TokenStream2> {
    let spec = parse(&item_impl)?;

    let impl_internal = spec.impl_internal();

    Ok(quote! {
        #impl_internal
    })
}
