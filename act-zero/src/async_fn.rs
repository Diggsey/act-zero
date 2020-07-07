use std::future::Future;
use std::marker::PhantomData;

use futures::channel::oneshot;
use futures::future::BoxFuture;
use futures::FutureExt;

pub trait AsyncFnOnce<T> {
    type Output: Send + 'static;
    fn call<'a>(self, arg: &'a T) -> BoxFuture<'a, Self::Output>;
    fn call_boxed<'a>(self: Box<Self>, arg: &'a T) -> BoxFuture<'a, Self::Output>;
    fn map<G, R>(self, g: G) -> AsyncMap<Self, G>
    where
        G: FnOnce(Self::Output, &T) -> R,
        Self: Sized,
    {
        AsyncMap { fun: self, g }
    }
    fn bind_output(self, tx: oneshot::Sender<Self::Output>) -> BindOutput<Self, Self::Output>
    where
        Self: Sized,
    {
        BindOutput { fun: self, tx }
    }
}

pub trait AsyncMutFnOnce<T> {
    type Output: Send + 'static;
    fn call<'a>(self, arg: &'a mut T) -> BoxFuture<'a, Self::Output>;
    fn call_boxed<'a>(self: Box<Self>, arg: &'a mut T) -> BoxFuture<'a, Self::Output>;
    fn map<G, R>(self, g: G) -> AsyncMap<Self, G>
    where
        G: FnOnce(Self::Output, &mut T) -> R,
        Self: Sized,
    {
        AsyncMap { fun: self, g }
    }
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
    fn call<'a>(self, arg: &'a T) -> BoxFuture<'a, Self::Output> {
        self.call_boxed(arg)
    }
    fn call_boxed<'a>(self: Box<Self>, arg: &'a T) -> BoxFuture<'a, Self::Output> {
        (*self).call(arg)
    }
}

impl<T, F> AsyncMutFnOnce<T> for Box<F>
where
    F: AsyncMutFnOnce<T>,
{
    type Output = F::Output;
    fn call<'a>(self, arg: &'a mut T) -> BoxFuture<'a, Self::Output> {
        self.call_boxed(arg)
    }
    fn call_boxed<'a>(self: Box<Self>, arg: &'a mut T) -> BoxFuture<'a, Self::Output> {
        (*self).call(arg)
    }
}

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
    fn call<'a>(self, arg: &'a T) -> BoxFuture<'a, R> {
        let AsyncMap { fun, g } = self;
        let fut = fun.call(arg);
        async move {
            let res = fut.await;
            g(res, arg)
        }
        .boxed()
    }
    fn call_boxed<'a>(self: Box<Self>, arg: &'a T) -> BoxFuture<'a, Self::Output> {
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
    fn call<'a>(self, arg: &'a mut T) -> BoxFuture<'a, R> {
        let AsyncMap { fun, g } = self;
        async move {
            let fut = fun.call(arg);
            let res = fut.await;
            g(res, arg)
        }
        .boxed()
    }
    fn call_boxed<'a>(self: Box<Self>, arg: &'a mut T) -> BoxFuture<'a, Self::Output> {
        (*self).call(arg)
    }
}

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
    fn call<'a>(self, arg: &'a T) -> BoxFuture<'a, ()> {
        let BindOutput { fun, tx } = self;
        fun.call(arg)
            .map(move |res| {
                tx.send(res).ok();
            })
            .boxed()
    }
    fn call_boxed<'a>(self: Box<Self>, arg: &'a T) -> BoxFuture<'a, Self::Output> {
        (*self).call(arg)
    }
}

impl<F, T, R> AsyncMutFnOnce<T> for BindOutput<F, R>
where
    F: AsyncMutFnOnce<T, Output = R>,
    R: Send + 'static,
{
    type Output = ();
    fn call<'a>(self, arg: &'a mut T) -> BoxFuture<'a, ()> {
        let BindOutput { fun, tx } = self;
        fun.call(arg)
            .map(move |res| {
                tx.send(res).ok();
            })
            .boxed()
    }
    fn call_boxed<'a>(self: Box<Self>, arg: &'a mut T) -> BoxFuture<'a, Self::Output> {
        (*self).call(arg)
    }
}

pub struct Closure<R, F, P> {
    fun: F,
    upvar: P,
    phantom: PhantomData<fn() -> R>,
}

impl<R, F, P> Closure<R, F, P> {
    pub fn new(fun: F, upvar: P) -> Self {
        Self {
            fun,
            upvar,
            phantom: PhantomData,
        }
    }
}

pub trait ClosureFn<'a, T, P, R> {
    type Future: Future<Output = R> + Send + 'a;
    fn call_closure(self, arg1: &'a T, arg2: P) -> Self::Future;
}

pub trait ClosureFnMut<'a, T, P, R> {
    type Future: Future<Output = R> + Send + 'a;
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
    fn call<'a>(self, arg: &'a T) -> BoxFuture<'a, R> {
        let Closure { fun, upvar, .. } = self;
        fun.call_closure(arg, upvar).boxed()
    }
    fn call_boxed<'a>(self: Box<Self>, arg: &'a T) -> BoxFuture<'a, Self::Output> {
        (*self).call(arg)
    }
}

impl<F, P, T, R> AsyncMutFnOnce<T> for Closure<R, F, P>
where
    F: for<'a> ClosureFnMut<'a, T, P, R>,
    R: Send + 'static,
{
    type Output = R;
    fn call<'a>(self, arg: &'a mut T) -> BoxFuture<'a, R> {
        let Closure { fun, upvar, .. } = self;
        fun.call_closure(arg, upvar).boxed()
    }
    fn call_boxed<'a>(self: Box<Self>, arg: &'a mut T) -> BoxFuture<'a, Self::Output> {
        (*self).call(arg)
    }
}
