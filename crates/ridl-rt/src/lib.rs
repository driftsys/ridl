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
pub mod sample;
