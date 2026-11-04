//! A listening and injecting debug gateway for an in-process loopback node.
//!
//! Usage: `debug_gateway <descriptor> <machine> <listen-port> [inject-port]`.
//! Port 0 requests an ephemeral port; stdout prints `port <n>` after binding
//! the requested ports and `inject-port <n>` when injection is enabled.
//! The example has no access policy beyond binding to 127.0.0.1. It reaches
//! only its own node; components must share this process and node to make their
//! activity visible. There is no connection to an external runtime. Injection
//! is opt-in.

#[path = "support/debug_gateway.rs"]
mod gateway;

use std::error::Error;
use std::io;

fn main() -> Result<(), Box<dyn Error>> {
    let options = gateway::Options::parse(std::env::args().skip(1))?;
    gateway::run(options, &mut io::stdout().lock())
}
