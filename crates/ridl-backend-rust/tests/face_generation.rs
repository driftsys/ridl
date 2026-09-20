//! The generated interaction face over the interaction-face fixture (Lane M
//! stage M3, Task 2). Asserts that `generate_face` emits, per interface, a
//! `Client` bound by exactly the ports that interface needs, a `Publisher`
//! over the writer ports, and a `Provider` trait with the settled method
//! signatures.
//!
//! These assertions read the generated source. The face is not compiled here
//! because nothing in the crate can compile it yet: the checked-in generated
//! file and its `include!` are Task 5's, and `ridl-rt` is not yet a
//! dev-dependency of this crate. Task 5 adds the compiled proof, including the
//! minimal `Attached + SignalReader` port that constructs the signal-only
//! interface's `Client` (design §6, RA-19). What this file can pin today is
//! the bound list itself, which is the text that proof would exercise.

use ridl_backend_rust::{generate, generate_face};

#[path = "support/ir.rs"]
mod ir;

/// Whitespace-stripped copy, so an assertion is robust to how prettyplease
/// spaces and wraps a token stream.
fn dense(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The source of one generated top-level `pub mod`, so a per-interface
/// assertion cannot be satisfied by another interface's module.
fn module(source: &str, name: &str) -> String {
    let header = format!("pub mod {name} {{");
    let start = source.find(&header).unwrap_or_else(|| {
        panic!("generated source has no `{header}`:\n{source}");
    });
    let rest = &source[start..];
    let end = rest[1..]
        .find("\npub mod ")
        .map_or(rest.len(), |index| index + 1);
    rest[..end].to_string()
}

/// The slice of `source` from `from` up to `to`, so an assertion about one
/// generated method is not satisfied by another method's body.
fn between(source: &str, from: &str, to: &str) -> String {
    let start = source
        .find(from)
        .unwrap_or_else(|| panic!("generated source has no `{from}`"));
    let rest = &source[start..];
    let end = rest
        .find(to)
        .unwrap_or_else(|| panic!("generated source has no `{to}` after `{from}`"));
    rest[..end].to_string()
}

fn face() -> String {
    let package = ir::compile_fixture("interaction_face.ridl");
    generate_face(&package).expect("generate_face").rust_source
}

#[test]
fn each_interface_gets_its_own_face_module() {
    let source = face();
    assert!(source.contains("pub mod cabin {"), "cabin face module");
    assert!(source.contains("pub mod horn {"), "horn face module");
}

#[test]
fn the_emitter_writes_no_inner_attribute() {
    // An inner attribute inside an `include!`d file is a hard error, so the
    // emitter must never write one (design §7).
    assert!(
        !face().contains("#!["),
        "generated source has an inner attribute"
    );
}

#[test]
fn the_client_is_bound_by_every_port_its_interface_needs() {
    let d = dense(&module(&face(), "cabin"));

    // A signal needs SignalReader, an event needs EventSource, a command and
    // a query need Caller.
    assert!(
        d.contains(
            "pubstructClient<P:::ridl_rt::port::SignalReader+::ridl_rt::port::EventSource\
             +::ridl_rt::port::Caller"
        ),
        "Cabin client port bounds",
    );
    // ADR-0023 decision 5: the face holds its port by value and has no
    // lifetime parameter, so `Client::new(&mut port)` infers `P` as
    // `&mut Port` under the forwarding impls of ADR-0021 decision 11, and an
    // owned handle or a wrapper is accepted too.
    assert!(d.contains("port:P,"), "the client holds its port by value");
    assert!(
        !d.contains("Client<'a,"),
        "the client carries no lifetime parameter"
    );
}

#[test]
fn a_signal_only_interface_gets_a_client_bound_only_by_signal_reader() {
    // RA-19, in the direction design §6 gives: the signal-only interface's
    // client must not carry a `Caller` or an `EventSource` bound it does not
    // need. Task 5 turns this into a compiled proof by constructing this
    // client from a port that implements `Attached` and `SignalReader` alone.
    let horn = module(&face(), "horn");
    let d = dense(&horn);

    assert!(
        d.contains("pubstructClient<P:::ridl_rt::port::SignalReader"),
        "Horn client is bound by SignalReader",
    );
    assert!(
        !d.contains("Caller"),
        "no Caller bound on a signal-only client"
    );
    assert!(
        !d.contains("EventSource"),
        "no EventSource bound on a signal-only client",
    );
    assert!(
        !d.contains("EventSink"),
        "no EventSink bound on a signal-only publisher",
    );
    assert!(
        d.contains("pubstructPublisher<W:::ridl_rt::port::SignalWriter>"),
        "a signal-only interface still gets a publisher, bound by SignalWriter alone",
    );
    assert!(
        !d.contains("Handler"),
        "a signal-only interface gets no dispatch",
    );
    assert!(
        !d.contains("pubtraitProvider"),
        "a signal-only interface gets no Provider trait",
    );
}

#[test]
fn the_client_reads_a_signal_through_the_signal_reader_port() {
    let d = dense(&module(&face(), "cabin"));

    assert!(
        d.contains("pubfntemperature(&self,)->::core::result::Result<::ridl_rt::sample::Sample<super::Temperature>,"),
        "signal accessor returns a Sample of the declared payload",
    );
    assert!(
        d.contains("self.port.read(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(1u32),&mutbuf,)"),
        "the accessor calls SignalReader::read"
    );
    assert!(
        d.contains("::ridl_rt::contract::Ordinal(1u32)"),
        "the signal's ordinal is read from the descriptor row",
    );
    // A payload that fails its check is reported as a detected invalid state,
    // not as a fabricated port error.
    assert!(
        d.contains("::ridl_rt::sample::Detection::InvalidValue(violation)"),
        "a constraint violation becomes Detection::InvalidValue",
    );
    assert!(
        d.contains("::ridl_rt::sample::Detection::Corrupt"),
        "malformed bytes become Detection::Corrupt",
    );
    assert!(
        d.contains("<super::CabinTemperatureas::ridl_rt::contract::Signal>::init()"),
        "the init value stands in for a payload that failed its check",
    );
}

#[test]
fn the_client_subscribes_and_polls_events_through_the_event_source_port() {
    let d = dense(&module(&face(), "cabin"));

    assert!(
        d.contains("pubfnsubscribe_warning(&mutself,)"),
        "one subscribe method per event",
    );
    assert!(
        d.contains("self.port.subscribe(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,&[::ridl_rt::contract::Ordinal(2u32)],)"),
        "subscribe calls EventSource::subscribe"
    );
    assert!(
        d.contains("self.port.next("),
        "polling calls EventSource::next"
    );
    assert!(d.contains("pubenumEvent{"), "an occurrence enum");
    assert!(
        d.contains("Warning(::ridl_rt::sample::Occurrence<super::Warning>),"),
        "one variant per event, carrying the declared payload",
    );
    assert!(
        d.contains("::ridl_rt::contract::Ordinal(2u32)=>{Ok(Some(Event::Warning("),
        "the poll routes the event's ordinal to its variant",
    );
    // The interface number is checked first: ordinals restart at 1 in each
    // interface, and a port is attached to a whole catalog, so an occurrence
    // of a sibling interface would otherwise be decoded as this interface's
    // payload.
    assert!(
        d.contains(
            "ifoccurrence.iface!=<super::Cabinas::ridl_rt::contract::Interface>::NUMBER{\
             returnErr(::ridl_rt::port::ReadError::Contract(\
             ::ridl_rt::error::Contract::UnknownInteraction,),);}"
        ),
        "the poll checks the interface number before the ordinal",
    );
}

#[test]
fn the_client_sends_a_command_and_a_query_through_the_caller_port() {
    let d = dense(&module(&face(), "cabin"));

    assert!(
        d.contains(
            "pubfnset_level(&mutself,level:super::Level,)\
             ->::core::result::Result<SetLevelCorrelation,::ridl_rt::port::SendError>"
        ),
        "the command method returns its own correlation newtype",
    );
    assert!(
        d.contains("self.port.command(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(3u32),&buf[..len],)"),
        "the command calls Caller::command"
    );
    // Scoped to the client's own method body: `dispatch` evaluates the same
    // clause, so an unscoped assertion would be satisfied by the provider
    // side and would not see a consumer that skipped it.
    let send_command = between(&d, "pubfnset_level(&mutself", "pubfnaverage(&mutself");
    assert!(
        send_command
            .contains("<super::CabinSetLevelas::ridl_rt::contract::Command>::require(&level)"),
        "the consumer evaluates the command's require, through the Command trait",
    );
    assert!(
        d.contains(
            "pubfnaverage(&mutself,window:super::Window,)\
             ->::core::result::Result<AverageCorrelation,::ridl_rt::port::SendError>"
        ),
        "the query method returns its own correlation newtype",
    );
    assert!(
        d.contains("self.port.query(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(4u32),&buf[..len],)"),
        "the query calls Caller::query"
    );
    assert!(
        d.contains("pubfnaverage_reply(&mutself,correlation:AverageCorrelation,)"),
        "a separate reply method polls the correlation, typed by its query",
    );
    assert!(
        d.contains("self.port.reply("),
        "the reply method calls Caller::reply"
    );
    let send_query = between(&d, "pubfnaverage(&mutself", "pubfnaverage_reply");
    assert!(
        send_query.contains("<super::CabinAverageas::ridl_rt::contract::Query>::require(&window)"),
        "the consumer evaluates the query's require, through the Query trait",
    );

    // The reply's own check failures map to the two call errors, in the same
    // split the dispatch side uses.
    let reply = between(&d, "pubfnaverage_reply", "pubfnset_level_ack");
    assert!(
        reply.contains("::ridl_rt::error::Contract::InvalidValue(violation)"),
        "a reply that breaks a typl constraint is an InvalidValue contract error",
    );
    assert!(
        reply.contains("::ridl_rt::error::Transport::Corrupt"),
        "malformed reply bytes are a transport corruption",
    );
    assert!(
        d.contains("pubfnset_level_ack(&mutself,correlation:SetLevelCorrelation,)"),
        "the acknowledgment method is per command and takes that command's newtype",
    );
    assert!(
        d.contains("self.port.ack(correlation.0)"),
        "the acknowledgment method calls Caller::ack through the newtype"
    );
}

#[test]
fn the_publisher_writes_signals_and_raises_events() {
    let d = dense(&module(&face(), "cabin"));

    assert!(
        d.contains("pubstructPublisher<W:::ridl_rt::port::SignalWriter+::ridl_rt::port::EventSink"),
        "Cabin publisher port bounds",
    );
    assert!(
        d.contains("port:W,"),
        "the publisher holds its port by value"
    );
    assert!(
        d.contains("pubfntemperature(&mutself,value:super::Temperature,)"),
        "one publish method per signal",
    );
    assert!(
        d.contains("self.port.set(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(1u32),&buf[..len],)"),
        "publishing calls SignalWriter::set"
    );
    assert!(
        d.contains("pubfninvalidate_temperature(&mutself,)"),
        "one invalidate method per signal",
    );
    assert!(
        d.contains("self.port.invalidate(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(1u32),)"),
        "invalidate calls SignalWriter::invalidate"
    );
    assert!(
        d.contains("pubfnwarning(&mutself,value:super::Warning,)"),
        "one raise method per event",
    );
    assert!(
        d.contains("self.port.raise(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(2u32),&buf[..len],)"),
        "raising calls EventSink::raise"
    );
    assert!(d.contains("pubfncommit(&mutself)"), "the publisher commits");
    assert!(
        d.contains("self.port.commit()"),
        "commit calls SignalWriter::commit"
    );
}

#[test]
fn the_provider_trait_carries_the_settled_method_signatures() {
    let d = dense(&module(&face(), "cabin"));

    // A command returns nothing (design §6: a command has no failure the
    // application reports), and a query returns its declared reply type.
    assert!(d.contains("pubtraitProvider{"), "the Provider trait");
    assert!(
        d.contains("fnset_level(&mutself,level:&super::Level);"),
        "the command provider method returns nothing",
    );
    assert!(
        d.contains("fnaverage(&mutself,window:&super::Window)->super::Average;"),
        "the query provider method returns its declared reply",
    );
}

#[test]
fn the_pipeline_generate_emits_no_face_module() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let plain = generate(&package).expect("generate").rust_source;

    assert!(
        !plain.contains("pub mod cabin"),
        "the pipeline emits no face module"
    );
    assert!(
        !plain.contains("Publisher"),
        "the pipeline emits no publisher"
    );
}
