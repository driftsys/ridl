//! The library that generated code links and a runtime implements.
//!
//! `ridl-rt` defines, once, the vocabulary that a package generated from ridl
//! and a runtime agree on: identity, time and the envelope, samples, the
//! payload traits, the interaction descriptors, the ports, and the contract and
//! transport errors. It contains no runtime, but it carries the pure data
//! structures every runtime would otherwise write alone, such as the
//! [`correlate`] call table. A runtime is a separate crate that
//! implements the traits of the `port` module (ADR-0020 decision 6), and
//! generated code calls those traits without naming the runtime.
//!
//! With the `std` feature off — the default — the crate is `no_std` and
//! allocates nothing; it contains no `unsafe` code and has no dependency in any
//! feature combination. The cargo features `flatbuffers`,
//! `proto3` and `repr-c` name the payload encodings. `flatbuffers` enables the
//! [`flatbuffers`] module, the reading and writing a generated
//! `Payload<FlatBuffers>` implementation shares; `proto3` and `repr-c` enable
//! nothing in this version. A fourth feature, `std`, off by default, is not an
//! encoding: it links the standard library and enables the [`task`] module —
//! `block_on`, which waits on a future by parking the thread until a deadline,
//! and `noop_waker` — for a blocking client built over an async one and for a
//! frame loop that polls a future once per frame. `task` is the one module
//! that links the standard library and allocates — one `Arc` per call of either
//! function. Every other module stays `no_std` with the feature on.
//!
//! Every public item lives in one of seven modules, or in one of the two that
//! the `flatbuffers` and `std` features add. Generated code names each
//! item by its full path, for example `ridl_rt::sample::Sample`, and imports
//! none, because several names here — `Duration`, `Handler`, `Kind` — are also
//! names in `core` or in application code.
//!
//! # Where to start
//!
//! Most of this crate is called by generated code, not by an application. An
//! application implements one generated `Provider` trait and calls generated
//! `Client` and `Publisher` methods; those methods call the ports here. So the
//! shortest path in is to read one interaction kind at a time, from the
//! generated side:
//!
//! | To do this                       | The generated face gives you | Over this port                                |
//! | -------------------------------- | ---------------------------- | --------------------------------------------- |
//! | Read a signal                    | `Client::<name>`             | [`port::SignalReader`], returning [`sample::Sample`] |
//! | Publish a signal                 | `Publisher::<name>`, `commit` | [`port::SignalWriter`]                        |
//! | Receive an event                 | `Client::subscribe_<name>`, `Client::next_event` | [`port::EventSource`]     |
//! | Raise an event                   | `Publisher::<name>`          | [`port::EventSink`]                           |
//! | Call a command or a query        | `Client::<name>`, returning a [`port::Correlation`] | [`port::Caller`]       |
//! | Serve a command or a query       | `Provider`, driven by the generated `dispatch` | [`port::Handler`]           |
//! | Read a provisioned constant      | nothing yet — call the port  | [`port::FixedReader`]                         |
//!
//! `docs/technotes/ridl-rt-by-example.md` in this repository walks that table
//! from top to bottom against concrete generated code, introducing each type
//! at the point where the generated code first needs it. The library's own
//! as-built description is `docs/design/ridl-rt.md`.
//!
//! Two properties hold everywhere and are worth knowing before reading any
//! individual item. **No port method waits** — every one returns immediately,
//! and a call's outcome is retrieved separately through a
//! [`port::Correlation`]. A face that waits registers its interest with the
//! [`port::Wakeable`] extension, keyed by a [`port::Interest`], and reads the
//! port again when the runtime wakes it; a runtime that serves a generated
//! async client implements that extension. And **no port names a payload
//! type** — ports carry interface numbers, ordinals and bytes, and the
//! generated binding is what encodes and decodes, through [`payload::Ref`].
//!
//! A call made through the poll face returns a newtype over a
//! [`port::Correlation`], and the runtime keeps the call's outcome until the
//! caller releases it. Until the generated async client lands (story E11.21),
//! whose future forgets its call when it is dropped, a caller that uses the
//! poll face must call [`port::Caller::forget`] on a correlation once it has
//! read the outcome. Otherwise a runtime that bounds how many calls it holds
//! at once refuses every call with [`port::SendError::Busy`] once that bound
//! is reached. Two cases:
//!
//! - **A `Client` built over `&mut port` for the call can forget.** Make the
//!   call, read the outcome, and once the `Client`'s borrow has ended call
//!   `port.forget(c.0)`: the newtype's field is public, and
//!   [`port::Caller::forget`] forwards through `&mut P`.
//! - **A `Client` that owns its port by value cannot forget**, because it has
//!   no accessor for the port, so the runtime's bound limits how many calls it
//!   can make until the async client lands.
//!
//! # How a runtime presents its ports
//!
//! A runtime crate exposes one handle type per port role it implements — a
//! port role is one port trait — rather than one type implementing them all,
//! and it may also offer an aggregate handle covering the port set one
//! interface's face needs. A generated face is built over one value
//! implementing at least the port traits its interface needs: the role handle
//! itself when the face needs exactly one, and an aggregate — the runtime's,
//! or one the application writes over role handles — when it needs more,
//! because a generated `Client` may be bound over `SignalReader`,
//! `EventSource` and `Caller` at once. Either value reaches the face by value
//! or as a `&mut` borrow of itself, because every port trait is implemented
//! for `&mut P` and the `&self`-only traits also for `&P`. In a runtime whose
//! handles are used from more than one thread, a handle whose port traits all
//! take `&self` is `Send + Sync`, because several threads may read one store
//! at once, and a handle carrying a trait with a `&mut self` method is `Send`
//! and need not be `Sync`, because one thread drives each. No port trait here
//! carries `Send` or `Sync` as a supertrait, so a single-threaded `no_std`
//! runtime whose handles use `Cell` or `RefCell` internally is held to
//! neither; a runtime checks its own handles itself, with a compile-time
//! assertion. ADR-0021 decision 12 records this.

#![no_std]
#![forbid(unsafe_code)]
pub mod contract;
pub mod correlate;
pub mod encoding;
pub mod error;
#[cfg(feature = "flatbuffers")]
pub mod flatbuffers;
pub mod payload;
pub mod port;
pub mod sample;
#[cfg(feature = "std")]
pub mod task;

/// Pins which enums stay `#[non_exhaustive]` under R-11: `Transport`,
/// `ReadError`, `WriteError`, `RaiseError`, `SendError`, `SubscribeError`,
/// `ServeError`, `SettleError`, `ClientError` and `ProviderError`. Each
/// `compile_fail` block below matches every variant of one such enum, with
/// no `_` arm. Matching a
/// `#[non_exhaustive]` enum from outside its crate with no `_` arm does not
/// compile, so a block fails until `#[non_exhaustive]` is removed from the
/// enum it names.
///
/// ```compile_fail
/// fn f(x: ridl_rt::error::Transport) {
///     match x {
///         ridl_rt::error::Transport::Timeout => {}
///         ridl_rt::error::Transport::Undelivered => {}
///         ridl_rt::error::Transport::Down => {}
///         ridl_rt::error::Transport::Corrupt => {}
///         ridl_rt::error::Transport::Busy => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::port::ReadError) {
///     match x {
///         ridl_rt::port::ReadError::Short { .. } => {}
///         ridl_rt::port::ReadError::TooFewSamples { .. } => {}
///         ridl_rt::port::ReadError::Contract(_) => {}
///         ridl_rt::port::ReadError::Detached => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::port::WriteError) {
///     match x {
///         ridl_rt::port::WriteError::TooLarge { .. } => {}
///         ridl_rt::port::WriteError::NotOwner => {}
///         ridl_rt::port::WriteError::Contract(_) => {}
///         ridl_rt::port::WriteError::Detached => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::port::RaiseError) {
///     match x {
///         ridl_rt::port::RaiseError::Busy => {}
///         ridl_rt::port::RaiseError::TooLarge { .. } => {}
///         ridl_rt::port::RaiseError::NotOwner => {}
///         ridl_rt::port::RaiseError::Contract(_) => {}
///         ridl_rt::port::RaiseError::Detached => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::port::SendError) {
///     match x {
///         ridl_rt::port::SendError::Busy => {}
///         ridl_rt::port::SendError::TooLarge { .. } => {}
///         ridl_rt::port::SendError::Contract(_) => {}
///         ridl_rt::port::SendError::Detached => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::port::SubscribeError) {
///     match x {
///         ridl_rt::port::SubscribeError::Contract(_) => {}
///         ridl_rt::port::SubscribeError::Detached => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::port::ServeError) {
///     match x {
///         ridl_rt::port::ServeError::Contract(_) => {}
///         ridl_rt::port::ServeError::NotOwner => {}
///         ridl_rt::port::ServeError::Detached => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::port::SettleError) {
///     match x {
///         ridl_rt::port::SettleError::UnknownClaim => {}
///         ridl_rt::port::SettleError::TooLarge { .. } => {}
///         ridl_rt::port::SettleError::Detached => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::error::ClientError) {
///     match x {
///         ridl_rt::error::ClientError::Send(_) => {}
///         ridl_rt::error::ClientError::Call(_) => {}
///         ridl_rt::error::ClientError::Read(_) => {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// fn f(x: ridl_rt::error::ProviderError) {
///     match x {
///         ridl_rt::error::ProviderError::Serve(_) => {}
///         ridl_rt::error::ProviderError::Claim(_) => {}
///     }
/// }
/// ```
///
/// The same ten matches, each with a `_` arm, compile: every path and every
/// variant name above resolves, so a block above fails only because it names
/// no `_` arm against a `#[non_exhaustive]` enum.
///
/// ```
/// fn read_error(x: ridl_rt::port::ReadError) {
///     match x {
///         ridl_rt::port::ReadError::Short { .. } => {}
///         ridl_rt::port::ReadError::TooFewSamples { .. } => {}
///         ridl_rt::port::ReadError::Contract(_) => {}
///         ridl_rt::port::ReadError::Detached => {}
///         _ => {}
///     }
/// }
/// fn write_error(x: ridl_rt::port::WriteError) {
///     match x {
///         ridl_rt::port::WriteError::TooLarge { .. } => {}
///         ridl_rt::port::WriteError::NotOwner => {}
///         ridl_rt::port::WriteError::Contract(_) => {}
///         ridl_rt::port::WriteError::Detached => {}
///         _ => {}
///     }
/// }
/// fn raise_error(x: ridl_rt::port::RaiseError) {
///     match x {
///         ridl_rt::port::RaiseError::Busy => {}
///         ridl_rt::port::RaiseError::TooLarge { .. } => {}
///         ridl_rt::port::RaiseError::NotOwner => {}
///         ridl_rt::port::RaiseError::Contract(_) => {}
///         ridl_rt::port::RaiseError::Detached => {}
///         _ => {}
///     }
/// }
/// fn send_error(x: ridl_rt::port::SendError) {
///     match x {
///         ridl_rt::port::SendError::Busy => {}
///         ridl_rt::port::SendError::TooLarge { .. } => {}
///         ridl_rt::port::SendError::Contract(_) => {}
///         ridl_rt::port::SendError::Detached => {}
///         _ => {}
///     }
/// }
/// fn subscribe_error(x: ridl_rt::port::SubscribeError) {
///     match x {
///         ridl_rt::port::SubscribeError::Contract(_) => {}
///         ridl_rt::port::SubscribeError::Detached => {}
///         _ => {}
///     }
/// }
/// fn serve_error(x: ridl_rt::port::ServeError) {
///     match x {
///         ridl_rt::port::ServeError::Contract(_) => {}
///         ridl_rt::port::ServeError::NotOwner => {}
///         ridl_rt::port::ServeError::Detached => {}
///         _ => {}
///     }
/// }
/// fn settle_error(x: ridl_rt::port::SettleError) {
///     match x {
///         ridl_rt::port::SettleError::UnknownClaim => {}
///         ridl_rt::port::SettleError::TooLarge { .. } => {}
///         ridl_rt::port::SettleError::Detached => {}
///         _ => {}
///     }
/// }
/// fn transport(x: ridl_rt::error::Transport) {
///     match x {
///         ridl_rt::error::Transport::Timeout => {}
///         ridl_rt::error::Transport::Undelivered => {}
///         ridl_rt::error::Transport::Down => {}
///         ridl_rt::error::Transport::Corrupt => {}
///         ridl_rt::error::Transport::Busy => {}
///         _ => {}
///     }
/// }
/// fn client_error(x: ridl_rt::error::ClientError) {
///     match x {
///         ridl_rt::error::ClientError::Send(_) => {}
///         ridl_rt::error::ClientError::Call(_) => {}
///         ridl_rt::error::ClientError::Read(_) => {}
///         _ => {}
///     }
/// }
/// fn provider_error(x: ridl_rt::error::ProviderError) {
///     match x {
///         ridl_rt::error::ProviderError::Serve(_) => {}
///         ridl_rt::error::ProviderError::Claim(_) => {}
///         _ => {}
///     }
/// }
/// ```
///
/// `Contract` and `CallError` stay exhaustive under R-11: a match naming
/// every variant, with no `_` arm, compiles.
///
/// ```
/// fn contract(x: ridl_rt::error::Contract) {
///     match x {
///         ridl_rt::error::Contract::InvalidValue(_) => {}
///         ridl_rt::error::Contract::PreconditionFailed => {}
///         ridl_rt::error::Contract::ContractBroken => {}
///         ridl_rt::error::Contract::UnknownInteraction => {}
///     }
/// }
/// fn call_error(x: ridl_rt::error::CallError) {
///     match x {
///         ridl_rt::error::CallError::Contract(_) => {}
///         ridl_rt::error::CallError::Transport(_) => {}
///     }
/// }
/// ```
///
/// `port::Interest` is exhaustive too, because a runtime must handle every key
/// (ADR-0021 decision 13): a match naming every key, with no `_` arm, compiles.
///
/// ```
/// fn interest(x: ridl_rt::port::Interest) {
///     match x {
///         ridl_rt::port::Interest::Outcome(_) => {}
///         ridl_rt::port::Interest::Slot => {}
///         ridl_rt::port::Interest::Event(_) => {}
///         ridl_rt::port::Interest::Claim(_) => {}
///     }
/// }
/// ```
#[cfg(doctest)]
mod exhaustiveness {}
