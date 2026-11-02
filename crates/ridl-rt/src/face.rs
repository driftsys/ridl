//! The traits of the generated face: the fixed methods of the consumer and
//! provider faces the Rust backend emits (ADR-0023 decision 7, ADR-0021
//! decision 19), and the stream traits an application implements for a
//! streamed reply or parameter.
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
//! An application implements [`Source`], [`ByteSource`], [`Consumer`] and
//! [`ByteConsumer`] rather than generated code: a provider's method for a
//! query whose reply is a stream of named elements returns a `Source` of
//! those elements, and the
//! generated `serve` drives it, one element per poll, over the
//! [`StreamHandler`](crate::port::StreamHandler) port. [`Interrupted`] is the
//! one way such a source stops its stream before the end. [`ByteSource`]
//! produces bytes or text; [`Consumer`] and [`ByteConsumer`] receive a
//! streamed parameter of named elements or bytes and text, respectively.

use core::task::{Context, Poll};

use crate::error::CallError;

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

/// A producer of bytes or text that the generated `serve` reads into a buffer.
///
/// The driver supplies a nonempty `buf`. `poll_read` writes at most
/// `buf.len()` bytes and answers their count. `Ready(Ok(0))` is the end; the
/// source is not polled again after the end or `Ready(Err(Interrupted))`.
/// `Pending` takes no bytes and stores the waker of `cx` to wake when bytes
/// or the end become available.
///
/// The trait is object safe and uses only `core`.
///
/// ```
/// use core::task::{Context, Poll};
/// use ridl_rt::face::{ByteSource, Interrupted};
///
/// struct Bytes<'a>(&'a [u8]);
/// impl ByteSource for Bytes<'_> {
///     fn poll_read(&mut self, _cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, Interrupted>> {
///         let n = buf.len().min(self.0.len());
///         buf[..n].copy_from_slice(&self.0[..n]);
///         self.0 = &self.0[n..];
///         Poll::Ready(Ok(n))
///     }
/// }
///
/// let mut source = Bytes(b"abc");
/// let mut cx = Context::from_waker(core::task::Waker::noop());
/// let mut buf = [0; 4];
/// assert_eq!(source.poll_read(&mut cx, &mut buf), Poll::Ready(Ok(3)));
/// assert_eq!(&buf[..3], b"abc");
/// assert_eq!(source.poll_read(&mut cx, &mut buf), Poll::Ready(Ok(0)));
/// ```
pub trait ByteSource {
    /// Reads bytes under the contract above; `Err(Interrupted)` stops the
    /// stream before its end.
    fn poll_read(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Interrupted>>;
}

/// A receiver of named elements that the generated `serve` feeds.
///
/// The driver waits for readiness before accepting each checked element.
/// The stream terminates with exactly one of `end` or `failed`; the driver
/// calls no method after that notification. The trait is object safe.
pub trait Consumer {
    /// The stream's declared element type in the generated crate.
    type Item;

    /// Answers `Ready(())` when one element can be accepted. `Pending`
    /// stores the waker of `cx` to wake when the consumer becomes ready.
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<()>;

    /// Takes one checked element after `poll_ready` answered `Ready(())`.
    fn accept(&mut self, item: Self::Item);

    /// The generated `serve` calls this when the producer ends the stream.
    fn end(&mut self);

    /// The generated `serve` calls this when the stream fails, including an
    /// interruption or a refusal by its own check of a received element.
    fn failed(&mut self, e: CallError);
}

/// A receiver of bytes or text that the generated `serve` feeds.
///
/// The driver offers any unconsumed suffix again before taking another
/// chunk. The stream terminates with exactly one of `end` or `failed`,
/// after which no method is called. The trait is object safe.
pub trait ByteConsumer {
    /// Takes a prefix of `buf` and answers its length, at most `buf.len()`
    /// and at least 1 when `buf` is nonempty. `Pending` takes nothing and
    /// stores the waker of `cx` to wake when bytes can be taken.
    fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<usize>;

    /// The generated `serve` calls this when the producer ends the stream,
    /// after every byte of the last chunk has been taken.
    fn end(&mut self);

    /// The generated `serve` calls this when the stream fails, including an
    /// interruption or a refusal by its own check of a received chunk.
    fn failed(&mut self, e: CallError);
}

/// A [`Source`] or [`ByteSource`] stopped its stream before the end. The
/// generated `serve` reports it to the caller as
/// [`Transport::Interrupted`](crate::error::Transport::Interrupted), the
/// outcome a producer stops a stream with (ridl §12).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Interrupted;

#[cfg(test)]
mod tests {
    use super::{ByteConsumer, ByteSource, Consumer, Interrupted, Source};
    use crate::error::CallError;
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

    struct Bytes<'a>(&'a [u8]);

    impl ByteSource for Bytes<'_> {
        fn poll_read(
            &mut self,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<usize, Interrupted>> {
            let n = self.0.len().min(buf.len());
            buf[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Poll::Ready(Ok(n))
        }
    }

    #[test]
    fn a_byte_source_is_read_until_ok_zero() {
        let mut source = Bytes(b"abcdef");
        let source: &mut dyn ByteSource = &mut source;
        let mut cx = Context::from_waker(core::task::Waker::noop());
        let mut buf = [0; 4];
        assert_eq!(source.poll_read(&mut cx, &mut buf), Poll::Ready(Ok(4)));
        assert_eq!(&buf, b"abcd");
        assert_eq!(source.poll_read(&mut cx, &mut buf), Poll::Ready(Ok(2)));
        assert_eq!(&buf[..2], b"ef");
        assert_eq!(source.poll_read(&mut cx, &mut buf), Poll::Ready(Ok(0)));
    }

    #[derive(Default)]
    struct Receiver {
        bytes: [u8; 8],
        len: usize,
        ended: bool,
    }

    impl Consumer for Receiver {
        type Item = u8;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<()> {
            Poll::Ready(())
        }

        fn accept(&mut self, item: u8) {
            assert!(!self.ended);
            self.bytes[self.len] = item;
            self.len += 1;
        }

        fn end(&mut self) {
            self.ended = true;
        }

        fn failed(&mut self, _e: CallError) {
            panic!("the test stream does not fail");
        }
    }

    #[test]
    fn a_consumer_is_fed_until_its_end() {
        let mut receiver = Receiver::default();
        let consumer: &mut dyn Consumer<Item = u8> = &mut receiver;
        let mut cx = Context::from_waker(core::task::Waker::noop());
        for item in [1, 2, 3] {
            assert_eq!(consumer.poll_ready(&mut cx), Poll::Ready(()));
            consumer.accept(item);
        }
        consumer.end();
        assert_eq!(&receiver.bytes[..receiver.len], &[1, 2, 3]);
        assert!(receiver.ended);
    }

    impl ByteConsumer for Receiver {
        fn poll_write(&mut self, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<usize> {
            assert!(!self.ended);
            let n = buf.len().min(3);
            self.bytes[self.len..self.len + n].copy_from_slice(&buf[..n]);
            self.len += n;
            Poll::Ready(n)
        }

        fn end(&mut self) {
            self.ended = true;
        }

        fn failed(&mut self, _e: CallError) {
            panic!("the test stream does not fail");
        }
    }

    fn feed_bytes(consumer: &mut dyn ByteConsumer, mut bytes: &[u8]) {
        let mut cx = Context::from_waker(core::task::Waker::noop());
        while !bytes.is_empty() {
            match consumer.poll_write(&mut cx, bytes) {
                Poll::Ready(n) => {
                    assert!((1..=bytes.len()).contains(&n));
                    bytes = &bytes[n..];
                }
                Poll::Pending => unreachable!("the test consumer is never pending"),
            }
        }
        consumer.end();
    }

    #[test]
    fn a_byte_consumer_that_takes_part_of_a_buffer_is_offered_the_rest() {
        let mut receiver = Receiver::default();
        feed_bytes(&mut receiver, b"abcdefgh");
        assert_eq!(&receiver.bytes[..receiver.len], b"abcdefgh");
        assert!(receiver.ended);
    }
}
