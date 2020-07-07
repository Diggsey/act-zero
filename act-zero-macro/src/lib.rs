use proc_macro::TokenStream;

use quote::ToTokens;
use syn::parse_quote;

#[proc_macro_attribute]
pub fn act_zero(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: syn::Item = syn::parse(item).expect("Not a valid item");
    match item {
        syn::Item::Trait(trait_item) => act_zero_trait(trait_item),
        syn::Item::Impl(impl_item) => act_zero_impl(impl_item),
        _ => panic!("act_zero attribute is not usable in this position"),
    }
}

fn is_callable_trait_method(method_item: &syn::TraitItemMethod) -> bool {
    if let Some(syn::FnArg::Receiver(receiver)) = method_item.sig.inputs.first() {
        if receiver.reference.is_some() && receiver.mutability.is_none() {
            if let Some(syn::FnArg::Typed(last_input)) = method_item.sig.inputs.last() {
                if let syn::Pat::Ident(input_ident) = last_input.pat.as_ref() {
                    return input_ident.ident == "res" || input_ident.ident == "_res";
                }
            }
        }
    }
    false
}

fn make_callable_trait_method(method_item: &syn::TraitItemMethod) -> syn::TraitItemMethod {
    let mut res = method_item.clone();
    let sender_arg = res.sig.inputs.pop().unwrap().into_value();
    let sender_ty = match sender_arg {
        syn::FnArg::Typed(sender_arg) => sender_arg.ty,
        _ => unreachable!(),
    };

    res.sig.output =
        parse_quote! {-> ::act_zero::Receiver<<#sender_ty as ::act_zero::hidden::SenderExt>::Item>};

    let prev_ident = res.sig.ident;
    let args: Vec<_> = res
        .sig
        .inputs
        .iter()
        .skip(1)
        .map(|input| match input {
            syn::FnArg::Typed(arg) => arg.pat.clone(),
            _ => unreachable!(),
        })
        .collect();
    res.sig.ident = syn::Ident::new(&format!("call_{}", prev_ident), prev_ident.span());
    res.default = Some(parse_quote! {{
        let (__tx, __rx) = ::act_zero::channel();
        self.#prev_ident(#(#args,)* __tx);
        __rx
    }});

    res
}

fn act_zero_trait(mut trait_item: syn::ItemTrait) -> TokenStream {
    let call_methods: Vec<_> = trait_item
        .items
        .iter()
        .filter_map(|item| match item {
            syn::TraitItem::Method(method_item) if is_callable_trait_method(method_item) => Some(
                syn::TraitItem::Method(make_callable_trait_method(method_item)),
            ),
            _ => None,
        })
        .collect();
    trait_item.items.extend(call_methods);
    trait_item.into_token_stream().into()
}

fn act_zero_impl(_item: syn::ItemImpl) -> TokenStream {
    unimplemented!()
}
