use std::fmt::{self, Debug};
use std::future::Future;

use futures::channel::mpsc;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::task::{Spawn, SpawnError, SpawnExt};
use futures::{select_biased, FutureExt, StreamExt};

use crate::async_fn::{AsyncFnOnce, AsyncMutFnOnce};

type ExclusiveItem<T> = Box<dyn AsyncMutFnOnce<T, Output = bool> + Send + 'static>;
type SharedItem<T> = Box<dyn AsyncFnOnce<T, Output = bool> + Send + 'static>;
type FutureItem = BoxFuture<'static, bool>;

enum Item<T> {
    Exclusive(ExclusiveItem<T>),
    Shared(SharedItem<T>),
}

#[derive(Clone)]
pub struct RwLock<T> {
    channel: mpsc::UnboundedSender<Item<T>>,
    futs: mpsc::UnboundedSender<FutureItem>,
}

impl<T> Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {{..}}", std::any::type_name::<Self>())
    }
}

async fn rwlock_run_shared_tasks<'a, T>(
    value: &'a mut T,
    initial: SharedItem<T>,
    channel: &'a mut mpsc::UnboundedReceiver<Item<T>>,
    fut_channel: &'a mut mpsc::UnboundedReceiver<FutureItem>,
    futs: &'a mut FuturesUnordered<FutureItem>,
) -> Option<ExclusiveItem<T>> {
    let mut shared = FuturesUnordered::new();
    shared.push(initial.call_boxed(value));
    while !select_biased! {
        done = shared.select_next_some() => done,
        done = futs.select_next_some() => done,
        item = channel.select_next_some() => match item {
            Item::Exclusive(task) => return Some(task),
            Item::Shared(task) => {
                shared.push(task.call_boxed(value));
                false
            },
        },
        item = fut_channel.select_next_some() => {
            futs.push(item);
            false
        },
        complete => true,
    } {}
    None
}

async fn rwlock_run_exclusive_tasks<'a, T>(
    value: &'a mut T,
    mut initial: Option<ExclusiveItem<T>>,
    channel: &'a mut mpsc::UnboundedReceiver<Item<T>>,
    fut_channel: &'a mut mpsc::UnboundedReceiver<FutureItem>,
    futs: &'a mut FuturesUnordered<FutureItem>,
) -> Option<SharedItem<T>> {
    loop {
        if let Some(initial) = initial {
            let mut exclusive = initial.call_boxed(value).fuse();
            // Poll the current task to completion
            loop {
                select_biased! {
                    done = exclusive => if done {
                        return None;
                    } else {
                        break
                    },
                    done = futs.select_next_some() => if done {
                        return None;
                    },
                    item = fut_channel.select_next_some() => futs.push(item),
                }
            }
        }

        // Obtain a new task
        initial = Some(loop {
            if select_biased! {
                done = futs.select_next_some() => done,
                item = channel.select_next_some() => match item {
                    Item::Shared(task) => return Some(task),
                    Item::Exclusive(task) => break task,
                },
                item = fut_channel.select_next_some() => {
                    futs.push(item);
                    false
                },
                complete => true,
            } {
                return None;
            }
        })
    }
}

async fn rwlock_task<T>(
    mut value: T,
    mut channel: mpsc::UnboundedReceiver<Item<T>>,
    mut fut_channel: mpsc::UnboundedReceiver<FutureItem>,
) {
    let mut futs = FuturesUnordered::new();
    let mut exclusive_task = None;
    loop {
        if let Some(task) = rwlock_run_exclusive_tasks(
            &mut value,
            exclusive_task,
            &mut channel,
            &mut fut_channel,
            &mut futs,
        )
        .await
        {
            if let Some(task) =
                rwlock_run_shared_tasks(&mut value, task, &mut channel, &mut fut_channel, &mut futs)
                    .await
            {
                exclusive_task = Some(task);
                continue;
            }
        }
        break;
    }
}

impl<T: Send + Sync + 'static> RwLock<T> {
    pub fn new<S: Spawn>(spawner: &S, value: T) -> Result<Self, SpawnError> {
        let (tx, rx) = mpsc::unbounded();
        let (ftx, frx) = mpsc::unbounded();
        spawner.spawn(rwlock_task(value, rx, frx))?;
        Ok(Self {
            channel: tx,
            futs: ftx,
        })
    }

    pub fn run_mut<F>(&self, f: F) -> bool
    where
        F: AsyncMutFnOnce<T, Output = bool> + Send + 'static,
    {
        self.channel
            .unbounded_send(Item::Exclusive(Box::new(f)))
            .is_ok()
    }

    pub fn run<F>(&self, f: F) -> bool
    where
        F: AsyncFnOnce<T, Output = bool> + Send + 'static,
    {
        self.channel
            .unbounded_send(Item::Shared(Box::new(f)))
            .is_ok()
    }

    pub fn run_fut<F>(&self, f: F) -> bool
    where
        F: Future<Output = bool> + Send + 'static,
    {
        self.futs.unbounded_send(Box::pin(f)).is_ok()
    }
}
