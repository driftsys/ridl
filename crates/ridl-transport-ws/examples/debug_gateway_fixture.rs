//! Cross-language fixture host. Stdio controls observe the shared node through
//! local consumer ports, independently of the gateway's transport buffers.

#[allow(dead_code)]
#[path = "support/debug_gateway.rs"]
mod gateway;

use gateway::Gateway;
use ridl_loopback::Attachment;
use ridl_rt::contract::{InstanceNo, InterfaceNo, Ordinal};
use ridl_rt::port::{Caller, EventSource, SignalReader};
use ridl_rt::sample::{Cause, Provenance};
use ridl_transport_ws::Config;
use std::error::Error;
use std::io::{self, BufRead, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const DESCRIPTOR: &[u8] =
    include_bytes!("../../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb");
const IFACE: InterfaceNo = InterfaceNo(1);
const SIGNAL: Ordinal = Ordinal(1);
const EVENT: Ordinal = Ordinal(2);
const COMMAND: Ordinal = Ordinal(3);
const QUERY: Ordinal = Ordinal(4);

fn wait<T>(mut poll: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(value) = poll() {
            return value;
        }
        assert!(Instant::now() < until, "local observation deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn control(gateway: &Gateway, ports: &mut Attachment, command: &str) -> String {
    let cat = *gateway
        .node()
        .catalogs()
        .find(|c| c.name == "alpha")
        .unwrap();
    let mut bytes = [0; 8];
    match command {
        "reservation" => {
            assert!(gateway
                .node()
                .attach(InstanceNo::new(1).unwrap(), &cat)
                .is_err());
            "blocked".into()
        }
        "released" => {
            let positive = wait(|| {
                gateway
                    .node()
                    .attach(InstanceNo::new(1).unwrap(), &cat)
                    .ok()
            });
            drop(positive);
            "released".into()
        }
        "signal" | "invalid" => {
            let provenance = if command == "signal" {
                Provenance::Live
            } else {
                Provenance::Invalid(Cause::Declared)
            };
            let sample = wait(|| {
                let s = ports.read(IFACE, SIGNAL, &mut bytes).unwrap();
                (s.provenance == provenance && s.len == 1 && bytes[0] == 42).then_some(s)
            });
            assert_eq!(sample.len, 1);
            format!("{command} {}", bytes[0])
        }
        "event" => {
            let event = wait(|| ports.next(&mut bytes).unwrap());
            assert_eq!((event.iface, event.ord, event.len), (IFACE, EVENT, 1));
            format!("event {}", bytes[0])
        }
        "command" => {
            let call = ports.command(IFACE, COMMAND, &[11], None).unwrap();
            wait(|| ports.ack(call)).unwrap();
            ports.forget(call);
            "command accepted".into()
        }
        "query" => {
            let call = ports.query(IFACE, QUERY, &[13], None).unwrap();
            let len = wait(|| ports.reply(call, &mut bytes).unwrap()).unwrap();
            assert_eq!(len, 1);
            ports.forget(call);
            format!("query {}", bytes[0])
        }
        _ => panic!("unknown fixture control: {command}"),
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let gateway = Arc::new(Gateway::from_descriptor(DESCRIPTOR, "Local")?);
    let catalog = *gateway
        .node()
        .catalogs()
        .find(|c| c.name == "alpha")
        .unwrap();
    // The positive instance is detached before injection starts.
    drop(
        gateway
            .node()
            .attach(InstanceNo::new(1).unwrap(), &catalog)?,
    );
    let mut consumer = gateway.node().attach(InstanceNo::LISTENER, &catalog)?;
    consumer.subscribe(IFACE, &[EVENT]).unwrap();
    let listening = TcpListener::bind("127.0.0.1:0")?;
    let injection = TcpListener::bind("127.0.0.1:0")?;
    println!("port {}", listening.local_addr()?.port());
    println!("inject-port {}", injection.local_addr()?.port());
    io::stdout().flush()?;
    for (listener, injecting) in [(listening, false), (injection, true)] {
        let gateway = gateway.clone();
        thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                conn.set_nonblocking(false).unwrap();
                let gateway = gateway.clone();
                thread::spawn(move || {
                    if injecting {
                        gateway.serve_injection(conn, &Config::default())
                    } else {
                        gateway.serve(conn, &Config::default())
                    }
                });
            }
        });
    }
    for command in io::stdin().lock().lines() {
        println!("{}", control(&gateway, &mut consumer, &command?));
        io::stdout().flush()?;
    }
    Ok(())
}
