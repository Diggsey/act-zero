use proc_macro2::TokenStream as TokenStream2;

use quote::{format_ident, quote, ToTokens};
use syn::{parse_quote, punctuated::Punctuated, token};

use crate::common::*;

struct ActorTrait {
    vis: syn::Visibility,
    unsafety: Option<token::Unsafe>,
    generics: syn::Generics,
    items: Vec<ActorTraitItem>,

    // Derived state
    internal_trait_ident: syn::Ident,
    internal_trait_path: syn::Path,
    msg_enum_ident: syn::Ident,
    msg_enum_path: syn::Path,
    trait_path: syn::Path,
    ext_trait_ident: syn::Ident,
    ext_trait_path: syn::Path,
}

fn doc_hidden_attr() -> syn::Attribute {
    parse_quote!(#[doc(hidden)])
}

impl ActorTrait {
    fn internal_trait(&self) -> syn::ItemTrait {
        syn::ItemTrait {
            attrs: vec![doc_hidden_attr()],
            vis: self.vis.clone(),
            unsafety: None,
            auto_token: None,
            trait_token: Default::default(),
            ident: self.internal_trait_ident.clone(),
            generics: self.generics.clone(),
            colon_token: Some(Default::default()),
            supertraits: parse_quote!(::core::marker::Sized + ::act_zero::Actor),
            brace_token: Default::default(),
            items: self
                .items
                .iter()
                .map(ActorTraitItem::internal_trait)
                .collect(),
        }
    }
    fn message_enum(&self) -> syn::ItemEnum {
        syn::ItemEnum {
            attrs: vec![doc_hidden_attr()],
            vis: self.vis.clone(),
            enum_token: Default::default(),
            ident: self.msg_enum_ident.clone(),
            generics: self.generics.clone(),
            brace_token: Default::default(),
            variants: self
                .items
                .iter()
                .filter(|item| item.is_object_safe)
                .map(ActorTraitItem::message_enum)
                .collect(),
        }
    }
    fn handle_impl(&self) -> syn::ItemImpl {
        let trait_path = &self.trait_path;
        let msg_enum_path = &self.msg_enum_path;

        let handle_msg_arms: Vec<_> = self
            .items
            .iter()
            .filter(|item| item.is_object_safe)
            .map(|item| ActorTraitItem::handle_impl(item, &self.msg_enum_ident))
            .collect();

        syn::ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: None,
            impl_token: Default::default(),
            generics: self.generics.clone(),
            trait_: Some((
                None,
                parse_quote!(::act_zero::Handle<#msg_enum_path>),
                Default::default(),
            )),
            self_ty: parse_quote!(dyn #trait_path),
            brace_token: Default::default(),
            items: vec![parse_quote!(
                fn handle(&self, msg: #msg_enum_path) {
                    match msg {
                        #(#handle_msg_arms)*
                    }
                }
            )]
            .into_iter()
            .collect(),
        }
    }
    fn upcast_impl(&self) -> syn::ItemImpl {
        let t_arg = format_ident!("__T");
        let trait_path = &self.trait_path;

        let mut generics = self.generics.clone();
        generics
            .params
            .push(parse_quote!(#t_arg: #trait_path + 'static));

        syn::ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: None,
            impl_token: Default::default(),
            generics,
            trait_: Some((
                None,
                parse_quote!(::act_zero::utils::UpcastFrom<#t_arg>),
                Default::default(),
            )),
            self_ty: parse_quote!(dyn #trait_path),
            brace_token: Default::default(),
            items: vec![
                parse_quote!(
                    fn upcast(this: ::std::sync::Arc<#t_arg>) -> ::std::sync::Arc<Self> {
                        this
                    }
                ),
                parse_quote!(
                    fn upcast_weak(this: ::std::sync::Weak<#t_arg>) -> ::std::sync::Weak<Self> {
                        this
                    }
                ),
            ],
        }
    }
    fn impl_remote(&self) -> syn::ItemImpl {
        let remote_arg = format_ident!("__R");
        let msg_enum_path = &self.msg_enum_path;

        let mut generics = self.generics.clone();
        generics
            .params
            .push(parse_quote!(#remote_arg: ::act_zero::Handle<#msg_enum_path>));

        syn::ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: self.unsafety.clone(),
            impl_token: Default::default(),
            generics,
            trait_: Some((None, self.trait_path.clone(), Default::default())),
            self_ty: parse_quote!(::act_zero::remote::Remote<#remote_arg>),
            brace_token: Default::default(),
            items: self
                .items
                .iter()
                .map(|item| item.impl_remote(&self.msg_enum_ident))
                .collect(),
        }
    }
    fn impl_local(&self) -> syn::ItemImpl {
        let local_arg = format_ident!("__A");
        let internal_trait_path = &self.internal_trait_path;

        let mut generics = self.generics.clone();
        generics
            .params
            .push(parse_quote!(#local_arg: #internal_trait_path));

        syn::ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: self.unsafety.clone(),
            impl_token: Default::default(),
            generics,
            trait_: Some((None, self.trait_path.clone(), Default::default())),
            self_ty: parse_quote!(::act_zero::Local<#local_arg>),
            brace_token: Default::default(),
            items: self
                .items
                .iter()
                .map(|item| item.impl_local(&local_arg))
                .collect(),
        }
    }
    fn ext_trait(&self) -> syn::ItemTrait {
        let trait_path = &self.trait_path;
        let mut generics = self.generics.clone();
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(Self::Inner: #trait_path));

        syn::ItemTrait {
            attrs: Vec::new(),
            vis: self.vis.clone(),
            unsafety: None,
            auto_token: None,
            trait_token: Default::default(),
            ident: self.ext_trait_ident.clone(),
            generics,
            colon_token: Some(Default::default()),
            supertraits: parse_quote!(::act_zero::AddrExt),
            brace_token: Default::default(),
            items: self
                .items
                .iter()
                .map(ActorTraitItem::ext_trait)
                .chain(
                    self.items
                        .iter()
                        .filter(|item| item.is_callable)
                        .map(ActorTraitItem::ext_trait_call),
                )
                .collect(),
        }
    }
    fn impl_ext(&self) -> syn::ItemImpl {
        let addr_arg = format_ident!("__A");
        let trait_path = &self.trait_path;

        let mut generics = self.generics.clone();
        generics
            .params
            .push(parse_quote!(#addr_arg: ::act_zero::AddrExt));
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#addr_arg::Inner: #trait_path));

        syn::ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: None,
            impl_token: Default::default(),
            generics,
            trait_: Some((None, self.ext_trait_path.clone(), Default::default())),
            self_ty: parse_quote!(#addr_arg),
            brace_token: Default::default(),
            items: Vec::new(),
        }
    }
}

struct ActorTraitItem {
    unsafety: Option<token::Unsafe>,
    ident: syn::Ident,
    generics: syn::Generics,
    inputs: Vec<syn::PatType>,
    is_object_safe: bool,
    is_callable: bool,
    default: Option<syn::Block>,

    // Derived state
    variant_ident: syn::Ident,
    safe_input_names: Punctuated<syn::Ident, token::Comma>,
    safe_input_args: Punctuated<syn::FnArg, token::Comma>,
}

impl ActorTraitItem {
    fn internal_trait(&self) -> syn::TraitItem {
        let this = this_ident();
        let mut inputs = Punctuated::new();
        inputs.push(parse_quote!(#this: &::act_zero::Local<Self>));
        inputs.extend(self.inputs.iter().cloned().map(syn::FnArg::Typed));

        syn::TraitItem::Method(syn::TraitItemMethod {
            attrs: Vec::new(),
            sig: syn::Signature {
                constness: None,
                asyncness: None,
                unsafety: self.unsafety.clone(),
                abi: None,
                fn_token: Default::default(),
                ident: self.ident.clone(),
                generics: self.generics.clone(),
                paren_token: Default::default(),
                inputs,
                variadic: None,
                output: syn::ReturnType::Default,
            },
            default: self.default.as_ref().map(|block| {
                let ts = block
                    .to_token_stream()
                    .replace_ident(&parse_quote!(self), &this);
                parse_quote!(#ts)
            }),
            semi_token: Some(Default::default()),
        })
    }

    fn message_enum(&self) -> syn::Variant {
        syn::Variant {
            attrs: Vec::new(),
            ident: self.variant_ident.clone(),
            fields: syn::Fields::Unnamed(syn::FieldsUnnamed {
                paren_token: Default::default(),
                unnamed: self
                    .inputs
                    .iter()
                    .map(|arg| syn::Field {
                        attrs: Vec::new(),
                        vis: syn::Visibility::Inherited,
                        ident: None,
                        colon_token: None,
                        ty: (*arg.ty).clone(),
                    })
                    .collect(),
            }),
            discriminant: None,
        }
    }

    fn handle_impl(&self, msg_enum_ident: &syn::Ident) -> syn::Arm {
        let method_ident = &self.ident;
        let variant_ident = &self.variant_ident;
        let safe_input_names = &self.safe_input_names;
        parse_quote!(
            #msg_enum_ident::#variant_ident(#safe_input_names) => self.#method_ident(#safe_input_names),
        )
    }

    fn impl_remote(&self, msg_enum_ident: &syn::Ident) -> syn::ImplItem {
        let variant_ident = &self.variant_ident;
        let safe_input_names = &self.safe_input_names;

        let mut inputs = Punctuated::new();
        inputs.push(parse_quote!(&self));
        inputs.extend(self.safe_input_args.clone());

        syn::ImplItem::Method(syn::ImplItemMethod {
            attrs: vec![parse_quote!(#[allow(unused)])],
            vis: syn::Visibility::Inherited,
            defaultness: None,
            sig: syn::Signature {
                constness: None,
                asyncness: None,
                unsafety: self.unsafety.clone(),
                abi: None,
                fn_token: Default::default(),
                ident: self.ident.clone(),
                generics: self.generics.clone(),
                paren_token: Default::default(),
                inputs,
                variadic: None,
                output: syn::ReturnType::Default,
            },
            block: if self.is_object_safe {
                parse_quote!({
                    self.inner().handle(#msg_enum_ident::#variant_ident(#safe_input_names));
                })
            } else {
                parse_quote!({
                    panic!("Only object-safe methods can be proxied");
                })
            },
        })
    }

    fn impl_local(&self, local_arg: &syn::Ident) -> syn::ImplItem {
        let method_ident = &self.ident;
        let safe_input_names = &self.safe_input_names;

        let mut inputs = Punctuated::new();
        inputs.push(parse_quote!(&self));
        inputs.extend(self.safe_input_args.clone());

        syn::ImplItem::Method(syn::ImplItemMethod {
            attrs: Vec::new(),
            vis: syn::Visibility::Inherited,
            defaultness: None,
            sig: syn::Signature {
                constness: None,
                asyncness: None,
                unsafety: self.unsafety.clone(),
                abi: None,
                fn_token: Default::default(),
                ident: self.ident.clone(),
                generics: self.generics.clone(),
                paren_token: Default::default(),
                inputs,
                variadic: None,
                output: syn::ReturnType::Default,
            },
            block: parse_quote!({
                #local_arg::#method_ident(self, #safe_input_names);
            }),
        })
    }
    fn ext_trait(&self) -> syn::TraitItem {
        let method_ident = &self.ident;
        let safe_input_names = &self.safe_input_names;

        let mut inputs = Punctuated::new();
        inputs.push(parse_quote!(&self));
        inputs.extend(self.safe_input_args.clone());

        syn::TraitItem::Method(syn::TraitItemMethod {
            attrs: Vec::new(),
            sig: syn::Signature {
                constness: None,
                asyncness: None,
                unsafety: self.unsafety.clone(),
                abi: None,
                fn_token: Default::default(),
                ident: self.ident.clone(),
                generics: self
                    .generics
                    .replace_path(&parse_quote!(Self), &parse_quote!(Self::Inner)),
                paren_token: Default::default(),
                inputs,
                variadic: None,
                output: syn::ReturnType::Default,
            },
            default: Some(parse_quote!({
                self.with(|inner| inner.#method_ident(#safe_input_names));
            })),
            semi_token: Some(Default::default()),
        })
    }
    fn ext_trait_call(&self) -> syn::TraitItem {
        let method_ident = &self.ident;
        let tx_ident = format_ident!("tx");
        let rx_ident = format_ident!("rx");

        let call_ident = format_ident!("call_{}", method_ident);
        let mut safe_input_names = self.safe_input_names.clone();
        safe_input_names.pop();
        safe_input_names.push(tx_ident.clone());

        let mut inputs = Punctuated::new();
        inputs.push(parse_quote!(&self));
        inputs.extend(self.safe_input_args.clone());
        let res_arg = inputs.pop().unwrap();
        let res_ty = if let syn::FnArg::Typed(x) = res_arg.value() {
            &x.ty
        } else {
            unreachable!()
        };

        syn::TraitItem::Method(syn::TraitItemMethod {
            attrs: Vec::new(),
            sig: syn::Signature {
                constness: None,
                asyncness: None,
                unsafety: self.unsafety.clone(),
                abi: None,
                fn_token: Default::default(),
                ident: call_ident,
                generics: self
                    .generics
                    .replace_path(&parse_quote!(Self), &parse_quote!(Self::Inner)),
                paren_token: Default::default(),
                inputs,
                variadic: None,
                output: parse_quote!(-> ::act_zero::Receiver<<#res_ty as ::act_zero::SenderExt>::Item>),
            },
            default: Some(parse_quote!({
                let (#tx_ident, #rx_ident) = ::act_zero::channel();
                self.#method_ident(#safe_input_names);
                #rx_ident
            })),
            semi_token: Some(Default::default()),
        })
    }
}

fn is_valid_receiver(receiver: &syn::Receiver) -> bool {
    if let Some((_, lifetime)) = &receiver.reference {
        lifetime.is_none() && receiver.mutability.is_none()
    } else {
        false
    }
}

fn is_self_type(t: &syn::Type) -> bool {
    *t == parse_quote!(Self)
}

fn is_sized_bound(t: &syn::TypeParamBound) -> bool {
    *t == parse_quote!(Sized)
        || *t == parse_quote!(core::marker::Sized)
        || *t == parse_quote!(std::marker::Sized)
        || *t == parse_quote!(::core::marker::Sized)
        || *t == parse_quote!(::std::marker::Sized)
}

fn parse(trait_item: &syn::ItemTrait) -> syn::Result<ActorTrait> {
    assert_none(&trait_item.auto_token, "Actor traits cannot be auto traits")?;

    let mut items = Vec::new();
    for item in trait_item.items.iter() {
        if let syn::TraitItem::Method(method) = item {
            assert_none(&method.sig.constness, "Actor trait methods cannot be const")?;
            assert_none(&method.sig.asyncness, "Actor trait methods cannot be async")?;
            assert_none(
                &method.sig.abi,
                "Actor trait methods must use the default ABI",
            )?;
            assert_none(
                &method.sig.variadic,
                "Actor trait methods cannot be variadic",
            )?;

            match &method.sig.output {
                syn::ReturnType::Default => {}
                _ => {
                    return Err(syn::Error::new_spanned(
                        &method.sig.output,
                        "Actor trait methods cannot specify a return type",
                    ))
                }
            }

            match method.sig.inputs.first() {
                Some(syn::FnArg::Receiver(recv)) if is_valid_receiver(recv) => {}
                _ => {
                    return Err(syn::Error::new_spanned(
                        &method.sig,
                        "Actor trait methods must have `&self` as the receiver",
                    ))
                }
            }
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

            let is_concrete = method.sig.generics.params.iter().all(|param| match param {
                syn::GenericParam::Type(_) | syn::GenericParam::Const(_) => false,
                syn::GenericParam::Lifetime(_) => true,
            });
            let has_sized_bound = method
                .sig
                .generics
                .where_clause
                .as_ref()
                .map(|clause| {
                    clause.predicates.iter().any(|pred| {
                        if let syn::WherePredicate::Type(t_pred) = pred {
                            is_self_type(&t_pred.bounded_ty)
                                && t_pred.bounds.iter().any(is_sized_bound)
                        } else {
                            false
                        }
                    })
                })
                .unwrap_or_default();

            let ident = method.sig.ident.clone();
            let variant_ident = camel_case_ident(&ident);
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

            let is_callable = safe_input_names
                .last()
                .map(|name| name == "res" || name == "_res")
                .unwrap_or_default();

            items.push(ActorTraitItem {
                unsafety: method.sig.unsafety.clone(),
                ident: method.sig.ident.clone(),
                generics: method.sig.generics.clone(),
                inputs,
                default: method.default.clone(),
                is_object_safe: is_concrete && !has_sized_bound,
                is_callable,
                variant_ident,
                safe_input_names,
                safe_input_args,
            })
        }
    }

    let ident = trait_item.ident.clone();
    let ty_generics = trait_item.generics.split_for_impl().1;
    let msg_enum_ident = message_enum_ident(&ident);
    let msg_enum_path = parse_quote!(#msg_enum_ident #ty_generics);
    let trait_path = parse_quote!(#ident #ty_generics);
    let internal_trait_ident = internal_trait_ident(&ident);
    let internal_trait_path = parse_quote!(#internal_trait_ident #ty_generics);
    let ext_trait_ident = ext_trait_ident(&ident);
    let ext_trait_path = parse_quote!(#ext_trait_ident #ty_generics);

    Ok(ActorTrait {
        unsafety: trait_item.unsafety.clone(),
        vis: trait_item.vis.clone(),
        generics: trait_item.generics.clone(),
        items,
        msg_enum_ident,
        msg_enum_path,
        trait_path,
        internal_trait_ident,
        internal_trait_path,
        ext_trait_ident,
        ext_trait_path,
    })
}

pub fn expand(mut trait_item: syn::ItemTrait) -> syn::Result<TokenStream2> {
    let spec = parse(&trait_item)?;

    // Clear all default implementations
    for item in &mut trait_item.items {
        if let syn::TraitItem::Method(m) = item {
            m.default = None;
        }
    }

    let internal_trait = spec.internal_trait();
    let message_enum = spec.message_enum();
    let handle_impl = spec.handle_impl();
    let upcast_impl = spec.upcast_impl();
    let impl_remote = spec.impl_remote();
    let impl_local = spec.impl_local();
    let ext_trait = spec.ext_trait();
    let impl_ext = spec.impl_ext();

    Ok(quote! {
        #trait_item
        #internal_trait
        #message_enum
        #handle_impl
        #upcast_impl
        #impl_remote
        #impl_local
        #ext_trait
        #impl_ext
    })
}
