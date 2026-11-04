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
        let catalog = self.node().catalogs().find(|c| c.name == request.name)?;
        let ports = match self.node.attach(InstanceNo::LISTENER, catalog) {
            Ok(ports) => ports,
            Err(error) => {
                eprintln!("catalog {}: listener attach failed: {error}", request.name);
                return None;
            }
        };
        match Described::new(Scanning(ports), self.table(catalog)?.clone()) {
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
