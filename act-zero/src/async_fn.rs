//! This module contains tools for dealing with async functions. These are similar to the
//! `FnOnce` trait from `std`, but they allow the lifetime of the returned future to be
//! bound by the lifetime of the argument. The different variants correspond to the
//! different reference types (`&T`, `&mut T`) but all variants consume `self` like `FnOnce`.
//!
//! For simplicity, only one argument is allowed. In addition, the returned future and its
//! output are assumed to be `Send + 'static`.
//!
//! Note: typically you will not need to use these tools. The exist primarily for use by
//! the expanded macro code from `#[act_zero]`.

use std::future::Future;
use std::marker::PhantomData;

use futures::channel::oneshot;
use futures::future::BoxFuture;
use futures::FutureExt;

/// Trait for async methods which take `&T` as the argument type.
pub trait AsyncFnOnce<T> {
    /// Output type of the returned future.
    type Output: Send + 'static;
    /// Call this function.
    fn call(self, arg: &T) -> BoxFuture<Self::Output>;
    /// Call this function when `self` is boxed.
    fn call_boxed(self: Box<Self>, arg: &T) -> BoxFuture<Self::Output>;
    /// Similar to `FutureExt::map`, except the callback also has access to the argument.
    fn map<G, R>(self, g: G) -> AsyncMap<Self, G>
    where
        G: FnOnce(Self::Output, &T) -> R,
        Self: Sized,
    {
        AsyncMap { fun: self, g }
    }
    /// Bind the output of this async function to this oneshot channel: when the future
    /// completes, the output will be sent on the channel.
    fn bind_output(self, tx: oneshot::Sender<Self::Output>) -> BindOutput<Self, Self::Output>
    where
        Self: Sized,
    {
        BindOutput { fun: self, tx }
    }
}

/// Trait for async methods which take `&mut T` as the argument type.
pub trait AsyncMutFnOnce<T> {
    /// Output type of the returned future.
    type Output: Send + 'static;
    /// Call this function.
    fn call(self, arg: &mut T) -> BoxFuture<Self::Output>;
    /// Call this function when `self` is boxed.
    fn call_boxed(self: Box<Self>, arg: &mut T) -> BoxFuture<Self::Output>;
    /// Similar to `FutureExt::map`, except the callback also has access to the argument.
    fn map<G, R>(self, g: G) -> AsyncMap<Self, G>
    where
        G: FnOnce(Self::Output, &mut T) -> R,
        Self: Sized,
    {
        AsyncMap { fun: self, g }
    }
    /// Bind the output of this async function to this oneshot channel: when the future
    /// completes, the output will be sent on the channel.
    fn bind_output(self, tx: oneshot::Sender<Self::Output>) -> BindOutput<Self, Self::Output>
    where
        Self: Sized,
    {
        BindOutput { fun: self, tx }
    }
}

impl<T, F> AsyncFnOnce<T> for Box<F>
where
    F: AsyncFnOnce<T> + ?Sized,
{
    type Output = F::Output;
    fn call(self, arg: &T) -> BoxFuture<Self::Output> {
        self.call_boxed(arg)
    }
    fn call_boxed(self: Box<Self>, arg: &T) -> BoxFuture<Self::Output> {
        (*self).call(arg)
    }
}

impl<T, F> AsyncMutFnOnce<T> for Box<F>
where
    F: AsyncMutFnOnce<T>,
{
    type Output = F::Output;
    fn call(self, arg: &mut T) -> BoxFuture<Self::Output> {
        self.call_boxed(arg)
    }
    fn call_boxed(self: Box<Self>, arg: &mut T) -> BoxFuture<Self::Output> {
        (*self).call(arg)
    }
}

/// Return type of `AsyncFnOnce::map` and `AsyncMutFnOnce::map`.
pub struct AsyncMap<F, G> {
    fun: F,
    g: G,
}

impl<F, G, T, R> AsyncFnOnce<T> for AsyncMap<F, G>
where
    F: AsyncFnOnce<T>,
    G: FnOnce(F::Output, &T) -> R + Send + 'static,
    R: Send + 'static,
    T: Sync,
{
    type Output = R;
    fn call(self, arg: &T) -> BoxFuture<R> {
        let AsyncMap { fun, g } = self;
        let fut = fun.call(arg);
        async move {
            let res = fut.await;
            g(res, arg)
        }
        .boxed()
    }
    fn call_boxed(self: Box<Self>, arg: &T) -> BoxFuture<Self::Output> {
        (*self).call(arg)
    }
}

impl<F, G, T, R> AsyncMutFnOnce<T> for AsyncMap<F, G>
where
    F: AsyncMutFnOnce<T> + Send + 'static,
    G: FnOnce(F::Output, &mut T) -> R + Send + 'static,
    R: Send + 'static,
    T: Send,
{
    type Output = R;
    fn call(self, arg: &mut T) -> BoxFuture<R> {
        let AsyncMap { fun, g } = self;
        async move {
            let fut = fun.call(arg);
            let res = fut.await;
            g(res, arg)
        }
        .boxed()
    }
    fn call_boxed(self: Box<Self>, arg: &mut T) -> BoxFuture<Self::Output> {
        (*self).call(arg)
    }
}

/// Return type of `AsyncFnOnce::bind_output` and `AsyncMutFnOnce::bind_output`.
pub struct BindOutput<F, R> {
    fun: F,
    tx: oneshot::Sender<R>,
}

impl<F, T, R> AsyncFnOnce<T> for BindOutput<F, R>
where
    F: AsyncFnOnce<T, Output = R>,
    R: Send + 'static,
{
    type Output = ();
    fn call(self, arg: &T) -> BoxFuture<()> {
        let BindOutput { fun, tx } = self;
        fun.call(arg)
            .map(move |res| {
                tx.send(res).ok();
            })
            .boxed()
    }
    fn call_boxed(self: Box<Self>, arg: &T) -> BoxFuture<Self::Output> {
        (*self).call(arg)
    }
}

impl<F, T, R> AsyncMutFnOnce<T> for BindOutput<F, R>
where
    F: AsyncMutFnOnce<T, Output = R>,
    R: Send + 'static,
{
    type Output = ();
    fn call(self, arg: &mut T) -> BoxFuture<()> {
        let BindOutput { fun, tx } = self;
        fun.call(arg)
            .map(move |res| {
                tx.send(res).ok();
            })
            .boxed()
    }
    fn call_boxed(self: Box<Self>, arg: &mut T) -> BoxFuture<Self::Output> {
        (*self).call(arg)
    }
}

/// Rust does not support async closures, and is incapable of inferring the correct lifetimes
/// for closures returning an `async move { ... }` block which captures a non-'static argument
/// to the closure.
/// To solve this problem, we introduce this explicit closure type, which takes a stand-alone
/// `async fn` expecting two arguments, and binds the second argument to a closure "upvar"
/// passed in at construction.
pub struct Closure<R, F, P> {
    fun: F,
    upvar: P,
    phantom: PhantomData<fn() -> R>,
}

impl<R, F, P> Closure<R, F, P> {
    /// Constructor.
    /// `fun` should implement `ClosureFn` or `ClosureFnMut` for this to be useful.
    pub fn new(fun: F, upvar: P) -> Self {
        Self {
            fun,
            upvar,
            phantom: PhantomData,
        }
    }
}

/// We can't even directly express the lifetime bounds of an `async fn` directly, so we
/// are forced to introduce a trait just to write the required bounds.
/// This trait is for the case when the first argument is `&T`.
pub trait ClosureFn<'a, T, P, R> {
    /// Type of the future returned by this async fn.
    type Future: Future<Output = R> + Send + 'a;
    /// Call this async fn with two arguments.
    fn call_closure(self, arg1: &'a T, arg2: P) -> Self::Future;
}

/// We can't even directly express the lifetime bounds of an `async fn` directly, so we
/// are forced to introduce a trait just to write the required bounds.
/// This trait is for the case when the first argument is `&mut T`.
pub trait ClosureFnMut<'a, T, P, R> {
    /// Type of the future returned by this async fn.
    type Future: Future<Output = R> + Send + 'a;
    /// Call this async fn with two arguments.
    fn call_closure(self, arg1: &'a mut T, arg2: P) -> Self::Future;
}

impl<'a, F, T, P, Fut, R> ClosureFn<'a, T, P, R> for F
where
    Fut: Future<Output = R> + Send + 'a,
    T: 'a,
    F: FnOnce(&'a T, P) -> Fut,
{
    type Future = Fut;
    fn call_closure(self, arg1: &'a T, arg2: P) -> Self::Future {
        self(arg1, arg2)
    }
}

impl<'a, F, T, P, Fut, R> ClosureFnMut<'a, T, P, R> for F
where
    Fut: Future<Output = R> + Send + 'a,
    T: 'a,
    F: FnOnce(&'a mut T, P) -> Fut,
{
    type Future = Fut;
    fn call_closure(self, arg1: &'a mut T, arg2: P) -> Self::Future {
        self(arg1, arg2)
    }
}

impl<F, T, P, R> AsyncFnOnce<T> for Closure<R, F, P>
where
    F: for<'a> ClosureFn<'a, T, P, R>,
    R: Send + 'static,
{
    type Output = R;
    fn call(self, arg: &T) -> BoxFuture<R> {
        let Closure { fun, upvar, .. } = self;
        fun.call_closure(arg, upvar).boxed()
    }
    fn call_boxed(self: Box<Self>, arg: &T) -> BoxFuture<Self::Output> {
        (*self).call(arg)
    }
}

impl<F, P, T, R> AsyncMutFnOnce<T> for Closure<R, F, P>
where
    F: for<'a> ClosureFnMut<'a, T, P, R>,
    R: Send + 'static,
{
    type Output = R;
    fn call(self, arg: &mut T) -> BoxFuture<R> {
        let Closure { fun, upvar, .. } = self;
        fun.call_closure(arg, upvar).boxed()
    }
    fn call_boxed(self: Box<Self>, arg: &mut T) -> BoxFuture<Self::Output> {
        (*self).call(arg)
    }
}
