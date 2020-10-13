//! This example shows how you write a library crate which spawns
//! actors using a runtime provided by the caller.

use act_zero::*;
use futures::executor::LocalPool;
use futures::task::Spawn;

struct HelloWorldActor;

impl Actor for HelloWorldActor {}

impl HelloWorldActor {
    async fn say_hello(&mut self) {
        println!("Hello, world!");
    }
}

async fn run_example(spawner: &impl Spawn) -> Result<(), ActorError> {
    let addr = Addr::new(spawner, HelloWorldActor)?;
    call!(addr.say_hello()).await?;
    Ok(())
}

fn main() -> Result<(), ActorError> {
    let mut pool = LocalPool::new();
    let spawner = pool.spawner();

    pool.run_until(run_example(&spawner))
}
