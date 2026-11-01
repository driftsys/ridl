//! The traits of the generated face: the fixed methods of the consumer and
//! provider faces the Rust backend emits (ADR-0023 decision 7, ADR-0021
//! decision 19), and [`Source`], which the application implements for a
//! streamed reply.
//!
//! A generated `Client` or `Publisher` has two kinds of method. A member
//! method is derived from the interface — one per signal, event, command and
//! query, named after the member — and is an inherent method of the type. A
//! fixed method is the emitter's own — `new`, `next_event`, `with_timeout`,
//! `set_timeout`, `commit` — and is a method of one of the traits here.
//! Rust gives one type one inherent namespace, so a member whose snake case
//! is `new` or `commit` could not sit beside a fixed inherent method of that
//! name (rustc E0592); a trait method lives in the trait's namespace, so the
//! member and the fixed method coexist, and the fixed method stays reachable
//! through the trait's path. The two derived methods a face adds beside a
//! member — `subscribe_<event>` and `invalidate_<signal>` — are methods of
//! two traits the emitter generates inside each interface module,
//! `Subscribe` and `Invalidate`, for the same reason.
//!
//! Which of the two a dot call reaches follows Rust's method probe: it tries
//! the receiver by value, then by `&`, then by `&mut`, and at each step an
//! inherent method before a trait method. A member takes `&self` (a signal
//! read) or `&mut self` (every other member); [`Bind::new`] takes no receiver
//! and is reached by a path call, and every other trait method here takes
//! `&mut self` except [`Timeout::with_timeout`], which takes `self` by value.
//! So the member keeps the dot call for every fixed and derived method but
//! `with_timeout`: beside a member named `withTimeout`,
//! `client.with_timeout(x)` on a blocking client held by value reaches the
//! trait method, because the by-value step comes first, and the consumer
//! reaches the member through the inherent path
//! `blocking::Client::with_timeout(&mut client, x)` (`&client` for a signal
//! read). `with_timeout` takes `self` so that
//! `Client::new(port).with_timeout(t)` stays one expression.
//!
//! A consumer of a generated face writes `use <crate>::<iface>::prelude::*;`
//! for each interface whose face it uses: the generated `prelude` re-exports
//! the traits here that the interface's types implement, and the interface's
//! two generated traits as `_`. rustc reports a prelude as an unused import
//! when the other imported preludes already bring every item it would add,
//! and that import can then be dropped. With the
//! prelude in scope every call site is the one an inherent method had —
//! `Client::new(port)`, `client.next_event()`, `publisher.commit()`. Only
//! when a member of the interface is itself named `new` does
//! `Client::new(port)` resolve to the member, because a path call finds an
//! inherent item first; the consumer then writes
//! `<Client<_> as Bind>::new(port)` or `let c: Client<_> = Bind::new(port)`.
//!
//! [`Timeout`] is under the `std` feature, because its only implementor is
//! the generated `blocking` module, which the emitted crate's `std` feature
//! enables together with this crate's. Every other trait here is
//! unconditional and `no_std`, like the rest of the crate. Nothing here is
//! implemented by a runtime: a runtime implements the [`port`](crate::port)
//! traits, and generated code implements the traits of the fixed methods.
//!
//! [`Source`] is the one trait here that an application implements rather
//! than generated code: a provider's method for a query whose reply is a
//! stream of named elements returns a `Source` of those elements, and the
//! generated `serve` drives it, one element per poll, over the
//! [`StreamHandler`](crate::port::StreamHandler) port. [`Interrupted`] is the
//! one way such a source stops its stream before the end.

use core::task::{Context, Poll};

/// Binds a face to the port it holds: the `new` of a generated `Client`,
/// `blocking::Client` and `Publisher`.
///
/// `Port` is the type the face is generic over — a runtime's handle, or a
/// `&mut` borrow of one — and `new` holds it by value.
///
/// ```
/// use ridl_rt::face::Bind;
///
/// struct Client<P> { port: P }
/// impl<P> Bind for Client<P> {
///     type Port = P;
///     fn new(port: P) -> Self { Client { port } }
/// }
///
/// let client = Client::new(7u32);
/// assert_eq!(client.port, 7);
/// ```
pub trait Bind {
    /// The port the face is built over.
    type Port;

    /// Binds the face to `port`. The port is held by value: pass a handle, or
    /// a `&mut` borrow of one.
    fn new(port: Self::Port) -> Self;
}

/// Takes the next occurrence of a subscribed event: the `next_event` of a
/// generated `Client` and `blocking::Client`.
///
/// `Next` is generic over the borrow of `self`, so one trait serves both
/// clients: the async client's `Next<'a>` is its event future, which borrows
/// the client's port, and the blocking client's is the owned
/// `Result<Option<Event>, ReadError>`. The trait gives the method a namespace
/// of its own and nothing more: `Next` carries no bound, so code generic over
/// `Events` cannot use what `next_event` returns.
///
/// ```
/// use ridl_rt::face::Events;
///
/// struct Client { queue: Vec<u8> }
/// impl Events for Client {
///     type Next<'a> = Option<u8> where Self: 'a;
///     fn next_event(&mut self) -> Option<u8> { self.queue.pop() }
/// }
///
/// let mut client = Client { queue: vec![1] };
/// assert_eq!(client.next_event(), Some(1));
/// assert_eq!(client.next_event(), None);
/// ```
pub trait Events {
    /// What one call of `next_event` returns: a future for the async client,
    /// an owned result for the blocking one.
    type Next<'a>
    where
        Self: 'a;

    /// Takes the next occurrence of any subscribed event of the interface.
    fn next_event(&mut self) -> Self::Next<'_>;
}

/// Bounds every waiting method of a blocking face by a wall-clock timeout:
/// the `with_timeout` and `set_timeout` of a generated `blocking::Client`.
///
/// The timeout is `None` until one of the two sets it; with none, a waiting
/// method returns only with its outcome. `core::time::Duration` is the type
/// `std::time::Duration` re-exports, so a consumer passes either.
///
/// ```
/// use core::time::Duration;
/// use ridl_rt::face::Timeout;
///
/// struct Client { timeout: Option<Duration> }
/// impl Timeout for Client {
///     fn with_timeout(mut self, timeout: Duration) -> Self {
///         self.timeout = Some(timeout);
///         self
///     }
///     fn set_timeout(&mut self, timeout: Option<Duration>) { self.timeout = timeout; }
/// }
///
/// let mut client = Client { timeout: None }.with_timeout(Duration::from_millis(5));
/// assert_eq!(client.timeout, Some(Duration::from_millis(5)));
/// client.set_timeout(None);
/// assert_eq!(client.timeout, None);
/// ```
#[cfg(feature = "std")]
pub trait Timeout: Sized {
    /// Sets the timeout every waiting method of the face is bounded by, and
    /// returns the face.
    fn with_timeout(self, timeout: core::time::Duration) -> Self;

    /// Sets or clears the timeout every waiting method of the face is bounded
    /// by.
    fn set_timeout(&mut self, timeout: Option<core::time::Duration>);
}

/// Publishes every staged signal change: the `commit` of a generated
/// `Publisher`.
///
/// ```
/// use ridl_rt::face::Publish;
///
/// struct Publisher { staged: u8, published: u8 }
/// impl Publish for Publisher {
///     fn commit(&mut self) { self.published = self.staged; }
/// }
///
/// let mut publisher = Publisher { staged: 3, published: 0 };
/// publisher.commit();
/// assert_eq!(publisher.published, 3);
/// ```
pub trait Publish {
    /// Publishes every staged signal change.
    fn commit(&mut self);
}

/// A producer of one stream of elements, pulled one element at a time: what
/// a `Provider` method returns for a query whose reply is a stream of named
/// elements, and what the generated `serve` drives.
///
/// The shape is `core::task`'s: `poll_next` answers `Poll::Ready(Some(Ok(item)))`
/// with the next element, `Poll::Ready(None)` once the stream ended,
/// `Poll::Ready(Some(Err(Interrupted)))` to stop the stream before its end,
/// and `Poll::Pending` when no element is ready yet, after it stored the
/// waker of `cx` to be woken when one is. The pull shape is what lets a
/// provider method stay synchronous on a target without `std`: the method
/// returns the source, and the task that polls `serve` is the one that waits
/// when the stream's window is full (ridl §12.2). After `Ready(None)` or
/// `Ready(Some(Err(_)))` the source is not polled again.
///
/// The trait is object safe, so a driver may hold one as
/// `&mut dyn Source<Item = T>`.
///
/// ```
/// use core::task::{Context, Poll};
/// use ridl_rt::face::{Interrupted, Source};
///
/// struct Countdown(u8);
/// impl Source for Countdown {
///     type Item = u8;
///     fn poll_next(&mut self, _cx: &mut Context<'_>) -> Poll<Option<Result<u8, Interrupted>>> {
///         if self.0 == 0 {
///             return Poll::Ready(None);
///         }
///         self.0 -= 1;
///         Poll::Ready(Some(Ok(self.0)))
///     }
/// }
///
/// let waker = core::task::Waker::noop();
/// let mut cx = Context::from_waker(waker);
/// let mut source = Countdown(2);
/// assert_eq!(source.poll_next(&mut cx), Poll::Ready(Some(Ok(1))));
/// assert_eq!(source.poll_next(&mut cx), Poll::Ready(Some(Ok(0))));
/// assert_eq!(source.poll_next(&mut cx), Poll::Ready(None));
/// ```
pub trait Source {
    /// The element type: the stream's declared element, as the generated
    /// crate names it.
    type Item;

    /// Pulls the next element, under the contract above.
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<Self::Item, Interrupted>>>;
}

/// A [`Source`] stopped its stream before the end. The generated `serve`
/// reports it to the caller as
/// [`Transport::Interrupted`](crate::error::Transport::Interrupted), the
/// outcome a producer stops a stream with (ridl §12).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Interrupted;

#[cfg(test)]
mod tests {
    use super::{Interrupted, Source};
    use core::task::{Context, Poll};

    /// A source over a slice, written as a provider of a generated face
    /// would write one: `core` only, no allocation.
    struct Counter<'a> {
        left: &'a [u8],
        interrupt_at: Option<u8>,
    }

    impl Source for Counter<'_> {
        type Item = u8;

        fn poll_next(&mut self, _cx: &mut Context<'_>) -> Poll<Option<Result<u8, Interrupted>>> {
            let Some((first, rest)) = self.left.split_first() else {
                return Poll::Ready(None);
            };
            self.left = rest;
            if self.interrupt_at == Some(*first) {
                return Poll::Ready(Some(Err(Interrupted)));
            }
            Poll::Ready(Some(Ok(*first)))
        }
    }

    /// Drains `source` into `out` through a trait object, the way a generic
    /// driver with no knowledge of the provider's type does; returns how many
    /// items were written and whether the source ended or interrupted.
    fn drain(
        source: &mut dyn Source<Item = u8>,
        out: &mut [u8],
    ) -> (usize, Result<(), Interrupted>) {
        let waker = core::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        let mut n = 0;
        loop {
            match source.poll_next(&mut cx) {
                Poll::Ready(Some(Ok(item))) => {
                    out[n] = item;
                    n += 1;
                }
                Poll::Ready(Some(Err(Interrupted))) => return (n, Err(Interrupted)),
                Poll::Ready(None) => return (n, Ok(())),
                Poll::Pending => unreachable!("the counter is never pending"),
            }
        }
    }

    #[test]
    fn a_source_is_driven_through_a_trait_object_until_its_end() {
        let mut source = Counter {
            left: &[1, 2, 3],
            interrupt_at: None,
        };
        let mut out = [0u8; 4];
        assert_eq!(drain(&mut source, &mut out), (3, Ok(())));
        assert_eq!(out, [1, 2, 3, 0]);
    }

    #[test]
    fn a_source_that_interrupts_yields_the_items_before_the_interruption() {
        let mut source = Counter {
            left: &[1, 2, 3],
            interrupt_at: Some(3),
        };
        let mut out = [0u8; 4];
        assert_eq!(drain(&mut source, &mut out), (2, Err(Interrupted)));
        assert_eq!(out, [1, 2, 0, 0]);
    }
}
