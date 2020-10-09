//! Contains integrations with specific runtimes
//!
//! Supported features:
//! - `tokio`
//!   Enables the tokio runtime.
//! - `async-std`
//!   Enables the async-std runtime.
//! - `default-tokio`
//!   Enables the tokio runtime and re-exports it under the name `default`.
//! - `default-async-std`
//!   Enables the async-std runtime and re-exports it under the name `default`.
//! - `default-disabled`
//!   Prevents a default runtime being exported, regardless of other features.
//!
//! Multiple runtimes may be enabled, but only one default runtime may be
//! chosen. It is not necessary to choose a default runtime unless you want
//! to use the `default` module.
//!
//! If no default runtime is selected, and the `default-disabled` option is
//! not enabled, the `panic` runtime will be re-exported as the default.
//! This allows library authors to build against the default runtime whilst
//! remaining runtime agnostic.

#[cfg(feature = "tokio")]
pub mod tokio;

#[cfg(feature = "async-std")]
pub mod async_std;

pub mod panic;

#[cfg(all(feature = "default-tokio", not(feature = "default-disabled")))]
pub use self::tokio as default;

#[cfg(all(feature = "default-async-std", not(feature = "default-disabled")))]
pub use self::async_std as default;

#[cfg(not(any(
    feature = "default-tokio",
    feature = "default-async-std",
    feature = "default-disabled"
)))]
pub use self::panic as default;
