//! Support for `tests/scenario.rs`: the test catalog and its provider, the
//! scenario, and a test-only WebSocket client. None of it is published: the
//! package leaves out `tests/`.

pub mod client;
pub mod provider;
pub mod scenario;
