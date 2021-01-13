use std::error::Error;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::future::FutureExt;
use log::error;

use crate::Addr;

/// The type of error returned by an actor method.
pub type ActorError = Box<dyn Error + Send + Sync>;
/// Short alias for a `Result<Produces<T>, ActorError>`.
pub type ActorResult<T> = Result<Produces<T>, ActorError>;

/// A concrete type similar to a `BoxFuture<'static, Result<T, oneshot::Canceled>>`, but
/// without requiring an allocation if the value is immediately ready.
/// This type implements the `Future` trait and can be directly `await`ed.
#[derive(Debug)]
#[non_exhaustive]
pub enum Produces<T> {
    /// No value was produced.
    None,
    /// A value is ready.
    Value(T),
    /// A value may be sent in the future.
    Deferred(oneshot::Receiver<Produces<T>>),
}

impl<T> Unpin for Produces<T> {}

impl<T> Produces<T> {
    /// Returns `Ok(Produces::Value(value))`
    pub fn ok(value: T) -> ActorResult<T> {
        Ok(Produces::Value(value))
    }
}

impl<T> Future for Produces<T> {
    type Output = Result<T, oneshot::Canceled>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            break match mem::replace(&mut *self, Produces::None) {
                Produces::None => Poll::Ready(Err(oneshot::Canceled)),
                Produces::Value(value) => Poll::Ready(Ok(value)),
                Produces::Deferred(mut recv) => match recv.poll_unpin(cx) {
                    Poll::Ready(Ok(producer)) => {
                        *self = producer;
                        continue;
                    }
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => {
                        *self = Produces::Deferred(recv);
                        Poll::Pending
                    }
                },
            };
        }
    }
}

/// Trait implemented by all actors.
/// This trait is defined using the `#[async_trait]` attribute:
/// ```ignore
/// #[async_trait]
/// pub trait Actor: Send + 'static {
///     /// Called automatically when an actor is started. Actors can use this
///     /// to store their own address for future use.
///     async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
///     where
///         Self: Sized,
///     {
///         Ok(())
///     }
///
///     /// Called when any actor method returns an error. If this method
///     /// returns `true`, the actor will stop.
///     /// The default implementation logs the error using the `log` crate
///     /// and then stops the actor.
///     async fn error(&mut self, error: ActorError) -> bool {
///         error!("{}", error);
///         true
///     }
/// }
/// ```
///
/// In order to use a trait object with the actor system, such as with `Addr<dyn Trait>`,
/// the trait must extend this `Actor` trait.
#[async_trait]
pub trait Actor: Send + 'static {
    /// Called automatically when an actor is started. Actors can use this
    /// to store their own address for future use.
    async fn started(&mut self, _addr: Addr<Self>) -> ActorResult<()>
    where
        Self: Sized,
    {
        Produces::ok(())
    }

    /// Called when any actor method returns an error. If this method
    /// returns `true`, the actor will stop.
    /// The default implementation logs the error using the `log` crate
    /// and then stops the actor.
    async fn error(&mut self, error: ActorError) -> bool {
        error!("{}", error);
        true
    }
}

/// Actor methods may return any type implementing this trait.
pub trait IntoActorResult {
    /// The type to be sent back to the caller.
    type Output;
    /// Perform the conversion to an ActorResult.
    fn into_actor_result(self) -> ActorResult<Self::Output>;
}

impl<T> IntoActorResult for ActorResult<T> {
    type Output = T;
    fn into_actor_result(self) -> ActorResult<T> {
        self
    }
}

impl IntoActorResult for () {
    type Output = ();
    fn into_actor_result(self) -> ActorResult<()> {
        Produces::ok(())
    }
}
