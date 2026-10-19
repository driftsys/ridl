//! The test catalog and its provider: one interface with a signal, an event,
//! a command, a query and a `fixed`, served by a `ridl-loopback` runtime.

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, Contract};
use ridl_rt::port::{EventSink, Handler, SignalWriter};

/// The test catalog: the unit `test.ws` with a hash of thirty-two 7s.
pub const CATALOG: CatalogRef = CatalogRef {
    name: "test.ws",
    hash: CatalogHash([7; 32]),
};
/// The one interface of the catalog.
pub const IFACE: InterfaceNo = InterfaceNo(1);
/// The signal, which the provider sets to each accepted command's argument.
pub const SIGNAL: Ordinal = Ordinal(1);
/// The event, which the provider raises with each accepted command's argument.
pub const EVENT: Ordinal = Ordinal(2);
/// The command.
pub const COMMAND: Ordinal = Ordinal(3);
/// The query, whose reply adds 1 to each argument byte, wrapping at 256.
pub const QUERY: Ordinal = Ordinal(4);
/// The `fixed`.
pub const FIXED: Ordinal = Ordinal(5);
/// The value the provider provisions for [`FIXED`].
pub const FIXED_VALUE: &[u8] = &[0xf0, 0x0d];

/// The provider's runtime: a loopback on [`CATALOG`] that serves the command
/// and the query, with [`FIXED`] provisioned to [`FIXED_VALUE`].
pub fn provider() -> Loopback {
    let mut rt = Loopback::new(CATALOG);
    rt.serve(IFACE, &[COMMAND, QUERY])
        .expect("the loopback serves");
    rt.provision_fixed(IFACE, FIXED, FIXED_VALUE);
    rt
}

/// Makes one pass over the claims waiting on `rt` and settles each.
///
/// - [`COMMAND`]: settles `Ok(&[])`, then sets [`SIGNAL`] to the argument,
///   commits, and raises [`EVENT`] with the argument.
/// - [`QUERY`]: settles with a reply whose byte `i` is argument byte `i` plus
///   1, wrapping at 256.
pub fn provider_step(rt: &mut Loopback) {
    let mut args = [0u8; 64];
    while let Some(claim) = rt.next_claim(&mut args).expect("a claim fits") {
        let arg = &args[..claim.len];
        match claim.ord {
            COMMAND => {
                rt.settle(claim.id, Ok(&[])).expect("settled");
                rt.set(IFACE, SIGNAL, arg).expect("staged");
                rt.commit();
                rt.raise(IFACE, EVENT, arg, None).expect("raised");
            }
            QUERY => {
                let reply: Vec<u8> = arg.iter().map(|b| b.wrapping_add(1)).collect();
                rt.settle(claim.id, Ok(&reply)).expect("settled");
            }
            _ => {
                let unknown = Err(CallError::Contract(Contract::UnknownInteraction));
                rt.settle(claim.id, unknown).expect("settled");
            }
        }
    }
}
