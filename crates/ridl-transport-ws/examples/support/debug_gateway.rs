//! Shared construction and catalog selection for the example and its tests.

use std::error::Error;
use std::io::{self, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;

use ridl_descriptor::deployment;
use ridl_loopback::{AttachError, Attachment, Node};
use ridl_rt::contract::{CatalogRef, InstanceNo};
use ridl_transport_ws::{
    serve_provided_selected, serve_selected, Config, Connection, Described, Ended, MemberTable,
    Providing, RequestedCatalog, Scanning, SelectionError,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type ListeningBackend = Described<Scanning<Attachment>>;

pub struct Options {
    pub descriptor: PathBuf,
    pub machine: String,
    port: u16,
    inject_port: Option<u16>,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut args = args.into_iter();
        let usage = || {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: debug_gateway <descriptor> <machine> <listen-port> [inject-port]",
            )
        };
        let descriptor = args.next().ok_or_else(usage)?.into();
        let machine = args.next().ok_or_else(usage)?;
        let port = args.next().ok_or_else(usage)?.parse()?;
        let inject_port = args.next().map(|p| p.parse()).transpose()?;
        if args.next().is_some() {
            return Err(usage().into());
        }
        Ok(Self {
            descriptor,
            machine,
            port,
            inject_port,
        })
    }

    pub fn bind_injection(&self) -> io::Result<Option<TcpListener>> {
        self.inject_port
            .map(|port| TcpListener::bind((Ipv4Addr::LOCALHOST, port)))
            .transpose()
    }

    pub fn bind(&self) -> io::Result<TcpListener> {
        TcpListener::bind((Ipv4Addr::LOCALHOST, self.port))
    }
}

pub fn announce(listener: &TcpListener, output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "port {}", listener.local_addr()?.port())?;
    output.flush()
}

/// Runs the executable's listener wiring after both requested binds succeed.
pub fn run(options: Options, output: &mut impl Write) -> Result<()> {
    let gateway = Arc::new(Gateway::load(&options)?);
    let listener = options.bind()?;
    let injection = options.bind_injection()?;
    announce(&listener, output)?;
    if let Some(injection) = injection {
        writeln!(output, "inject-port {}", injection.local_addr()?.port())?;
        output.flush()?;
        let gateway = gateway.clone();
        std::thread::spawn(move || {
            for conn in accepted(injection.incoming()) {
                let gateway = gateway.clone();
                std::thread::spawn(move || gateway.serve_injection(conn, &Config::default()));
            }
        });
    }
    for conn in accepted(listener.incoming()) {
        let gateway = gateway.clone();
        std::thread::spawn(move || gateway.serve(conn, &Config::default()));
    }
    Ok(())
}

struct Injection {
    path: String,
    who: InstanceNo,
    catalog: CatalogRef,
    table: MemberTable,
}

pub struct Gateway {
    node: Node,
    injections: Vec<Injection>,
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
        let mut injections = Vec::new();
        for instance in view.instances().map_err(invalid)? {
            let instance = instance.map_err(invalid)?;
            if instance.machine().map_err(invalid)? != machine {
                continue;
            }
            let offers = instance
                .offers()
                .map_err(invalid)?
                .into_iter()
                .map(|offer| {
                    let offer = offer.map_err(invalid)?;
                    Ok((
                        offer.catalog().map_err(invalid)?,
                        offer.number().map_err(invalid)?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            for (catalog, full) in &tables {
                let mut table = MemberTable::new(catalog.name, catalog.hash);
                for (target, max, limit) in full.iter() {
                    if !offers.contains(&(catalog.name, target.iface.0)) {
                        continue;
                    }
                    table.insert(target.iface, target.ord, target.kind, max);
                    if full.reply_stream(target.iface, target.ord) {
                        table.set_reply_stream(target.iface, target.ord);
                    }
                    if let Some(limit) = limit {
                        table.set_limit(target.iface, target.ord, limit);
                    }
                }
                if !offers.iter().any(|(name, _)| *name == catalog.name) {
                    continue;
                }
                injections.push(Injection {
                    path: format!(
                        "/{}/{}/{}",
                        instance.component().map_err(invalid)?,
                        instance.instance().map_err(invalid)?,
                        catalog.name
                    ),
                    who: InstanceNo::new(instance.number().map_err(invalid)?)
                        .expect("verified positive instance"),
                    catalog: *catalog,
                    table,
                });
            }
        }
        Ok(Self {
            node,
            tables,
            injections,
        })
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

    fn select_injection(
        &self,
        path: &str,
    ) -> std::result::Result<Providing<Attachment>, SelectionError> {
        let segments: Vec<_> = path.split('/').collect();
        if segments.len() != 4
            || !segments[0].is_empty()
            || segments[1..].iter().any(|s| s.is_empty())
            || path.contains(['?', '%', '#'])
        {
            return Err(SelectionError::NotFound);
        }
        let selected = self
            .injections
            .iter()
            .find(|i| i.path == path)
            .ok_or(SelectionError::NotFound)?;
        let ports = self
            .node
            .attach(
                selected.who.twin().expect("positive instance"),
                &selected.catalog,
            )
            .map_err(|error| match error {
                AttachError::IdentityConflict => SelectionError::Conflict,
                _ => SelectionError::NotFound,
            })?;
        Providing::new(ports, selected.table.clone()).map_err(|_| SelectionError::NotFound)
    }

    pub fn serve_injection(&self, conn: impl Connection, config: &Config) -> Ended {
        serve_provided_selected(conn, |path| self.select_injection(path), config)
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
    fn injection_table_preserves_bounds_and_limits() {
        use ridl_rt::contract::{InterfaceNo, Kind, Ordinal};
        use ridl_transport_ws::ProvidedBackend;
        use std::time::Duration;
        let bytes = include_bytes!(
            "../../../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb"
        );
        let mut descriptor =
            deployment::Deployment::try_from(deployment::verify(bytes).unwrap()).unwrap();
        let embedded = descriptor
            .catalogs
            .iter_mut()
            .find(|c| c.name == "alpha")
            .unwrap();
        let mut catalog =
            ridl_descriptor::Catalog::try_from(ridl_descriptor::verify(&embedded.bytes).unwrap())
                .unwrap();
        for interface in &mut catalog.interfaces {
            for member in &mut interface.members {
                member.limit = Some(17 + member.ordinal);
            }
        }
        embedded.bytes = ridl_descriptor::finish(&catalog);
        let bytes = Box::leak(deployment::finish(&descriptor).into_boxed_slice());
        let gateway = Gateway::from_descriptor(bytes, "Local").unwrap();
        let backend = gateway.select_injection("/alpha.First/Unit/alpha").unwrap();
        let table = backend.members();
        for (ordinal, kind) in [
            (1, Kind::Signal),
            (2, Kind::Event),
            (3, Kind::Command),
            (4, Kind::Query),
        ] {
            assert_eq!(
                table.member(InterfaceNo(1), Ordinal(ordinal)),
                Some((kind, Some(Duration::from_millis(100))))
            );
            assert_eq!(
                table.limit(InterfaceNo(1), Ordinal(ordinal)),
                Some(17 + ordinal)
            );
        }
    }

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
