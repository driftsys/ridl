//! The server: the backend trait, the configuration, and the session core
//! that turns decoded frames and backend samples into frames to send.

mod backend;
#[allow(dead_code)]
mod calls;
mod config;
// `serve`, the loop that drives a session over a socket, is not in the crate
// yet, so outside the tests the session and its calls are unused. Remove both
// `allow`s when that loop uses them.
#[allow(dead_code)]
mod session;

pub use backend::{Backend, Polling, Scanning};
pub use config::{Config, LimitTooSmall, Note};
