//! # act-zero
//! An actor system for Rust, designed with several goals in mind:
//! - No boilerplate.
//! - Ergonomic.
//! - Supports standard trait-based static and dynamic polymorphism.
//! - Embraces async/await.
//! - Executor agnostic.
//!
//! There are also some basic building blocks to support remoting, but the actual
//! mechanism to transfer messages is left to the user.
//!
//! Very little code is required to get started:
//!
//! ```
//! use std::error::Error;
//!
//! use futures::executor::LocalPool;
//! use act_zero::*;
//!
//! struct SimpleGreeter {
//!     number_of_greets: i32,
//! }
//!
//! impl Actor for SimpleGreeter {
//!     type Error = ();
//! }
//!
//! #[act_zero]
//! trait Greeter {
//!     fn greet(&self, name: String, res: Sender<String>);
//! }
//!
//! #[act_zero]
//! impl Greeter for SimpleGreeter {
//!     async fn greet(&mut self, name: String, res: Sender<String>) {
//!         self.number_of_greets += 1;
//!         res.send(format!(
//!             "Hello, {}. You are number {}!",
//!             name, self.number_of_greets
//!         ))
//!         .ok();
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut pool = LocalPool::new();
//!     let spawner = pool.spawner();
//!     pool.run_until(async move {
//!         let actor_ref = spawn(
//!             &spawner,
//!             SimpleGreeter {
//!                 number_of_greets: 0,
//!             },
//!         )?;
//!
//!         let greeting = actor_ref.call_greet("John".into()).await?;
//!         println!("{}", greeting);
//!
//!         let greeting = actor_ref.call_greet("Emma".into()).await?;
//!         println!("{}", greeting);
//!         Ok(())
//!     })
//! }
//! ```
//!
//! See [`#[act_zero]`](attr.act_zero.html) for documentation on the attribute which powers this crate.

#![deny(missing_docs)]

use std::future::Future;
use std::sync::Arc;

pub use act_zero_macro::act_zero;
use futures::task::{Spawn, SpawnError};
use futures::FutureExt;

mod addr;
pub mod async_fn;
mod channel;
pub mod remote;
mod sync;
pub mod utils;

pub use addr::{Addr, AddrExt, WeakAddr};
pub use channel::{channel, Receiver, Sender, SenderExt};

use async_fn::{AsyncFnOnce, AsyncMutFnOnce};
use utils::IntoResult;

/// Type of an actor running locally.
pub struct Local<T: Actor> {
    actor: sync::RwLock<T>,
}

/// This type is automatically implemented for local actors which implement the actor trait
/// corresponding to the message type `M`.
pub trait Handle<M: Send + 'static>: Send + Sync + 'static {
    /// Handle the message
    fn handle(&self, msg: M);
}

impl<T: Actor + Sync> Local<T> {
    #[doc(hidden)]
    pub fn send<F>(&self, f: F)
    where
        F: AsyncFnOnce<T> + Send + 'static,
        F::Output: IntoResult<(), T::Error>,
    {
        self.actor.run(f.map(|res, actor| {
            (if let Err(e) = res.into_result() {
                actor.errored(e)
            } else {
                false
            }) || actor.should_terminate()
        }));
    }
    #[doc(hidden)]
    pub fn send_mut<F>(&self, f: F)
    where
        F: AsyncMutFnOnce<T> + Send + 'static,
        F::Output: IntoResult<(), T::Error>,
    {
        self.actor.run_mut(f.map(|res, actor| {
            (if let Err(e) = res.into_result() {
                actor.errored_mut(e)
            } else {
                false
            }) || actor.should_terminate()
        }));
    }
    #[doc(hidden)]
    pub fn send_fut<F>(&self, f: F)
    where
        F: Future + Send + 'static,
        F::Output: IntoResult<(), T::Error>,
    {
        self.actor.run_fut(f.map(|res| {
            if let Err(e) = res.into_result() {
                T::errored_fut(e)
            } else {
                false
            }
        }));
    }
    #[doc(hidden)]
    pub fn addr(&self) -> Addr<Self> {
        // Safety: we mustn't allow callers to access a `Local` outside
        // of an `Arc`. Also, we mustn't add a destructor that calls this
        // method.
        unsafe {
            let res = Arc::from_raw(self);
            Arc::into_raw(res.clone());
            Addr(Some(res))
        }
    }
}

/// Implement this trait for types representing actors. The only requirement
/// is that you specify an `Error` type, all other methods are optional.
pub trait Actor: Send + Sync + 'static {
    /// The type of errors returned by actor methods.
    type Error: Send + 'static;

    /// Called automatically after an actor is spawned but before any messages are processed. Use
    /// this if you want the actor to keep a reference to itself. Usually you would first downcast
    /// the `Addr` to a `WeakAddr` to avoid the actor keeping itself alive.
    fn started(&mut self, _addr: Addr<Local<Self>>) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        Ok(())
    }

    /// Called when a future running on this actor returns an error. Use this method to log the
    /// error. Return `true` to stop the actor immediately.
    ///
    /// The default implementation discards the error and returns `false`.
    fn errored_fut(_error: Self::Error) -> bool {
        false
    }
    /// Called when a method taking `&self` returns an error. Use this method to log the
    /// error. Return `true` to stop the actor immediately.
    ///
    /// The default implementation defers to `Self::errored_fut`.
    fn errored(&self, error: Self::Error) -> bool {
        Self::errored_fut(error)
    }
    /// Called when a method taking `&mut self` returns an error. Use this method to log the
    /// error. Return `true` to stop the actor immediately.
    ///
    /// The default implementation first calls `Self::errored`, and then returns `true`.
    fn errored_mut(&mut self, error: Self::Error) -> bool {
        self.errored(error);
        true
    }
    /// Called after every actor method. If this returns `true` the actor will stop immediately.
    /// This can be used to gracefully shutdown the actor without returning an error.
    ///
    /// The default implementation returns `false`, so the actor will only stop when there are
    /// no more strong references to it.
    fn should_terminate(&self) -> bool {
        false
    }
}

/// Spawn an actor the provided spawner, returning its address or an error.
pub fn spawn<S: Spawn, T: Actor>(spawner: &S, actor: T) -> Result<Addr<Local<T>>, SpawnError> {
    let addr = Addr(Some(Arc::new(Local {
        actor: sync::RwLock::new(spawner, actor)?,
    })));
    async fn call_started<T: Actor>(actor: &mut T, addr: Addr<Local<T>>) -> Result<(), T::Error> {
        actor.started(addr)
    }
    addr.with(|inner| inner.send_mut(async_fn::Closure::new(call_started, addr.clone())));
    Ok(addr)
}
