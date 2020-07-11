# act-zero

An actor system for Rust, designed with several goals in mind:

- No boilerplate.
- Ergonomic.
- Supports standard trait-based static and dynamic polymorphism.
- Embraces async/await.
- Executor agnostic.

There are also some basic building blocks to support remoting, but the actual
mechanism to trasfer messages is left to the user.

Very little code is required to get started:

```rust
use std::error::Error;

use futures::executor::LocalPool;
use act_zero::*;

struct SimpleGreeter {
    number_of_greets: i32,
}

impl Actor for SimpleGreeter {
    type Error = ();
}

#[act_zero]
trait Greeter {
    fn greet(&self, name: String, res: Sender<String>);
}

#[act_zero]
impl Greeter for SimpleGreeter {
    async fn greet(&mut self, name: String, res: Sender<String>) {
        self.number_of_greets += 1;
        res.send(format!(
            "Hello, {}. You are number {}!",
            name, self.number_of_greets
        ))
        .ok();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut pool = LocalPool::new();
    let spawner = pool.spawner();
    pool.run_until(async move {
        let actor_ref = spawn(
            &spawner,
            SimpleGreeter {
                number_of_greets: 0,
            },
        )?;

        let greeting = actor_ref.call_greet("John".into()).await?;
        println!("{}", greeting);

        let greeting = actor_ref.call_greet("Emma".into()).await?;
        println!("{}", greeting);
        Ok(())
    })
}
```
