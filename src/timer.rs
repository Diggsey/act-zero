//! Functionality related to timers.
//!
//! Timers requires support from a runtime implementing the `SupportsTimers` trait.

use std::future::Future;
use std::mem;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::future::FutureExt;
use futures::select_biased;

use crate::{send, upcast, Actor, ActorResult, Addr, AddrLike, WeakAddr};

/// Timers can be used on runtimes implementing this trait.
pub trait SupportsTimers {
    /// The type of future returned by `delay`.
    type Delay: Future<Output = ()> + Send + Unpin + 'static;

    /// Create a future which will complete when the deadline
    /// is passed.
    fn delay(&self, deadline: Instant) -> Self::Delay;
}

/// Provides an actor with a "tick" method, that will be called whenever
/// a timer elapses.
///
/// Note: spurious tick events may be received: the expectation is that
/// actors respond to this event by checking if any timers have elapsed.
/// The `Timer` struct has a `tick()` method for this purpose.
///
/// This trait is defined using the `#[async_trait]` attribute as follows:
/// ```ignore
/// #[async_trait]
/// pub trait Tick: Actor {
///     /// Called whenever a timer might have elapsed.
///     async fn tick(&mut self) -> ActorResult<()>;
/// }
/// ```
///
#[async_trait]
pub trait Tick: Actor {
    /// Called whenever a timer might have elapsed.
    async fn tick(&mut self) -> ActorResult<()>;
}

/// Timers will be in one of these states.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TimerState {
    /// The timer is inactive. This is the default state.
    Inactive,
    /// The timer is configured to tick once, when the deadline
    /// is reached.
    Timeout {
        /// When this timer will tick
        deadline: Instant,
    },
    /// The timer is configured to tick when the deadline is
    /// reached, and to repeat at a set interval.
    Interval {
        /// When this timer will next tick
        deadline: Instant,
        /// Interval between ticks.
        interval: Duration,
    },
}

impl TimerState {
    /// Returns the point in time when this timer will next fire, or
    /// `None` if the timer is currently inactive.
    pub fn deadline(&self) -> Option<Instant> {
        match *self {
            TimerState::Inactive => None,
            TimerState::Timeout { deadline } => Some(deadline),
            TimerState::Interval { deadline, .. } => Some(deadline),
        }
    }
    /// Returns the interval between ticks if the timer is active and set
    /// to repeat.
    pub fn interval(&self) -> Option<Duration> {
        match *self {
            TimerState::Inactive | TimerState::Timeout { .. } => None,
            TimerState::Interval { interval, .. } => Some(interval),
        }
    }
}

impl Default for TimerState {
    fn default() -> Self {
        Self::Inactive
    }
}

#[derive(Debug)]
enum InternalTimerState {
    Inactive,
    Timeout {
        deadline: Instant,
    },
    IntervalWeak {
        addr: WeakAddr<dyn Tick>,
        deadline: Instant,
        interval: Duration,
    },
    IntervalStrong {
        addr: Addr<dyn Tick>,
        deadline: Instant,
        interval: Duration,
    },
}

impl Default for InternalTimerState {
    fn default() -> Self {
        Self::Inactive
    }
}

impl InternalTimerState {
    fn public_state(&self) -> TimerState {
        match *self {
            InternalTimerState::Inactive => TimerState::Inactive,
            InternalTimerState::Timeout { deadline } => TimerState::Timeout { deadline },
            InternalTimerState::IntervalWeak {
                deadline, interval, ..
            }
            | InternalTimerState::IntervalStrong {
                deadline, interval, ..
            } => TimerState::Interval { deadline, interval },
        }
    }
}

/// A timer suitable for use by actors.
#[derive(Debug, Default)]
pub struct Timer<R> {
    runtime: R,
    state: InternalTimerState,
}

impl<R: SupportsTimers> Timer<R> {
    /// Construct a new timer with the provided runtime.
    pub fn new(runtime: R) -> Self {
        Self {
            runtime,
            state: InternalTimerState::Inactive,
        }
    }
    /// Get the state of the timer
    pub fn state(&self) -> TimerState {
        self.state.public_state()
    }
    /// True if this timer is expected to tick in the future.
    pub fn is_active(&self) -> bool {
        self.state() != TimerState::Inactive
    }
    /// Reset the timer to the inactive state.
    pub fn clear(&mut self) {
        self.state = InternalTimerState::Inactive;
    }
    /// Check if the timer has elapsed.
    pub fn tick(&mut self) -> bool {
        match mem::replace(&mut self.state, InternalTimerState::Inactive) {
            InternalTimerState::Inactive => false,
            InternalTimerState::Timeout { deadline } => {
                if deadline <= Instant::now() {
                    true
                } else {
                    self.state = InternalTimerState::Timeout { deadline };
                    false
                }
            }
            InternalTimerState::IntervalWeak {
                deadline,
                interval,
                addr,
            } => {
                if deadline <= Instant::now() {
                    self.set_interval_at_weak_internal(addr, deadline + interval, interval);
                    true
                } else {
                    self.state = InternalTimerState::IntervalWeak {
                        deadline,
                        interval,
                        addr,
                    };
                    false
                }
            }
            InternalTimerState::IntervalStrong {
                deadline,
                interval,
                addr,
            } => {
                if deadline <= Instant::now() {
                    self.set_interval_at_strong_internal(addr, deadline + interval, interval);
                    true
                } else {
                    self.state = InternalTimerState::IntervalStrong {
                        deadline,
                        interval,
                        addr,
                    };
                    false
                }
            }
        }
    }
    fn set_interval_at_weak_internal(
        &mut self,
        addr: WeakAddr<dyn Tick>,
        start: Instant,
        interval: Duration,
    ) {
        let addr2 = addr.clone();
        let delay = self.runtime.delay(start);
        addr.send_fut(async move {
            delay.await;
            send!(addr2.tick());
        });

        self.state = InternalTimerState::IntervalWeak {
            deadline: start,
            interval,
            addr,
        };
    }
    fn set_interval_at_strong_internal(
        &mut self,
        addr: Addr<dyn Tick>,
        start: Instant,
        interval: Duration,
    ) {
        let addr2 = addr.clone();
        let delay = self.runtime.delay(start);
        addr.send_fut(async move {
            delay.await;
            send!(addr2.tick());
        });

        self.state = InternalTimerState::IntervalStrong {
            deadline: start,
            interval,
            addr,
        };
    }
    fn set_timeout_internal<T: Tick + ?Sized>(
        &mut self,
        addr: impl AddrLike<Actor = T>,
        deadline: Instant,
    ) {
        let addr2 = addr.clone();
        let delay = self.runtime.delay(deadline);
        addr.send_fut(async move {
            delay.await;
            send!(addr2.tick());
        });

        self.state = InternalTimerState::Timeout { deadline };
    }
    fn run_with_timeout_internal<
        T: Tick + ?Sized,
        A: AddrLike<Actor = T>,
        F: Future<Output = ()> + Send + 'static,
    >(
        &mut self,
        addr: A,
        deadline: Instant,
        f: impl FnOnce(A) -> F + Send + 'static,
    ) {
        let addr2 = addr.clone();
        let mut delay = self.runtime.delay(deadline).fuse();
        addr.send_fut(async move {
            if select_biased! {
                _ = f(addr2.clone()).fuse() => true,
                _ = delay => false,
            } {
                // Future completed first, so wait for delay too
                delay.await;
            }
            send!(addr2.tick());
        });

        self.state = InternalTimerState::Timeout { deadline };
    }

    /// Configure the timer to tick at a set interval with an initial delay.
    /// The timer will not try to keep the actor alive.
    pub fn set_interval_at_weak<T: Tick>(
        &mut self,
        addr: WeakAddr<T>,
        start: Instant,
        interval: Duration,
    ) {
        self.set_interval_at_weak_internal(upcast!(addr), start, interval);
    }
    /// Configure the timer to tick at a set interval with an initial delay.
    /// The timer will try to keep the actor alive.
    pub fn set_interval_at_strong<T: Tick>(
        &mut self,
        addr: Addr<T>,
        start: Instant,
        interval: Duration,
    ) {
        self.set_interval_at_strong_internal(upcast!(addr), start, interval);
    }
    /// Configure the timer to tick at a set interval, with the initial tick sent immediately.
    /// The timer will not try to keep the actor alive.
    pub fn set_interval_weak<T: Tick>(&mut self, addr: WeakAddr<T>, interval: Duration) {
        self.set_interval_at_weak_internal(upcast!(addr), Instant::now(), interval);
    }
    /// Configure the timer to tick at a set interval, with the initial tick sent immediately.
    /// The timer will try to keep the actor alive.
    pub fn set_interval_strong<T: Tick>(&mut self, addr: Addr<T>, interval: Duration) {
        self.set_interval_at_strong_internal(upcast!(addr), Instant::now(), interval);
    }
    /// Configure the timer to tick once at the specified time.
    /// The timer will not try to keep the actor alive.
    pub fn set_timeout_weak<T: Tick>(&mut self, addr: WeakAddr<T>, deadline: Instant) {
        self.set_timeout_internal(addr, deadline);
    }
    /// Configure the timer to tick once at the specified time.
    /// The timer will try to keep the actor alive until that time.
    pub fn set_timeout_strong<T: Tick>(&mut self, addr: Addr<T>, deadline: Instant) {
        self.set_timeout_internal(addr, deadline);
    }
    /// Configure the timer to tick once after a delay.
    /// The timer will not try to keep the actor alive.
    pub fn set_timeout_for_weak<T: Tick>(&mut self, addr: WeakAddr<T>, duration: Duration) {
        self.set_timeout_internal(addr, Instant::now() + duration);
    }
    /// Configure the timer to tick once after a delay.
    /// The timer will try to keep the actor alive until that time.
    pub fn set_timeout_for_strong<T: Tick>(&mut self, addr: Addr<T>, duration: Duration) {
        self.set_timeout_internal(addr, Instant::now() + duration);
    }
    /// Configure the timer to tick once at the specified time, whilst simultaneously
    /// running a task to completion. If the timeout completes first, the task will
    /// be dropped.
    /// The timer will not try to keep the actor alive.
    pub fn run_with_timeout_weak<T: Tick + ?Sized, F: Future<Output = ()> + Send + 'static>(
        &mut self,
        addr: WeakAddr<T>,
        deadline: Instant,
        f: impl FnOnce(WeakAddr<T>) -> F + Send + 'static,
    ) {
        self.run_with_timeout_internal(addr, deadline, f);
    }
    /// Configure the timer to tick once at the specified time, whilst simultaneously
    /// running a task to completion. If the timeout completes first, the task will
    /// be dropped.
    /// The timer will try to keep the actor alive until that time.
    pub fn run_with_timeout_strong<T: Tick + ?Sized, F: Future<Output = ()> + Send + 'static>(
        &mut self,
        addr: Addr<T>,
        deadline: Instant,
        f: impl FnOnce(Addr<T>) -> F + Send + 'static,
    ) {
        self.run_with_timeout_internal(addr, deadline, f);
    }
    /// Configure the timer to tick once at the specified time, whilst simultaneously
    /// running a task to completion. If the timeout completes first, the task will
    /// be dropped.
    /// The timer will not try to keep the actor alive.
    pub fn run_with_timeout_for_weak<T: Tick + ?Sized, F: Future<Output = ()> + Send + 'static>(
        &mut self,
        addr: WeakAddr<T>,
        duration: Duration,
        f: impl FnOnce(WeakAddr<T>) -> F + Send + 'static,
    ) {
        self.run_with_timeout_internal(addr, Instant::now() + duration, f);
    }
    /// Configure the timer to tick once at the specified time, whilst simultaneously
    /// running a task to completion. If the timeout completes first, the task will
    /// be dropped.
    /// The timer will try to keep the actor alive until that time.
    pub fn run_with_timeout_for_strong<
        T: Tick + ?Sized,
        F: Future<Output = ()> + Send + 'static,
    >(
        &mut self,
        addr: Addr<T>,
        duration: Duration,
        f: impl FnOnce(Addr<T>) -> F + Send + 'static,
    ) {
        self.run_with_timeout_internal(addr, Instant::now() + duration, f);
    }
}
