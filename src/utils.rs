use std::future::Future;

use futures::channel::oneshot;
use futures::future::FutureExt;

/// A future which completes upon termination of an actor.
#[derive(Debug)]
pub struct Termination(pub(crate) oneshot::Receiver<()>);

impl Future for Termination {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.0.poll_unpin(cx).map(|_| ())
    }
}
