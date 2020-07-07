use std::sync::Arc;

pub use act_zero_macro::act_zero;
pub use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::task::{Spawn, SpawnError};

#[doc(hidden)]
pub mod hidden;

mod addr;
pub mod async_fn;
pub mod channel;
pub mod remote;
pub mod sync;

pub use addr::{Addr, AddrExt, WeakAddr};
use async_fn::{AsyncFnOnce, AsyncMutFnOnce};

pub struct Local<T: Actor> {
    actor: sync::RwLock<T>,
}

pub trait Handle<M: Send + 'static> {
    fn handle(&self, msg: M);
}

impl<T: Actor + Sync> Local<T> {
    pub fn send(&self, f: impl AsyncFnOnce<T, Output = Result<(), T::Error>> + Send + 'static) {
        self.actor.run(f.map(|res, actor| {
            (if let Err(e) = res {
                actor.errored(e)
            } else {
                false
            }) || actor.should_terminate()
        }));
    }
    pub fn send_mut(
        &self,
        f: impl AsyncMutFnOnce<T, Output = Result<(), T::Error>> + Send + 'static,
    ) {
        self.actor.run_mut(f.map(|res, actor| {
            (if let Err(e) = res {
                actor.errored_mut(e)
            } else {
                false
            }) || actor.should_terminate()
        }));
    }
}

pub trait Actor: Send + Sync + 'static {
    type Error: Send + 'static;

    fn started(&mut self, _addr: Addr<Local<Self>>) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn errored(&self, _error: Self::Error) -> bool {
        false
    }
    fn errored_mut(&mut self, error: Self::Error) -> bool {
        self.errored(error);
        true
    }
    fn should_terminate(&self) -> bool {
        false
    }
}

pub fn spawn<S: Spawn, T: Actor>(spawner: &S, actor: T) -> Result<Addr<Local<T>>, SpawnError> {
    let addr = Addr(Some(Arc::new(Local {
        actor: sync::RwLock::new(spawner, actor)?,
    })));
    async fn call_started<T: Actor>(actor: &mut T, addr: Addr<Local<T>>) -> Result<(), T::Error> {
        actor.started(addr)
    }
    addr.with(|inner| inner.send_mut(async_fn::Closure::new(call_started, addr.clone())));
    Ok(addr)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
