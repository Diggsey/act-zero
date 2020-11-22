//! `async-std`-specific functionality

use std::time::Instant;

use futures::future::{BoxFuture, FutureExt};
use futures::task::{Spawn, SpawnError};

use crate::{timer, Actor, Addr};

/// Type representing the async-std runtime.
#[derive(Debug, Copy, Clone, Default)]
pub struct Runtime;

/// Alias for a timer based on async-std. This type can be default-constructed.
pub type Timer = timer::Timer<Runtime>;

/// Provides an infallible way to spawn an actor onto the async-std runtime,
/// equivalent to `Addr::new`.
pub fn spawn_actor<T: Actor>(actor: T) -> Addr<T> {
    Addr::new(&Runtime, actor).unwrap()
}

impl Spawn for Runtime {
    fn spawn_obj(&self, future: futures::future::FutureObj<'static, ()>) -> Result<(), SpawnError> {
        async_std::task::spawn(future);
        Ok(())
    }
}

impl timer::SupportsTimers for Runtime {
    type Delay = BoxFuture<'static, ()>;
    fn delay(&self, deadline: Instant) -> Self::Delay {
        let duration = deadline.saturating_duration_since(Instant::now());
        async_std::task::sleep(duration).boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    struct Echo;

    impl Actor for Echo {}
    impl Echo {
        async fn echo(&mut self, x: &'static str) -> ActorResult<&'static str> {
            Produces::ok(x)
        }
    }

    #[async_std::test]
    async fn smoke_test() {
        let addr = spawn_actor(Echo);

        let res = call!(addr.echo("test")).await.unwrap();

        assert_eq!(res, "test");
    }

    // Tests that .termination() waits for the Actor to be dropped
    #[async_std::test]
    async fn wait_drop_test() {
        struct WaitDrop {
            tx: std::sync::mpsc::Sender<u32>,
        }
        impl Actor for WaitDrop {}
        impl Drop for WaitDrop {
            fn drop(&mut self) {
                std::thread::sleep(Duration::from_millis(100));
                self.tx.send(5).unwrap();
            }
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let addr = spawn_actor(WaitDrop { tx});
        let ended = addr.termination();
        std::mem::drop(addr);
        ended.await;
        let res = rx.try_recv();
        assert_eq!(res, Ok(5));
    }
}
