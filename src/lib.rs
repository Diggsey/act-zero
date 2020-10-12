//! # act-zero
//! An actor system for Rust, designed with several goals in mind:
//! - No boilerplate.
//! - Ergonomic.
//! - Supports standard trait-based static and dynamic polymorphism.
//! - Embraces async/await.
//! - Executor agnostic.
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
//! impl Actor for SimpleGreeter {}
//!
//! impl SimpleGreeter {
//!     async fn greet(&mut self, name: String) -> ActorResult<String> {
//!         self.number_of_greets += 1;
//!         Produces::ok(format!(
//!             "Hello, {}. You are number {}!",
//!             name, self.number_of_greets
//!         ))
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut pool = LocalPool::new();
//!     let spawner = pool.spawner();
//!     pool.run_until(async move {
//!         let actor_ref = Addr::new(
//!             &spawner,
//!             SimpleGreeter {
//!                 number_of_greets: 0,
//!             },
//!         )?;
//!
//!         let greeting = call!(actor_ref.greet("John".into())).await?;
//!         println!("{}", greeting);
//!
//!         let greeting = call!(actor_ref.greet("Emma".into())).await?;
//!         println!("{}", greeting);
//!         Ok(())
//!     })
//! }
//! ```
//!
//! For mixing traits and actors, it's recommended to use the `async_trait` crate
//! to allow using async methods in traits.

#![deny(missing_docs)]

mod actor;
mod addr;
mod macros;
pub mod runtimes;
pub mod timer;
mod utils;

pub use actor::*;
pub use addr::*;
pub use macros::*;
pub use utils::*;

#[doc(hidden)]
pub mod hidden {
    pub use futures::channel::oneshot;
    pub use futures::future::FutureExt;

    #[cfg(feature = "tracing")]
    pub use log::trace;

    #[cfg(not(feature = "tracing"))]
    #[doc(hidden)]
    #[macro_export]
    macro_rules! trace {
        // Strip out all uses of `trace!`
        ($($args:tt)*) => {};
    }
    #[cfg(not(feature = "tracing"))]
    pub use trace;

    #[cfg(feature = "tracing")]
    pub fn type_name_of_val<T: ?Sized>(_val: &T) -> tynm::TypeName<'static> {
        tynm::TypeName::new::<&T>()
    }
    #[cfg(feature = "tracing")]
    pub fn type_name_of_addr<T: crate::AddrLike>(_val: &T) -> tynm::TypeName<'static> {
        tynm::TypeName::new::<&T::Actor>()
    }
}
