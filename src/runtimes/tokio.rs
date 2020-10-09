//! Tokio-specific functionality

use std::time::Instant;

use futures::task::{Spawn, SpawnError};

use crate::{timer, Actor, Addr};

/// Type representing the Tokio runtime.
#[derive(Debug, Copy, Clone, Default)]
pub struct Runtime;

/// Alias for a timer based on Tokio. This type can be default-constructed.
pub type Timer = timer::Timer<Runtime>;

/// Provides an infallible way to spawn an actor onto the Tokio runtime,
/// equivalent to `Addr::new`.
pub fn spawn_actor<T: Actor>(actor: T) -> Addr<T> {
    Addr::new(&Runtime, actor).unwrap()
}

impl Spawn for Runtime {
    fn spawn_obj(&self, future: futures::future::FutureObj<'static, ()>) -> Result<(), SpawnError> {
        tokio::spawn(future);
        Ok(())
    }
}

impl timer::SupportsTimers for Runtime {
    type Delay = tokio::time::Delay;
    fn delay(&self, deadline: Instant) -> Self::Delay {
        tokio::time::delay_until(deadline.into())
    }
}
