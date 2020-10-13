//! This example shows how you can use the async-std runtime with act-zero.

use act_zero::runtimes::async_std::spawn_actor;
use act_zero::*;

struct HelloWorldActor;

impl Actor for HelloWorldActor {}

impl HelloWorldActor {
    async fn say_hello(&mut self) {
        println!("Hello, world!");
    }
}

#[async_std::main]
async fn main() -> Result<(), ActorError> {
    let addr = spawn_actor(HelloWorldActor);
    call!(addr.say_hello()).await?;
    Ok(())
}
