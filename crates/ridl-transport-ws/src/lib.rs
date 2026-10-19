//! The WebSocket binding of the ridl frame.
//!
//! The [`frame`] module is the header codec of frame specification §11.1. It
//! needs `core` only: no `alloc` and no standard library.

#![cfg_attr(not(feature = "server"), no_std)]

pub mod frame;

// The server loop that calls these items is not in the crate yet, so outside
// the tests they are unused. Remove the `allow` when that loop uses them.
#[cfg(feature = "server")]
#[allow(dead_code)]
mod ws;
