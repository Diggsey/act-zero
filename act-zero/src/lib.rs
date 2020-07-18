use std::future::Future;
use std::sync::Arc;

pub use act_zero_macro::act_zero;
use futures::task::{Spawn, SpawnError};
use futures::FutureExt;

mod addr;
pub mod async_fn;
mod channel;
pub mod remote;
pub mod sync;
pub mod utils;

pub use addr::{Addr, AddrExt, WeakAddr};
pub use channel::{channel, Receiver, Sender, SenderExt};

use async_fn::{AsyncFnOnce, AsyncMutFnOnce};
use utils::IntoResult;

pub struct Local<T: Actor> {
    actor: sync::RwLock<T>,
}

pub trait Handle<M: Send + 'static>: Send + Sync + 'static {
    fn handle(&self, msg: M);
}

impl<T: Actor + Sync> Local<T> {
    pub fn send<F>(&self, f: F)
    where
        F: AsyncFnOnce<T> + Send + 'static,
        F::Output: IntoResult<(), T::Error>,
    {
        self.actor.run(f.map(|res, actor| {
            (if let Err(e) = res.into_result() {
                actor.errored(e)
            } else {
                false
            }) || actor.should_terminate()
        }));
    }
    pub fn send_mut<F>(&self, f: F)
    where
        F: AsyncMutFnOnce<T> + Send + 'static,
        F::Output: IntoResult<(), T::Error>,
    {
        self.actor.run_mut(f.map(|res, actor| {
            (if let Err(e) = res.into_result() {
                actor.errored_mut(e)
            } else {
                false
            }) || actor.should_terminate()
        }));
    }
    pub fn send_fut<F>(&self, f: F)
    where
        F: Future + Send + 'static,
        F::Output: IntoResult<(), T::Error>,
    {
        self.actor.run_fut(f.map(|res| {
            if let Err(e) = res.into_result() {
                T::errored_fut(e)
            } else {
                false
            }
        }));
    }
    pub fn addr(&self) -> Addr<Self> {
        // Safety: we mustn't allow callers to access a `Local` outside
        // of an `Arc`. Also, we mustn't add a destructor that calls this
        // method.
        unsafe {
            let res = Arc::from_raw(self);
            Arc::into_raw(res.clone());
            Addr(Some(res))
        }
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

    fn errored_fut(_error: Self::Error) -> bool {
        false
    }
    fn errored(&self, error: Self::Error) -> bool {
        Self::errored_fut(error)
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
