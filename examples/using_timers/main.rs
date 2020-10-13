//! This example shows how you can use timers. For this example
//! we're using the Tokio runtime, but any runtime implementing
//! `SupportsTimers` can be used.
//!
//! See the runtime examples for different ways of configuring
//! the runtime.

use std::time::Duration;

use act_zero::runtimes::tokio::{spawn_actor, Timer};
use act_zero::timer::Tick;
use act_zero::*;
use async_trait::async_trait;
use tokio::task::spawn_blocking;

// Create an actor that prompts for input if the user is idle for too long
#[derive(Default)]
struct LonelyActor {
    addr: WeakAddr<Self>,
    timer: Timer,
}

#[async_trait]
impl Actor for LonelyActor {
    async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()> {
        // Store our own address for later
        self.addr = addr.downgrade();

        // Start the timer
        self.comfort().await;
        Produces::ok(())
    }
}

impl LonelyActor {
    async fn comfort(&mut self) {
        println!(":)");

        // Schedule our "tick" method to be called in 10 seconds.
        // Timer methods have variations for "strong" and "weak" addresses: the former will keep
        // the actor alive as long as the timer is still active, whilst the latter will allow the
        // actor to stop if there are no other references to it.
        // In this case it doesn't really matter, but using the weak variation avoids a conversion.
        self.timer
            .set_timeout_for_weak(self.addr.clone(), Duration::from_secs(10));
    }
}

// Actors must implement the `Tick` trait in order to use timers.
#[async_trait]
impl Tick for LonelyActor {
    async fn tick(&mut self) -> ActorResult<()> {
        // Tick events may be produced spuriously, so you should always call `tick()` on each timer
        // owned by the actor to confirm that the timer has elapsed.
        // This also allows you to determine *which* timer elapsed if you have multiple.
        if self.timer.tick() {
            println!("Are you still there?");
        }
        Produces::ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), ActorError> {
    let addr = spawn_actor(LonelyActor::default());

    // Spawn a blocking task to read stdin. We could do this with Tokio's
    // async IO functionality but this is just easier.
    spawn_blocking(move || {
        let mut input = String::new();
        loop {
            std::io::stdin().read_line(&mut input)?;
            send!(addr.comfort());
        }
    })
    .await?
}
