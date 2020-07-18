use std::sync::{Arc, Weak};

use super::Handle;
use crate::utils::UpcastFrom;

pub trait AddrExt {
    type Inner: ?Sized;

    fn with<F: FnOnce(&Self::Inner)>(&self, f: F);
}

#[derive(Debug)]
pub struct WeakAddr<T: ?Sized>(pub(crate) Option<Weak<T>>);

impl<T: ?Sized> WeakAddr<T> {
    // This must be private as direct access to the `Arc` could allow the user
    // to obtain a `Local` outside of an `Arc`.
    fn map<U: ?Sized>(self, f: impl FnOnce(Weak<T>) -> Weak<U>) -> WeakAddr<U> {
        WeakAddr(self.0.map(f))
    }
    pub fn upcast<U: ?Sized + UpcastFrom<T>>(self) -> WeakAddr<U> {
        self.map(UpcastFrom::upcast_weak)
    }
}

impl<T: ?Sized> Clone for WeakAddr<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: ?Sized> Default for WeakAddr<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T: ?Sized> AddrExt for WeakAddr<T> {
    type Inner = T;
    fn with<F: FnOnce(&Self::Inner)>(&self, f: F) {
        if let Some(inner) = self.0.as_ref().and_then(Weak::upgrade) {
            f(&inner);
        }
    }
}

impl<M: Send + 'static, T: Handle<M>> Handle<M> for WeakAddr<T> {
    fn handle(&self, msg: M) {
        self.with(|inner| inner.handle(msg));
    }
}

#[derive(Debug)]
pub struct Addr<T: ?Sized>(pub(crate) Option<Arc<T>>);

impl<T: ?Sized> Addr<T> {
    // This must be private as direct access to the `Arc` could allow the user
    // to obtain a `Local` outside of an `Arc`.
    fn map<U: ?Sized>(self, f: impl FnOnce(Arc<T>) -> Arc<U>) -> Addr<U> {
        Addr(self.0.map(f))
    }
    pub fn upcast<U: ?Sized + UpcastFrom<T>>(self) -> Addr<U> {
        self.map(UpcastFrom::upcast)
    }
    pub fn downgrade(&self) -> WeakAddr<T> {
        WeakAddr(self.0.as_ref().map(Arc::downgrade))
    }
}

impl<T: ?Sized> Clone for Addr<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: ?Sized> Default for Addr<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: ?Sized> AddrExt for Addr<T> {
    type Inner = T;
    fn with<F: FnOnce(&Self::Inner)>(&self, f: F) {
        if let Some(inner) = &self.0 {
            f(inner);
        }
    }
}

impl<M: Send + 'static, T: Handle<M>> Handle<M> for Addr<T> {
    fn handle(&self, msg: M) {
        self.with(|inner| inner.handle(msg));
    }
}
