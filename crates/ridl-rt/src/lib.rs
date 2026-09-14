//! The library that generated code links and a runtime implements.
//!
//! `ridl-rt` defines, once, the vocabulary that a package generated from ridl
//! and a runtime agree on: identity, time and the envelope, samples, the
//! payload traits, the interaction descriptors, the ports, and the contract and
//! transport errors. It contains no runtime. A runtime is a separate crate that
//! implements the traits of the `port` module (ADR-0020 decision 6), and
//! generated code calls those traits without naming the runtime.
//!
//! The crate is `no_std`, allocates nothing, contains no `unsafe` code, and has
//! no dependency. The cargo features `flatbuffers`, `proto3` and `repr-c` are
//! declared and enable nothing in this version.
//!
//! Every public item lives in one of six modules. Generated code names each
//! item by its full path, for example `ridl_rt::sample::Sample`, and imports
//! none, because several names here — `Duration`, `Handler`, `Kind` — are also
//! names in `core` or in application code.

#![no_std]
#![forbid(unsafe_code)]
pub mod contract;
pub mod encoding;
pub mod error;
pub mod payload;
pub mod port;
pub mod sample;

/// Pins which enums stay `#[non_exhaustive]` under R-11: `Transport`,
/// `ReadError`, `WriteError`, `RaiseError`, `SendError`, `SubscribeError`,
/// `ServeError` and `SettleError`. Each `compile_fail` block below matches
/// every variant of one such enum, with no `_` arm. Matching a
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
/// The same eight matches, each with a `_` arm, compile: every path and every
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
#[cfg(doctest)]
mod exhaustiveness {}
