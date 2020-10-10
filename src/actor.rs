use std::error::Error;

use async_trait::async_trait;
use log::error;

use crate::Addr;

/// The type of error returned by an actor method.
pub type ActorError = Box<dyn Error + Send>;
/// Short alias for a `Result<T, ActorError>`.
pub type ActorResult<T> = Result<T, ActorError>;

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
        Ok(())
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
        Ok(())
    }
}
