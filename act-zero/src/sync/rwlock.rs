use futures::channel::mpsc;
use futures::stream::FuturesUnordered;
use futures::task::{Spawn, SpawnError, SpawnExt};
use futures::{select_biased, StreamExt};

use crate::async_fn::{AsyncFnOnce, AsyncMutFnOnce};

type ExclusiveItem<T> = Box<dyn AsyncMutFnOnce<T, Output = bool> + Send + 'static>;
type SharedItem<T> = Box<dyn AsyncFnOnce<T, Output = bool> + Send + 'static>;

enum Item<T> {
    Exclusive(ExclusiveItem<T>),
    Shared(SharedItem<T>),
}

#[derive(Clone)]
pub struct RwLock<T> {
    channel: mpsc::UnboundedSender<Item<T>>,
}

async fn rwlock_run_shared_tasks<'a, T>(
    value: &'a mut T,
    initial: SharedItem<T>,
    channel: &'a mut mpsc::UnboundedReceiver<Item<T>>,
) -> Option<ExclusiveItem<T>> {
    let mut shared = FuturesUnordered::new();
    shared.push(initial.call_boxed(value));
    let res = loop {
        let item = if shared.is_empty() {
            channel.next().await
        } else {
            select_biased! {
                done = shared.next() => if done == Some(true) {
                    return None
                } else {
                    continue
                },
                item = channel.next() => item
            }
        };
        match item {
            Some(Item::Exclusive(task)) => break Some(task),
            Some(Item::Shared(task)) => shared.push(task.call_boxed(value)),
            None => break None,
        }
    };
    while let Some(_) = shared.next().await {}
    res
}

async fn rwlock_run_exclusive_tasks<'a, T>(
    value: &'a mut T,
    mut initial: Option<ExclusiveItem<T>>,
    channel: &'a mut mpsc::UnboundedReceiver<Item<T>>,
) -> Option<SharedItem<T>> {
    loop {
        if let Some(initial) = initial {
            if initial.call_boxed(value).await {
                return None;
            }
        }
        initial = match channel.next().await {
            Some(Item::Exclusive(task)) => Some(task),
            Some(Item::Shared(task)) => break Some(task),
            None => break None,
        }
    }
}

async fn rwlock_task<T>(mut value: T, mut channel: mpsc::UnboundedReceiver<Item<T>>) {
    let mut exclusive_task = None;
    loop {
        if let Some(task) =
            rwlock_run_exclusive_tasks(&mut value, exclusive_task, &mut channel).await
        {
            if let Some(task) = rwlock_run_shared_tasks(&mut value, task, &mut channel).await {
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
        spawner.spawn(rwlock_task(value, rx))?;
        Ok(Self { channel: tx })
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
}
