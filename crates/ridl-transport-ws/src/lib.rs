//! The WebSocket binding of the ridl frame.
//!
//! The [`frame`] module is the header codec of frame specification §11.1. It
//! needs `core` only: no `alloc` and no standard library.

#![cfg_attr(not(feature = "server"), no_std)]

pub mod frame;
