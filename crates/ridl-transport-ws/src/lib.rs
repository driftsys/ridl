//! The WebSocket binding of the ridl frame.
//!
//! The [`frame`] module is the header codec of frame specification §11.1. It
//! needs `core` only: no `alloc` and no standard library.
//!
//! With the `server` feature, the crate also serves a [`Backend`] to one
//! WebSocket client: the embedder gives [`Polling`] or [`Scanning`] a port
//! set, and a [`Config`] whose [`Note`]s report what the server drops.

#![cfg_attr(not(feature = "server"), no_std)]

pub mod frame;

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
pub use server::{Backend, Config, LimitTooSmall, Note, Polling, Scanning};

// The server loop that calls these items is not in the crate yet, so outside
// the tests they are unused. Remove the `allow` when that loop uses them.
#[cfg(feature = "server")]
#[allow(dead_code)]
mod ws;
