//! Dummy runtime implementation which panics on use.
//!
//! This is used when no default runtime is enabled, and allows
//! libraries to be built against this crate that *use* features
//! requiring a runtime, but remaining runtime-agnostic.
//!
//! Binary crates should enable a specific default runtime, or
//! enable the `default-disabled` feature to ensure this runtime
//! is not used.

use std::time::Instant;

use futures::future::Pending;
use futures::task::{Spawn, SpawnError};

use crate::{timer, Actor, Addr};

/// Type representing the dummy runtime.
#[derive(Debug, Copy, Clone, Default)]
pub struct Runtime;

/// Alias for a dummy timer. This type can be default-constructed.
/// Will always panic on use.
pub type Timer = timer::Timer<Runtime>;

/// Spawn an actor onto the dummy runtime.
/// Will always panic.
pub fn spawn_actor<T: Actor>(actor: T) -> Addr<T> {
    Addr::new(&Runtime, actor).unwrap()
}

impl Spawn for Runtime {
    fn spawn_obj(
        &self,
        _future: futures::future::FutureObj<'static, ()>,
    ) -> Result<(), SpawnError> {
        panic!("No default runtime selected")
    }
}

impl timer::SupportsTimers for Runtime {
    type Delay = Pending<()>;
    fn delay(&self, _deadline: Instant) -> Self::Delay {
        panic!("No default runtime selected")
    }
}
