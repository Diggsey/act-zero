use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::channel::oneshot;

/// Creates a new one-shot channel for sending values across asynchronous tasks.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = oneshot::channel();
    (Sender(tx), Receiver(rx))
}

/// A future for a value that will be provided by another asynchronous task.
///
/// This is created by the [`channel`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct Receiver<T>(oneshot::Receiver<T>);

/// A means of transmitting a single value to another task.
///
/// This is created by the [`channel`] function.
#[derive(Debug)]
pub struct Sender<T>(oneshot::Sender<T>);

#[doc(hidden)]
pub trait SenderExt {
    type Item;
}

impl<T> SenderExt for Sender<T> {
    type Item = T;
}

impl<T> Sender<T> {
    /// Completes this oneshot with a successful result.
    ///
    /// This function will consume `self` and indicate to the other end, the
    /// [`Receiver`](Receiver), that the value provided is the result of the
    /// computation this represents.
    ///
    /// If the value is successfully enqueued for the remote end to receive,
    /// then `Ok(())` is returned. If the receiving end was dropped before
    /// this function was called, however, then `Err` is returned with the value
    /// provided.
    pub fn send(self, t: T) -> Result<(), T> {
        self.0.send(t)
    }

    /// Polls this `Sender` half to detect whether its associated
    /// [`Receiver`](Receiver) with has been dropped.
    ///
    /// # Return values
    ///
    /// If `Ready(())` is returned then the associated `Receiver` has been
    /// dropped, which means any work required for sending should be canceled.
    ///
    /// If `Pending` is returned then the associated `Receiver` is still
    /// alive and may be able to receive a message if sent. The current task,
    /// however, is scheduled to receive a notification if the corresponding
    /// `Receiver` goes away.
    pub fn poll_canceled(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.0.poll_canceled(cx)
    }

    /// Creates a future that resolves when this `Sender`'s corresponding
    /// [`Receiver`](Receiver) half has hung up.
    ///
    /// This is a utility wrapping [`poll_canceled`](Sender::poll_canceled)
    /// to expose a [`Future`](core::future::Future).
    pub fn cancellation(&mut self) -> Cancellation<'_, T> {
        Cancellation(self.0.cancellation())
    }

    /// Tests to see whether this `Sender`'s corresponding `Receiver`
    /// has been dropped.
    ///
    /// Unlike [`poll_canceled`](Sender::poll_canceled), this function does not
    /// enqueue a task for wakeup upon cancellation, but merely reports the
    /// current state, which may be subject to concurrent modification.
    pub fn is_canceled(&self) -> bool {
        self.0.is_canceled()
    }
}

/// A future that resolves when the receiving end of a channel has hung up.
///
/// This is an `.await`-friendly interface around [`poll_canceled`](Sender::poll_canceled).
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct Cancellation<'a, T>(oneshot::Cancellation<'a, T>);

impl<T> Future for Cancellation<'_, T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Error returned from a [`Receiver`](Receiver) when the corresponding
/// [`Sender`](Sender) is dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Canceled;

impl std::fmt::Display for Canceled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "oneshot canceled")
    }
}

impl std::error::Error for Canceled {}

impl From<Canceled> for Box<dyn std::error::Error + Send> {
    fn from(e: Canceled) -> Self {
        Box::new(e)
    }
}

impl<T> Receiver<T> {
    /// Gracefully close this receiver, preventing any subsequent attempts to
    /// send to it.
    ///
    /// Any `send` operation which happens after this method returns is
    /// guaranteed to fail. After calling this method, you can use
    /// [`Receiver::poll`](core::future::Future::poll) to determine whether a
    /// message had previously been sent.
    pub fn close(&mut self) {
        self.0.close()
    }

    /// Attempts to receive a message outside of the context of a task.
    ///
    /// Does not schedule a task wakeup or have any other side effects.
    ///
    /// A return value of `None` must be considered immediately stale (out of
    /// date) unless [`close`](Receiver::close) has been called first.
    ///
    /// Returns an error if the sender was dropped.
    pub fn try_recv(&mut self) -> Result<Option<T>, Canceled> {
        self.0.try_recv().map_err(|_| Canceled)
    }
}

impl<T> Future for Receiver<T> {
    type Output = Result<T, Canceled>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Canceled>> {
        Pin::new(&mut self.0).poll(cx).map_err(|_| Canceled)
    }
}
