//! This example shows how you can use the Tokio runtime with act-zero.

use act_zero::runtimes::tokio::spawn_actor;
use act_zero::*;

struct HelloWorldActor;

impl Actor for HelloWorldActor {}

impl HelloWorldActor {
    async fn say_hello(&mut self) {
        println!("Hello, world!");
    }
}

#[tokio::main]
async fn main() -> Result<(), ActorError> {
    let addr = spawn_actor(HelloWorldActor);
    call!(addr.say_hello()).await?;
    Ok(())
}
