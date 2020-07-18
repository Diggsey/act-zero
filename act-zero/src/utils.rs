use std::sync::{Arc, Weak};

/// Safety: implementors must not extract `T` from the
/// `Arc` or `Weak` passed in.
pub unsafe trait UpcastFrom<T: ?Sized> {
    fn upcast(this: Arc<T>) -> Arc<Self>;
    fn upcast_weak(this: Weak<T>) -> Weak<Self>;
}

pub trait IntoResult<T, E> {
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
