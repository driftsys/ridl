//! A listening debug gateway for an in-process loopback node.
//!
//! Usage: `debug_gateway <descriptor> <machine> <listen-port>`. Port 0 requests
//! an ephemeral port; stdout prints `port <n>` after binding. The example has
//! no access policy beyond binding to 127.0.0.1. It reaches only its own node;
//! components must share this process and node to make their activity visible.
//! There is no connection to an external runtime and no injection listener.

#[path = "support/debug_gateway.rs"]
mod gateway;

use std::error::Error;
use std::io;
use std::sync::Arc;
use std::thread;

use gateway::{Gateway, Options};
use ridl_transport_ws::Config;

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::parse(std::env::args().skip(1))?;
    let gateway = Arc::new(Gateway::load(&options)?);
    let listener = options.bind()?;
    gateway::announce(&listener, &mut io::stdout().lock())?;
    for conn in listener.incoming() {
        let conn = conn?;
        let gateway = gateway.clone();
        thread::spawn(move || gateway.serve(conn, &Config::default()));
    }
    Ok(())
}
