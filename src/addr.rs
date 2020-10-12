use std::any::Any;
use std::cmp::Ordering;
use std::fmt::{self, Debug};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};
use std::{mem, ptr};

use futures::channel::{mpsc, oneshot};
use futures::future::{self, BoxFuture, FutureExt};
use futures::select_biased;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::task::{Spawn, SpawnError, SpawnExt};

use crate::{send, Actor, Produces, Termination};

type MutItem<T> = Box<dyn for<'a> FnOnce(&'a mut T) -> BoxFuture<'a, bool> + Send>;
type FutItem = BoxFuture<'static, ()>;

async fn mutex_task<T>(
    mut value: T,
    mut mut_channel: mpsc::UnboundedReceiver<MutItem<T>>,
    mut fut_channel: mpsc::UnboundedReceiver<FutItem>,
) {
    let mut futs = FuturesUnordered::new();
    loop {
        // Obtain an item
        let current_item = loop {
            if select_biased! {
                _ = futs.select_next_some() => false,
                item = mut_channel.next() => if let Some(item) = item {
                    break item
                } else {
                    true
                },
                item = fut_channel.select_next_some() => {
                    futs.push(item);
                    false
                },
                complete => true,
            } {
                return;
            }
        };

        // Wait for the current item to run
        let mut current_future = current_item(&mut value).fuse();
        loop {
            select_biased! {
                done = current_future => if done {
                    return;
                } else {
                    break
                },
                _ = futs.select_next_some() => {},
                item = fut_channel.select_next_some() => futs.push(item),
            }
        }
    }
}

struct AddrInner<T> {
    mut_channel: mpsc::UnboundedSender<MutItem<T>>,
    fut_channel: mpsc::UnboundedSender<FutItem>,
}

impl<T: 'static> AddrInner<T> {
    fn send_mut(this: &Arc<dyn Any + Send + Sync>, item: MutItem<T>) {
        this.downcast_ref::<Self>()
            .unwrap()
            .mut_channel
            .unbounded_send(item)
            .ok();
    }
    fn send_fut(this: &Arc<dyn Any + Send + Sync>, item: FutItem) {
        this.downcast_ref::<Self>()
            .unwrap()
            .fut_channel
            .unbounded_send(item)
            .ok();
    }

    // Must only be called if we have previously encountered a witness value of type `F`.
    fn send_mut_upcasted<U: ?Sized + 'static, F: Fn(&mut T) -> &mut U + Copy + Send>(
        this: &Arc<dyn Any + Send + Sync>,
        item: MutItem<U>,
    ) {
        assert_eq!(mem::size_of::<F>(), 0);

        this.downcast_ref::<Self>()
            .unwrap()
            .mut_channel
            .unbounded_send(Box::new(move |x| {
                let f: F = unsafe { mem::zeroed() };
                item(f(x))
            }))
            .ok();
    }
}

fn send_unreachable<T>(_: &Arc<dyn Any + Send + Sync>, _: T) {
    unreachable!()
}

/// Trait provides methods for spawning futures onto an actor. Implemented by
/// `Addr` and `WeakAddr` alike.
pub trait AddrLike: Send + Sync + Clone + Debug + 'static + AsAddr<Addr = Self> {
    /// Type of the actor reference by this address.
    type Actor: Actor + ?Sized;

    #[doc(hidden)]
    fn send_mut(&self, item: MutItem<Self::Actor>);

    /// Spawn a future onto the actor which does not return a value.
    fn send_fut(&self, fut: impl Future<Output = ()> + Send + 'static);

    /// Spawn a future onto the actor and provide the means to get back
    /// the result. The future will be cancelled if the receiver is
    /// dropped before it has completed.
    fn call_fut<R: Send + 'static>(
        &self,
        fut: impl Future<Output = Produces<R>> + Send + 'static,
    ) -> Produces<R> {
        let (mut tx, rx) = oneshot::channel();
        self.send_fut(async move {
            select_biased! {
                _ = tx.cancellation().fuse() => {}
                res = fut.fuse() => {
                    let _ = tx.send(res);
                }
            };
        });
        Produces::Deferred(rx)
    }

    /// Equivalent to `send_fut` but provides access to the actor's address.
    fn send_fut_with<F: Future<Output = ()> + Send + 'static>(&self, f: impl FnOnce(Self) -> F) {
        self.send_fut(f(self.clone()));
    }

    /// Equivalent to `call_fut` but provides access to the actor's address.
    fn call_fut_with<R: Send + 'static, F: Future<Output = Produces<R>> + Send + 'static>(
        &self,
        f: impl FnOnce(Self) -> F,
    ) -> Produces<R> {
        self.call_fut(f(self.clone()))
    }

    /// Returns a future which resolves when the actor terminates. If the
    /// actor has already terminated, or if this address is detached, the
    /// future will resolve immediately.
    fn termination(&self) -> Termination {
        Termination(self.call_fut(future::pending()))
    }
}

/// Implemented by addresses and references to addresses
pub trait AsAddr {
    /// The inner address type
    type Addr: AddrLike;

    /// Obtain a direct reference to the address
    fn as_addr(&self) -> &Self::Addr;
}

impl<T: AsAddr + ?Sized> AsAddr for &T {
    type Addr = T::Addr;
    fn as_addr(&self) -> &Self::Addr {
        (**self).as_addr()
    }
}
impl<T: Actor + ?Sized> AsAddr for crate::Addr<T> {
    type Addr = Self;
    fn as_addr(&self) -> &Self::Addr {
        self
    }
}
impl<T: Actor + ?Sized> AsAddr for crate::WeakAddr<T> {
    type Addr = Self;
    fn as_addr(&self) -> &Self::Addr {
        self
    }
}

/// A strong reference to a spawned actor. Actors can be spawned using `Addr::new`.
///
/// Methods can be called on the actor after it has been spawned using the
/// `send!(...)` and `call!(...)` macros.
///
/// Can be converted to the address of a trait-object using the `upcast!(...)`
/// macro.
pub struct Addr<T: ?Sized + 'static> {
    inner: Option<Arc<dyn Any + Send + Sync>>,
    send_mut: &'static (dyn Fn(&Arc<dyn Any + Send + Sync>, MutItem<T>) + Send + Sync),
    send_fut: &'static (dyn Fn(&Arc<dyn Any + Send + Sync>, FutItem) + Send + Sync),
}

impl<T: ?Sized> Debug for Addr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {{ detached: {} }}",
            std::any::type_name::<Self>(),
            self.inner.is_none()
        )
    }
}

impl<T: ?Sized> Clone for Addr<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            send_mut: self.send_mut,
            send_fut: self.send_fut,
        }
    }
}

impl<T: ?Sized> Default for Addr<T> {
    fn default() -> Self {
        Self::detached()
    }
}

impl<T: ?Sized, U: ?Sized> PartialEq<Addr<U>> for Addr<T> {
    fn eq(&self, rhs: &Addr<U>) -> bool {
        self.ptr() == rhs.ptr()
    }
}

impl<T: ?Sized, U: ?Sized> PartialEq<WeakAddr<U>> for Addr<T> {
    fn eq(&self, rhs: &WeakAddr<U>) -> bool {
        self.ptr() == rhs.ptr()
    }
}

impl<T: ?Sized> Eq for Addr<T> {}
impl<T: ?Sized> Hash for Addr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ptr().hash(state)
    }
}

impl<T: ?Sized, U: ?Sized> PartialOrd<Addr<U>> for Addr<T> {
    fn partial_cmp(&self, rhs: &Addr<U>) -> Option<Ordering> {
        self.ptr().partial_cmp(&rhs.ptr())
    }
}

impl<T: ?Sized, U: ?Sized> PartialOrd<WeakAddr<U>> for Addr<T> {
    fn partial_cmp(&self, rhs: &WeakAddr<U>) -> Option<Ordering> {
        self.ptr().partial_cmp(&rhs.ptr())
    }
}
impl<T: ?Sized> Ord for Addr<T> {
    fn cmp(&self, rhs: &Addr<T>) -> Ordering {
        self.ptr().cmp(&rhs.ptr())
    }
}

impl<T: Actor + ?Sized> AddrLike for Addr<T> {
    type Actor = T;

    #[doc(hidden)]
    fn send_mut(&self, item: MutItem<Self::Actor>) {
        if let Some(inner) = &self.inner {
            (self.send_mut)(inner, item);
        }
    }

    fn send_fut(&self, fut: impl Future<Output = ()> + Send + 'static) {
        if let Some(inner) = &self.inner {
            (self.send_fut)(inner, FutureExt::boxed(fut));
        }
    }
}

impl<T: Actor> Addr<T> {
    /// Spawn an actor using the given spawner. If successful returns the address of the actor.
    pub fn new<S: Spawn + ?Sized>(spawner: &S, value: T) -> Result<Self, SpawnError> {
        let (mtx, mrx) = mpsc::unbounded();
        let (ftx, frx) = mpsc::unbounded();
        spawner.spawn(mutex_task(value, mrx, frx))?;
        let addr = Self {
            inner: Some(Arc::new(AddrInner {
                mut_channel: mtx,
                fut_channel: ftx,
            })),
            send_mut: &AddrInner::<T>::send_mut,
            send_fut: &AddrInner::<T>::send_fut,
        };

        // Tell the actor its own address
        send!(addr.started(addr.clone()));

        Ok(addr)
    }
    #[doc(hidden)]
    pub fn upcast<U: ?Sized + Send + 'static, F: Fn(&mut T) -> &mut U + Copy + Send + 'static>(
        self,
        _f: F,
    ) -> Addr<U> {
        Addr {
            inner: self.inner,
            send_mut: &AddrInner::<T>::send_mut_upcasted::<U, F>,
            send_fut: self.send_fut,
        }
    }
}
impl<T: ?Sized> Addr<T> {
    /// Create an address which does not refer to any actor.
    pub fn detached() -> Self {
        Self {
            inner: None,
            send_mut: &send_unreachable,
            send_fut: &send_unreachable,
        }
    }
    fn ptr(&self) -> *const () {
        if let Some(inner) = &self.inner {
            Arc::as_ptr(inner) as *const ()
        } else {
            ptr::null()
        }
    }
}
impl<T: ?Sized + Send + 'static> Addr<T> {
    /// Downgrade to a weak reference, which does not try to keep the actor alive.
    pub fn downgrade(&self) -> WeakAddr<T> {
        WeakAddr {
            inner: self.inner.as_ref().map(Arc::downgrade),
            send_mut: self.send_mut,
            send_fut: self.send_fut,
        }
    }
    /// Attempt to downcast the address of a "trait-object actor" to a concrete type.
    ///
    /// This function may succeed even when the cast would normally be
    /// unsuccessful if the address has become detached.
    pub fn downcast<U: Send + 'static>(self) -> Result<Addr<U>, Addr<T>> {
        if let Some(inner) = &self.inner {
            if inner.is::<AddrInner<U>>() {
                Ok(Addr {
                    inner: self.inner,
                    send_mut: &AddrInner::<U>::send_mut,
                    send_fut: self.send_fut,
                })
            } else {
                Err(self)
            }
        } else {
            Ok(Addr::detached())
        }
    }
}

/// A weak reference to a spawned actor.
///
/// Methods can be called on the actor after it has been spawned using the
/// `send!(...)` and `call!(...)` macros.
///
/// Can be converted to the address of a trait-object using the `upcast!(...)`
/// macro.
pub struct WeakAddr<T: ?Sized + 'static> {
    inner: Option<Weak<dyn Any + Send + Sync>>,
    send_mut: &'static (dyn Fn(&Arc<dyn Any + Send + Sync>, MutItem<T>) + Send + Sync),
    send_fut: &'static (dyn Fn(&Arc<dyn Any + Send + Sync>, FutItem) + Send + Sync),
}

impl<T: ?Sized> Clone for WeakAddr<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            send_mut: self.send_mut,
            send_fut: self.send_fut,
        }
    }
}

impl<T: ?Sized> Debug for WeakAddr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {{..}}", std::any::type_name::<Self>())
    }
}

impl<T: ?Sized> Default for WeakAddr<T> {
    fn default() -> Self {
        Self::detached()
    }
}

impl<T: ?Sized, U: ?Sized> PartialEq<Addr<U>> for WeakAddr<T> {
    fn eq(&self, rhs: &Addr<U>) -> bool {
        self.ptr() == rhs.ptr()
    }
}

impl<T: ?Sized, U: ?Sized> PartialEq<WeakAddr<U>> for WeakAddr<T> {
    fn eq(&self, rhs: &WeakAddr<U>) -> bool {
        self.ptr() == rhs.ptr()
    }
}

impl<T: ?Sized> Eq for WeakAddr<T> {}
impl<T: ?Sized> Hash for WeakAddr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ptr().hash(state)
    }
}

impl<T: ?Sized, U: ?Sized> PartialOrd<Addr<U>> for WeakAddr<T> {
    fn partial_cmp(&self, rhs: &Addr<U>) -> Option<Ordering> {
        self.ptr().partial_cmp(&rhs.ptr())
    }
}

impl<T: ?Sized, U: ?Sized> PartialOrd<WeakAddr<U>> for WeakAddr<T> {
    fn partial_cmp(&self, rhs: &WeakAddr<U>) -> Option<Ordering> {
        self.ptr().partial_cmp(&rhs.ptr())
    }
}
impl<T: ?Sized> Ord for WeakAddr<T> {
    fn cmp(&self, rhs: &WeakAddr<T>) -> Ordering {
        self.ptr().cmp(&rhs.ptr())
    }
}

fn upgrade_weak<T: ?Sized>(maybe_weak: &Option<Weak<T>>) -> Option<Arc<T>> {
    maybe_weak.as_ref().and_then(Weak::upgrade)
}

impl<T: Actor + ?Sized> AddrLike for WeakAddr<T> {
    type Actor = T;

    #[doc(hidden)]
    fn send_mut(&self, item: MutItem<Self::Actor>) {
        if let Some(inner) = upgrade_weak(&self.inner) {
            (self.send_mut)(&inner, item);
        }
    }

    fn send_fut(&self, fut: impl Future<Output = ()> + Send + 'static) {
        if let Some(inner) = upgrade_weak(&self.inner) {
            (self.send_fut)(&inner, FutureExt::boxed(fut));
        }
    }
}

impl<T: ?Sized> WeakAddr<T> {
    /// Create an address which does not refer to any actor.
    pub fn detached() -> Self {
        Self {
            inner: None,
            send_mut: &send_unreachable,
            send_fut: &send_unreachable,
        }
    }
    // TODO: Replace this with an implementation using `Weak::as_ptr` once support for
    // unsized values hits stable.
    fn ptr(&self) -> *const () {
        if let Some(inner) = upgrade_weak(&self.inner) {
            Arc::as_ptr(&inner) as *const ()
        } else {
            ptr::null()
        }
    }
}
impl<T: Send + 'static> WeakAddr<T> {
    #[doc(hidden)]
    pub fn upcast<U: ?Sized + Send + 'static, F: Fn(&mut T) -> &mut U + Copy + Send + 'static>(
        self,
        _f: F,
    ) -> WeakAddr<U> {
        WeakAddr {
            inner: self.inner,
            send_mut: &AddrInner::<T>::send_mut_upcasted::<U, F>,
            send_fut: self.send_fut,
        }
    }
}
impl<T: ?Sized + Send + 'static> WeakAddr<T> {
    /// Upgrade this to a strong reference. If the actor has already stopped the returned
    /// address will be detached.
    pub fn upgrade(&self) -> Addr<T> {
        if let Some(inner) = upgrade_weak(&self.inner) {
            Addr {
                inner: Some(inner),
                send_mut: self.send_mut,
                send_fut: self.send_fut,
            }
        } else {
            Addr::detached()
        }
    }
}
