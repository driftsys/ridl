//! The WebSocket binding of the ridl frame (frame specification §11.1).
//!
//! The [`frame`] module is the header codec of frame specification §11.1. It
//! needs `core` only: no `alloc` and no standard library.
//!
//! With the `server` feature, which is on by default, the crate also serves a
//! [`Backend`] to one WebSocket client per connection. [`serve`] answers the
//! opening handshake on a [`Connection`] the embedder has accepted, runs the
//! session until it ends, and returns why, as an [`Ended`]. The embedder gives
//! [`Polling`] or [`Scanning`] a port set, and a [`Config`] whose [`Note`]s
//! report what the server drops.
//!
//! # Embedding the server
//!
//! The embedder binds the listener, accepts each connection, applies its
//! access policy, chooses the thread, and builds one backend value per
//! connection. Here each connection gets its own thread and its own aggregate
//! handle on one `ridl-loopback` store:
//!
// The example uses the `server` API, so it is compiled only when that feature
// is on.
#![cfg_attr(feature = "server", doc = "```no_run")]
#![cfg_attr(not(feature = "server"), doc = "```ignore")]
//! use std::net::TcpListener;
//! use std::thread;
//!
//! use ridl_loopback::Loopback;
//! use ridl_rt::contract::{CatalogHash, CatalogRef};
//! use ridl_transport_ws::{serve, Config, Scanning};
//!
//! const CATALOG: CatalogRef = CatalogRef {
//!     name: "veh.cabin",
//!     hash: CatalogHash([0u8; 32]),
//! };
//!
//! fn main() -> std::io::Result<()> {
//!     let rt = Loopback::new(CATALOG);
//!     let config = Config::default();
//!     let listener = TcpListener::bind("127.0.0.1:9000")?;
//!     for stream in listener.incoming() {
//!         let stream = stream?;
//!         // The embedder's access policy runs here, before `serve`.
//!         let backend = Scanning(rt.attach());
//!         let config = config.clone();
//!         thread::spawn(move || {
//!             let ended = serve(stream, backend, &config);
//!             eprintln!("session ended: {ended:?}");
//!         });
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # What the crate does not do
//!
//! - It binds, listens, accepts, connects and spawns nothing: the embedder
//!   owns the sockets and the threads.
//! - It has no TLS. A `wss://` deployment ends TLS in front of the server, or
//!   gives [`serve`] a TLS stream that implements [`Connection`].
//! - It authenticates no one. The path and the query of the upgrade request
//!   are ignored, and so is every header other than the ones the handshake
//!   reads (`Host`, `Upgrade`, `Connection`, `Sec-WebSocket-Key`,
//!   `Sec-WebSocket-Version` and `Sec-WebSocket-Protocol`; §11.1 item 1, and
//!   `Origin`, which it gives to the check that
//!   [`Config::with_origin_check`] sets): the access policy is the
//!   embedder's, and runs before [`serve`].
//! - It checks no `Origin` unless the embedder sets a check. Any web page a
//!   browser loads can open a WebSocket to a local address, such as
//!   `ws://127.0.0.1:9000`, and the embedder's access policy sees the same
//!   peer address for that page as for a trusted one. An embedder whose
//!   server a browser reaches should set [`Config::with_origin_check`] to
//!   accept only its own origins.
//! - It reads a catalog descriptor only with the `descriptor` feature
//!   (`MemberTable::from_descriptor`), and does no I/O: the embedder reads
//!   `<unit>.catalog.binfb`, written by `ridl build --emit catalog`, or embeds
//!   it with `include_bytes!`. An `attach` is
//!   accepted on a match
//!   of the unit name and of the catalog hash, or of a compatible earlier hash
//!   the [`MemberTable`] lists; without a table, on an exact match. Without a
//!   table the server checks a frame's `kind` against the form, not against the
//!   catalog; with one ([`Described`]) it refuses an interaction the catalog
//!   does not hold.

#![cfg_attr(not(feature = "server"), no_std)]

pub mod frame;

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "descriptor")]
pub use server::DescriptorError;
#[cfg(feature = "server")]
pub use server::{
    serve, Backend, Config, Connection, Described, Ended, LimitTooSmall, MemberTable, Note,
    Polling, RetainedTooSmall, Scanning, TableMismatch,
};

#[cfg(feature = "server")]
mod ws;
