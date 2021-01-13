//! Tokio-specific functionality

use std::time::Instant;

use futures::task::{Spawn, SpawnError};

use crate::{timer, Actor, Addr};

/// Type representing the Tokio runtime.
#[derive(Debug, Copy, Clone, Default)]
pub struct Runtime;

/// Alias for a timer based on Tokio. This type can be default-constructed.
pub type Timer = timer::Timer<Runtime>;

/// Provides an infallible way to spawn an actor onto the Tokio runtime,
/// equivalent to `Addr::new`.
pub fn spawn_actor<T: Actor>(actor: T) -> Addr<T> {
    Addr::new(&Runtime, actor).unwrap()
}

impl Spawn for Runtime {
    fn spawn_obj(&self, future: futures::future::FutureObj<'static, ()>) -> Result<(), SpawnError> {
        tokio::spawn(future);
        Ok(())
    }
}

impl timer::SupportsTimers for Runtime {
    type Delay = tokio::time::Sleep;

    fn delay(&self, deadline: Instant) -> Self::Delay{
        tokio::time::sleep_until(deadline.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[tokio::test]
    async fn smoke_test() {
        struct Echo;

        impl Actor for Echo {}
        impl Echo {
            async fn echo(&mut self, x: &'static str) -> ActorResult<&'static str> {
                Produces::ok(x)
            }
        }

        let addr = spawn_actor(Echo);

        let res = call!(addr.echo("test")).await.unwrap();

        assert_eq!(res, "test");
    }

    #[tokio::test]
    async fn timer_test() {
        use std::time::{Duration, Instant};

        use async_trait::async_trait;
        use futures::channel::oneshot;

        #[derive(Default)]
        struct DebouncedEcho {
            addr: WeakAddr<Self>,
            timer: Timer,
            response: Option<(&'static str, oneshot::Sender<&'static str>)>,
        }

        #[async_trait]
        impl Actor for DebouncedEcho {
            async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()> {
                self.addr = addr.downgrade();
                Produces::ok(())
            }
        }

        #[async_trait]
        impl timer::Tick for DebouncedEcho {
            async fn tick(&mut self) -> ActorResult<()> {
                if self.timer.tick() {
                    let (msg, tx) = self.response.take().unwrap();
                    let _ = tx.send(msg);
                }
                Produces::ok(())
            }
        }
        impl DebouncedEcho {
            async fn echo(
                &mut self,
                msg: &'static str,
            ) -> ActorResult<oneshot::Receiver<&'static str>> {
                let (tx, rx) = oneshot::channel();
                self.response = Some((msg, tx));
                self.timer
                    .set_timeout_for_strong(self.addr.upgrade(), Duration::from_secs(1));
                Produces::ok(rx)
            }
        }

        let addr = spawn_actor(DebouncedEcho::default());

        let start_time = Instant::now();
        let res = call!(addr.echo("test")).await.unwrap();
        drop(addr);

        assert_eq!(res.await.unwrap(), "test");
        let end_time = Instant::now();

        assert!(end_time - start_time >= Duration::from_secs(1));
    }

    #[tokio::test]
    async fn weak_timer_test() {
        use std::time::{Duration, Instant};

        use async_trait::async_trait;
        use futures::channel::oneshot;

        #[derive(Default)]
        struct DebouncedEcho {
            addr: WeakAddr<Self>,
            timer: Timer,
            response: Option<(&'static str, oneshot::Sender<&'static str>)>,
        }

        #[async_trait]
        impl Actor for DebouncedEcho {
            async fn started(&mut self, addr: Addr<Self>) -> ActorResult<()> {
                self.addr = addr.downgrade();
                Produces::ok(())
            }
        }

        #[async_trait]
        impl timer::Tick for DebouncedEcho {
            async fn tick(&mut self) -> ActorResult<()> {
                if self.timer.tick() {
                    let (msg, tx) = self.response.take().unwrap();
                    let _ = tx.send(msg);
                }
                Produces::ok(())
            }
        }
        impl DebouncedEcho {
            async fn echo(
                &mut self,
                msg: &'static str,
            ) -> ActorResult<oneshot::Receiver<&'static str>> {
                let (tx, rx) = oneshot::channel();
                self.response = Some((msg, tx));
                self.timer
                    .set_timeout_for_weak(self.addr.clone(), Duration::from_secs(1));
                Produces::ok(rx)
            }
        }

        let addr = spawn_actor(DebouncedEcho::default());

        let start_time = Instant::now();
        let res = call!(addr.echo("test")).await.unwrap();
        drop(addr);

        assert!(res.await.is_err());
        let end_time = Instant::now();

        assert!(end_time - start_time < Duration::from_millis(10));
    }

    // Tests that .termination() waits for the Actor to be dropped.
    // Note that this probably won't race anyway, tokio would need
    // rt-threaded feature.
    #[tokio::test]
    async fn wait_drop_test() {
        use std::time::{Duration, Instant};
        struct WaitDrop {
            tx: std::sync::mpsc::SyncSender<u32>,
        }
        impl Actor for WaitDrop {}
        impl Drop for WaitDrop {
            fn drop(&mut self) {
                std::thread::sleep(Duration::from_millis(100));
                self.tx.send(5).unwrap();
            }
        }

        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let addr = spawn_actor(WaitDrop { tx });
        let ended = addr.termination();
        drop(addr);
        ended.await;
        let res = rx.try_recv();
        assert_eq!(res, Ok(5));
    }
}
