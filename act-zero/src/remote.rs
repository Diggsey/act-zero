//! Tools for creating actor proxies. These can be used to communicate with actors in a different
//! process, or on a different machine.
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::Addr;

/// Generic actor proxy type. Implements the actor trait when `T` implements `Handle<M>` for the
/// message type corresponding to that actor trait.
#[derive(Debug, Serialize, Deserialize)]
pub struct Remote<T>(T);

impl<T> Remote<T> {
    /// Construct an instance of the remote proxy. Typically the caller will then `upcast()` this
    /// actor ref into an `Addr<dyn ActorTrait>` to conceal the fact that this is a remote proxy.
    pub fn new(inner: T) -> Addr<Self> {
        Addr(Some(Arc::new(Remote(inner))))
    }
    /// Access the inner value.
    pub fn inner(&self) -> &T {
        &self.0
    }
}
