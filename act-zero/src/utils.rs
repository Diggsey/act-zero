//! Utility types and traits. These are typically used internally or by macro expansions
//! and you will not normally need to use them directly.

use std::sync::{Arc, Weak};

/// Helper trait to support upcasting from a concrete actor type to an actor trait object.
/// (see `Addr::upcast` and `WeakAddr::upcast`). This is automatically implemented by the
/// `#[act_zero]` macro.
///
/// Safety: implementors must not extract `T` from the
/// `Arc` or `Weak` passed in.
pub unsafe trait UpcastFrom<T: ?Sized> {
    /// Upcast an `Arc<T>`
    fn upcast(this: Arc<T>) -> Arc<Self>;
    /// Upcast a `Weak<T>`
    fn upcast_weak(this: Weak<T>) -> Weak<Self>;
}

/// Helper trait for things which can be converted into a result. This is used to allow
/// actor method implementations to optionally return a result.
pub trait IntoResult<T, E> {
    /// Perform the conversion.
    fn into_result(self) -> Result<T, E>;
}

impl<T, E> IntoResult<T, E> for Result<T, E> {
    fn into_result(self) -> Result<T, E> {
        self
    }
}

impl<T, E> IntoResult<T, E> for T {
    fn into_result(self) -> Result<T, E> {
        Ok(self)
    }
}
