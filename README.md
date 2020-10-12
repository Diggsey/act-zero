# act-zero

An actor system for Rust, designed with several goals in mind:

- No boilerplate.
- Ergonomic.
- Supports standard trait-based static and dynamic polymorphism.
- Embraces async/await.
- Executor agnostic.

Very little code is required to get started:

```rust
use std::error::Error;

use futures::executor::LocalPool;
use act_zero::*;

struct SimpleGreeter {
    number_of_greets: i32,
}

impl Actor for SimpleGreeter {}

impl SimpleGreeter {
    async fn greet(&mut self, name: String) -> ActorResult<String> {
        self.number_of_greets += 1;
        Produces::ok(format!(
            "Hello, {}. You are number {}!",
            name, self.number_of_greets
        ))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut pool = LocalPool::new();
    let spawner = pool.spawner();
    pool.run_until(async move {
        let actor_ref = Addr::new(
            &spawner,
            SimpleGreeter {
                number_of_greets: 0,
            },
        )?;

        let greeting = call!(actor_ref.greet("John".into())).await?;
        println!("{}", greeting);

        let greeting = call!(actor_ref.greet("Emma".into())).await?;
        println!("{}", greeting);
        Ok(())
    })
}
```
