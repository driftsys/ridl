//! The traits a generated face implements: the fixed methods of the
//! consumer and provider faces the Rust backend emits (ADR-0023 decision 7,
//! ADR-0021 decision 19).
//!
//! A generated `Client` or `Publisher` has two kinds of method. A member
//! method is derived from the interface — one per signal, event, command and
//! query, named after the member — and is an inherent method of the type. A
//! fixed method is the emitter's own — `new`, `next_event`, `with_timeout`,
//! `set_timeout`, `commit` — and is a method of one of the four traits here.
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
//! read) or `&mut self` (every other member), and so does every trait method
//! here except [`Timeout::with_timeout`], which takes `self` by value. So the
//! member keeps the dot call for every fixed and derived method but
//! `with_timeout`: beside a member named `withTimeout`,
//! `client.with_timeout(x)` on a blocking client held by value reaches the
//! trait method, because the by-value step comes first, and the consumer
//! reaches the member through the inherent path
//! `blocking::Client::with_timeout(&mut client, x)` (`&client` for a signal
//! read). `with_timeout` takes `self` so that
//! `Client::new(port).with_timeout(t)` stays one expression.
//!
//! A consumer of a generated face writes `use <crate>::<iface>::prelude::*;`:
//! the generated `prelude` re-exports the traits here that the module's
//! types implement, and the two generated traits as `_`. One prelude puts the
//! traits here in scope for every interface of the crate; a further
//! interface's prelude is needed only for that interface's own `Subscribe`
//! and `Invalidate`, and one that adds nothing is an unused import. With the
//! prelude in scope every call site is the one an inherent method had —
//! `Client::new(port)`, `client.next_event()`, `publisher.commit()`. Only
//! when a member of the interface is itself named `new` does
//! `Client::new(port)` resolve to the member, because a path call finds an
//! inherent item first; the consumer then writes
//! `<Client<_> as Bind>::new(port)` or `let c: Client<_> = Bind::new(port)`.
//!
//! [`Timeout`] is under the `std` feature, because its only implementor is
//! the generated `blocking` module, which the emitted crate's `std` feature
//! enables together with this crate's. The other three traits are
//! unconditional and `no_std`, like the rest of the crate. Nothing here is
//! implemented by a runtime: a runtime implements the [`port`](crate::port)
//! traits, and generated code implements these.

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
