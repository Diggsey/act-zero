use heck::CamelCase;
use proc_macro2::{Group as Group2, TokenStream as TokenStream2, TokenTree as TokenTree2};
use quote::{format_ident, ToTokens};
use syn::punctuated::{Pair, Punctuated};

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

pub fn this_ident() -> syn::Ident {
    format_ident!("this")
}

const RESERVED_IDENTS: &[&str] = &["this", "tx", "rx", "inner"];

pub fn sanitize_ident(ident: &syn::Ident) -> syn::Ident {
    if RESERVED_IDENTS.iter().any(|r| ident == r) {
        format_ident!("{}_", ident)
    } else {
        ident.clone()
    }
}

pub trait SubstitutePath: Sized {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self;
    fn replace_path(&self, old: &syn::Path, new: &syn::Path) -> Self {
        self.substitute_path(&mut |path| if path == old { new } else { path }.clone())
    }
}

impl<T: SubstitutePath, U: Clone> SubstitutePath for Punctuated<T, U> {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        self.pairs()
            .map(Pair::into_tuple)
            .map(|(value, punct)| Pair::new(value.substitute_path(f), punct.cloned()))
            .collect()
    }
}

impl<T: SubstitutePath> SubstitutePath for Option<T> {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        self.as_ref().map(|item| item.substitute_path(f))
    }
}

impl<T: SubstitutePath> SubstitutePath for Box<T> {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Box::new((**self).substitute_path(f))
    }
}

impl<T: SubstitutePath> SubstitutePath for Vec<T> {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        self.iter().map(|item| item.substitute_path(f)).collect()
    }
}

impl SubstitutePath for syn::Path {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        f(self)
    }
}

impl SubstitutePath for syn::Generics {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            lt_token: self.lt_token,
            params: self.params.substitute_path(f),
            gt_token: self.gt_token,
            where_clause: self.where_clause.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::WhereClause {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            where_token: self.where_token,
            predicates: self.predicates.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::WherePredicate {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        match self {
            Self::Type(x) => Self::Type(x.substitute_path(f)),
            Self::Lifetime(x) => Self::Lifetime(x.clone()),
            Self::Eq(x) => Self::Eq(x.substitute_path(f)),
        }
    }
}

impl SubstitutePath for syn::PredicateEq {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            lhs_ty: self.lhs_ty.substitute_path(f),
            eq_token: self.eq_token,
            rhs_ty: self.rhs_ty.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::PredicateType {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            lifetimes: self.lifetimes.clone(),
            bounded_ty: self.bounded_ty.substitute_path(f),
            colon_token: self.colon_token,
            bounds: self.bounds.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::GenericParam {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        match self {
            Self::Type(x) => Self::Type(x.substitute_path(f)),
            Self::Lifetime(x) => Self::Lifetime(x.clone()),
            Self::Const(x) => Self::Const(x.substitute_path(f)),
        }
    }
}

impl SubstitutePath for syn::ConstParam {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            attrs: self.attrs.clone(),
            const_token: self.const_token,
            ident: self.ident.clone(),
            colon_token: self.colon_token,
            ty: self.ty.substitute_path(f),
            eq_token: self.eq_token,
            default: self.default.clone(),
        }
    }
}

impl SubstitutePath for syn::TypeParam {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            attrs: self.attrs.clone(),
            ident: self.ident.clone(),
            colon_token: self.colon_token,
            bounds: self.bounds.substitute_path(f),
            eq_token: self.eq_token,
            default: self.default.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::Type {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        match self {
            Self::Array(x) => Self::Array(x.substitute_path(f)),
            Self::BareFn(x) => Self::BareFn(x.substitute_path(f)),
            Self::Group(x) => Self::Group(x.substitute_path(f)),
            Self::ImplTrait(x) => Self::ImplTrait(x.substitute_path(f)),
            Self::Paren(x) => Self::Paren(x.substitute_path(f)),
            Self::Path(x) => Self::Path(x.substitute_path(f)),
            Self::Ptr(x) => Self::Ptr(x.substitute_path(f)),
            Self::Reference(x) => Self::Reference(x.substitute_path(f)),
            Self::Slice(x) => Self::Slice(x.substitute_path(f)),
            Self::TraitObject(x) => Self::TraitObject(x.substitute_path(f)),
            Self::Tuple(x) => Self::Tuple(x.substitute_path(f)),
            other => other.clone(),
        }
    }
}

impl SubstitutePath for syn::TypeTuple {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            paren_token: self.paren_token,
            elems: self.elems.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::TypeTraitObject {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            dyn_token: self.dyn_token,
            bounds: self.bounds.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::TypeSlice {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            bracket_token: self.bracket_token,
            elem: self.elem.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::TypeReference {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            and_token: self.and_token,
            lifetime: self.lifetime.clone(),
            mutability: self.mutability,
            elem: self.elem.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::TypePtr {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            star_token: self.star_token,
            const_token: self.const_token,
            mutability: self.mutability,
            elem: self.elem.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::TypePath {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            qself: self.qself.substitute_path(f),
            path: self.path.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::QSelf {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            lt_token: self.lt_token,
            ty: self.ty.substitute_path(f),
            position: self.position,
            as_token: self.as_token,
            gt_token: self.gt_token,
        }
    }
}

impl SubstitutePath for syn::TypeParen {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            paren_token: self.paren_token,
            elem: self.elem.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::TypeImplTrait {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            impl_token: self.impl_token,
            bounds: self.bounds.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::TypeGroup {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            group_token: self.group_token,
            elem: self.elem.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::TypeArray {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            bracket_token: self.bracket_token,
            elem: self.elem.substitute_path(f),
            semi_token: self.semi_token,
            len: self.len.clone(),
        }
    }
}

impl SubstitutePath for syn::TypeBareFn {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            lifetimes: self.lifetimes.clone(),
            unsafety: self.unsafety,
            abi: self.abi.clone(),
            fn_token: self.fn_token,
            paren_token: self.paren_token,
            inputs: self.inputs.substitute_path(f),
            variadic: self.variadic.clone(),
            output: self.output.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::BareFnArg {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            attrs: self.attrs.clone(),
            name: self.name.clone(),
            ty: self.ty.substitute_path(f),
        }
    }
}

impl SubstitutePath for syn::ReturnType {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        match self {
            Self::Default => Self::Default,
            Self::Type(x, y) => Self::Type(*x, y.substitute_path(f)),
        }
    }
}

impl SubstitutePath for syn::TypeParamBound {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        match self {
            Self::Trait(x) => Self::Trait(x.substitute_path(f)),
            Self::Lifetime(x) => Self::Lifetime(x.clone()),
        }
    }
}

impl SubstitutePath for syn::TraitBound {
    fn substitute_path(&self, f: &mut dyn FnMut(&syn::Path) -> syn::Path) -> Self {
        Self {
            paren_token: self.paren_token,
            modifier: self.modifier,
            lifetimes: self.lifetimes.clone(),
            path: self.path.substitute_path(f),
        }
    }
}

pub trait SubstituteToken: Sized {
    fn substitute_token(self, f: &mut dyn FnMut(TokenTree2) -> TokenTree2) -> Self;
    fn replace_ident(self, old: &syn::Ident, new: &syn::Ident) -> Self {
        self.substitute_token(&mut |tt| {
            if let TokenTree2::Ident(ident) = tt {
                TokenTree2::Ident(if ident == *old { new.clone() } else { ident })
            } else {
                tt
            }
        })
    }
}

impl SubstituteToken for TokenStream2 {
    fn substitute_token(self, f: &mut dyn FnMut(TokenTree2) -> TokenTree2) -> Self {
        self.into_iter().map(|tt| tt.substitute_token(f)).collect()
    }
}

impl SubstituteToken for TokenTree2 {
    fn substitute_token(self, f: &mut dyn FnMut(TokenTree2) -> TokenTree2) -> Self {
        let tmp = if let Self::Group(x) = self {
            Self::Group(x.substitute_token(f))
        } else {
            self
        };
        f(tmp)
    }
}

impl SubstituteToken for Group2 {
    fn substitute_token(self, f: &mut dyn FnMut(TokenTree2) -> TokenTree2) -> Self {
        Group2::new(self.delimiter(), self.stream().substitute_token(f))
    }
}
