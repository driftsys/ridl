//! Shared construction and catalog selection for the example and its tests.

use std::error::Error;
use std::io::{self, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;

use ridl_descriptor::deployment;
use ridl_loopback::{Attachment, Node};
use ridl_rt::contract::{CatalogRef, InstanceNo};
use ridl_transport_ws::{
    serve_selected, Config, Connection, Described, Ended, MemberTable, RequestedCatalog, Scanning,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type ListeningBackend = Described<Scanning<Attachment>>;

pub struct Options {
    pub descriptor: PathBuf,
    pub machine: String,
    port: u16,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut args = args.into_iter();
        let usage = || {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: debug_gateway <descriptor> <machine> <listen-port>",
            )
        };
        let descriptor = args.next().ok_or_else(usage)?.into();
        let machine = args.next().ok_or_else(usage)?;
        let port = args.next().ok_or_else(usage)?.parse()?;
        if args.next().is_some() {
            return Err(usage().into());
        }
        Ok(Self {
            descriptor,
            machine,
            port,
        })
    }

    pub fn bind(&self) -> io::Result<TcpListener> {
        TcpListener::bind((Ipv4Addr::LOCALHOST, self.port))
    }
}

pub fn announce(listener: &TcpListener, output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "port {}", listener.local_addr()?.port())?;
    output.flush()
}

pub struct Gateway {
    node: Node,
    tables: Vec<(CatalogRef, Arc<MemberTable>)>,
}

impl Gateway {
    pub fn load(options: &Options) -> Result<Self> {
        // Catalog names borrow these bytes for the lifetime of the process.
        // This is the only descriptor allocation leaked by the executable.
        let bytes = Box::leak(std::fs::read(&options.descriptor)?.into_boxed_slice());
        Self::from_descriptor(bytes, &options.machine)
    }

    pub fn from_descriptor(bytes: &'static [u8], machine: &str) -> Result<Self> {
        let node = Node::from_descriptor(bytes, machine)?;
        let view = deployment::verify(bytes)?;
        let invalid = deployment::VerifyError::Invalid;
        let mut tables = Vec::new();
        for embedded in view.catalogs().map_err(invalid)? {
            let embedded = embedded.map_err(invalid)?;
            let name = embedded.name().map_err(invalid)?;
            let hash = embedded.hash().map_err(invalid)?;
            if let Some(catalog) = node.catalogs().find(|c| c.name == name && c.hash.0 == hash) {
                tables.push((
                    *catalog,
                    Arc::new(MemberTable::from_descriptor(
                        embedded.bytes().map_err(invalid)?,
                    )?),
                ));
            }
        }
        Ok(Self { node, tables })
    }

    pub fn node(&self) -> &Node {
        &self.node
    }

    pub fn table(&self, catalog: &CatalogRef) -> Option<&Arc<MemberTable>> {
        self.tables
            .iter()
            .find(|(c, _)| c == catalog)
            .map(|(_, table)| table)
    }

    fn select(&self, request: RequestedCatalog<'_>) -> Option<ListeningBackend> {
        let catalog = self
            .node()
            .catalogs()
            .find(|c| c.name == request.name && c.hash == request.hash)
            .or_else(|| self.node().catalogs().find(|c| c.name == request.name))?;
        let ports = match self.node.attach(InstanceNo::LISTENER, catalog) {
            Ok(ports) => ports,
            Err(error) => {
                eprintln!("catalog {}: listener attach failed: {error}", request.name);
                return None;
            }
        };
        let Some(table) = self.table(catalog) else {
            eprintln!("catalog {}: member table missing", request.name);
            return None;
        };
        match Described::new(Scanning(ports), table.clone()) {
            Ok(backend) => Some(backend),
            Err(error) => {
                eprintln!("catalog {}: member table mismatch: {error}", request.name);
                None
            }
        }
    }

    pub fn serve(&self, conn: impl Connection, config: &Config) -> Ended {
        serve_selected(conn, 2, |request| self.select(request), config)
    }
}

/// Logs accept errors and keeps accepting subsequent connections.
pub fn accepted<T>(connections: impl Iterator<Item = io::Result<T>>) -> impl Iterator<Item = T> {
    connections.filter_map(|conn| match conn {
        Ok(conn) => Some(conn),
        Err(error) => {
            eprintln!("accept: {error}");
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_transport_ws::frame::{Attach, Frame};

    #[test]
    fn missing_member_table_logs_to_stderr() {
        const CHILD: &str = "RIDL_GATEWAY_MISSING_TABLE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["missing_member_table_logs_to_stderr", "--nocapture"])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(output.status.success());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(
                stderr.contains("catalog alpha: member table missing"),
                "{stderr}"
            );
            return;
        }
        let bytes = include_bytes!(
            "../../../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb"
        );
        let mut gateway = Gateway::from_descriptor(bytes, "Local").unwrap();
        let catalog = *gateway
            .node()
            .catalogs()
            .find(|c| c.name == "alpha")
            .unwrap();
        gateway.tables.clear();
        let frame = Frame::Attach(Attach::new(2, 1, catalog.hash, catalog.name).unwrap());
        let mut msg = vec![0; frame.encoded_len()];
        frame.encode(&mut msg).unwrap();
        let mut input = b"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: ridl-ws-v2\r\n\r\n".to_vec();
        input.extend([0x82, 0x80 | msg.len() as u8, 0, 0, 0, 0]);
        input.extend(msg);
        assert_eq!(
            gateway.serve(Memory(io::Cursor::new(input)), &Config::default()),
            Ended::ServerClosed(1000)
        );
    }

    struct Memory(io::Cursor<Vec<u8>>);
    impl io::Read for Memory {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            io::Read::read(&mut self.0, out)
        }
    }
    impl Write for Memory {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Connection for Memory {
        fn set_read_timeout(&self, _: Option<std::time::Duration>) -> io::Result<()> {
            Ok(())
        }
        fn set_write_timeout(&self, _: Option<std::time::Duration>) -> io::Result<()> {
            Ok(())
        }
    }
}
