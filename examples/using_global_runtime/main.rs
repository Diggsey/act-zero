//! This example shows how you can allow a downstream crate to select a
//! single global runtime. This is useful when writing a library crate.
//!
//! The `cfg` attributes are used to make it easier to run this example,
//! but are not necessary in library code.
#![allow(unused)]

#[cfg(not(feature = "default-disabled"))]
use act_zero::runtimes::default::spawn_actor;
use act_zero::*;

struct HelloWorldActor;

impl Actor for HelloWorldActor {}

impl HelloWorldActor {
    async fn say_hello(&mut self) {
        println!("Hello, world!");
    }
}

#[cfg(not(feature = "default-disabled"))]
async fn run_example() -> Result<(), ActorError> {
    let addr = spawn_actor(HelloWorldActor);
    call!(addr.say_hello()).await?;
    Ok(())
}

// Everything below this point is only necessary for this example because
// it's a binary crate, and not a library.
#[cfg(all(
    any(feature = "default-tokio", feature = "default-async-std"),
    not(feature = "default-disabled")
))]
#[cfg_attr(feature = "default-async-std", async_std::main)]
#[cfg_attr(feature = "default-tokio", tokio::main)]
async fn main() -> Result<(), ActorError> {
    run_example().await
}

#[cfg(not(all(
    any(feature = "default-tokio", feature = "default-async-std"),
    not(feature = "default-disabled")
)))]
fn main() {
    panic!("No default runtime selected")
}
