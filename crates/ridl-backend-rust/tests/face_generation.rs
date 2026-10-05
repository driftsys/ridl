//! The generated interaction face over the interaction-face fixture.
//! Asserts that `generate_face` emits, per interface, a
//! `Client` bound by exactly the ports that interface needs, a `Publisher`
//! over the writer ports, and a `Provider` trait with the settled method
//! signatures.
//!
//! These assertions read the generated source and pin its text. The face is
//! compiled and run elsewhere: `interaction_face.rs` `include!`s the checked-in
//! generated file and runs the round trips over `ridl-loopback`, including the
//! minimal `Attached + SignalReader` port that constructs the signal-only
//! interface's `Client` (design §6, RA-19), and `face_compile.rs` compiles the
//! face of inline sources with a bare `rustc`. What this file pins is the
//! shape of the text those proofs exercise.

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

/// The byte offset of `needle`, or a failure naming it, so two generated
/// statements can be checked for their order.
fn at(source: &str, needle: &str) -> usize {
    source
        .find(needle)
        .unwrap_or_else(|| panic!("generated source has no `{needle}`"))
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
    // a query need Caller; a command or a query also needs Clock for the
    // call's deadline and Wakeable for the wait, and an event needs Wakeable
    // for the wait on the queue (the async face design, note F-10).
    assert!(
        d.contains(
            "pubstructClient<P:::ridl_rt::port::SignalReader+::ridl_rt::port::EventSource\
             +::ridl_rt::port::Caller+::ridl_rt::port::Clock+::ridl_rt::port::Wakeable,>"
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
    // A signal read returns at once, so nothing on this client waits: no
    // deadline to read a clock for, and no interest to register.
    assert!(
        !d.contains("Clock"),
        "no Clock bound on a signal-only client"
    );
    assert!(
        !d.contains("Wakeable"),
        "no Wakeable bound on a signal-only client"
    );
    assert!(
        d.contains("pubstructPublisher<W:::ridl_rt::port::SignalWriter>"),
        "a signal-only interface still gets a publisher, bound by SignalWriter alone",
    );
    assert!(
        !d.contains("Handler"),
        "a signal-only interface gets no dispatch and no serve",
    );
    assert!(
        !d.contains("pubtraitProvider"),
        "a signal-only interface gets no Provider trait",
    );
}

#[test]
fn an_event_only_interface_gets_a_client_bound_by_event_source_and_wakeable() {
    // The third bound set of note F-10: an event needs `Wakeable`, because
    // `next_event` returns a future that waits on the queue, and not `Clock`,
    // because an event has no response bound; its time to live is applied by
    // the runtime inside `EventSource::next`.
    let d = dense(&module(&face(), "siren"));

    assert!(
        d.contains("pubstructClient<P:::ridl_rt::port::EventSource+::ridl_rt::port::Wakeable>"),
        "Siren client port bounds",
    );
    assert!(
        !d.contains("Clock"),
        "no Clock bound on an event-only client"
    );
    assert!(
        !d.contains("Caller"),
        "no Caller bound on an event-only client"
    );
    assert!(
        d.contains(
            "pubstructNextEvent<'a,P:::ridl_rt::port::EventSource+::ridl_rt::port::Wakeable,>"
        ),
        "the event future carries the same two bounds",
    );
}

#[test]
fn a_calls_only_interface_gets_a_client_bound_by_caller_clock_and_wakeable() {
    // The second bound set of note F-10 on its own: `Caller` for the send,
    // `Clock` for the deadline, `Wakeable` for the wait, and nothing for a
    // signal or an event the interface does not declare.
    let d = dense(&module(&face(), "valve"));

    assert!(
        d.contains(
            "pubstructClient<P:::ridl_rt::port::Caller+::ridl_rt::port::Clock+::ridl_rt::port::Wakeable"
        ),
        "Valve client port bounds",
    );
    assert!(
        !d.contains("SignalReader"),
        "no SignalReader bound on a calls-only client"
    );
    assert!(
        !d.contains("EventSource"),
        "no EventSource bound on a calls-only client"
    );
    assert!(!d.contains("NextEvent"), "no event future without an event");
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
    // Before the first publication, or when the channel is invalidated with
    // no prior publication, there is no payload to check: the accessor
    // returns the init value under the port's own provenance directly,
    // without running `verify` (driftsys/ridl#517).
    assert!(
        d.contains(
            "matchraw.provenance{::ridl_rt::sample::Provenance::Init\
             |::ridl_rt::sample::Provenance::Invalid(::ridl_rt::sample::Cause::Declared,)\
             ifraw.len==0=>{(<super::CabinTemperatureas::ridl_rt::contract::Signal>::init(),"
        ),
        "Init and a zero-length Invalid(Declared) both return the init value \
         without verifying",
    );
    // `init()` appears twice: once for the never-published/never-set case
    // above, and once here, where a payload that fails its check substitutes
    // it. This assertion is anchored to the `Err` arm's own occurrence, not
    // to `init()` generally.
    assert!(
        d.contains(
            "Err(error)=>{(<super::CabinTemperatureas::ridl_rt::contract::Signal>::init(),\
             ::ridl_rt::sample::Provenance::Invalid("
        ),
        "the init value stands in for a payload that failed its check",
    );
}

#[test]
fn the_client_subscribes_and_polls_events_through_the_event_source_port() {
    let d = dense(&module(&face(), "cabin"));

    // `subscribe_<event>` is a method of the module's `Subscribe` trait, and
    // `next_event` of `ridl_rt::face::Events`, not inherent methods (ADR-0023
    // decision 7): a member named `subscribeWarning` or `nextEvent` compiles.
    assert!(
        d.contains("pubtraitSubscribe{")
            && d.contains(
                "fnsubscribe_warning(&mutself,)\
                 ->::core::result::Result<(),::ridl_rt::port::SubscribeError>;"
            ),
        "one subscribe method per event, declared by the per-interface trait",
    );
    assert!(
        d.contains("SubscribeforClient<P>{fnsubscribe_warning(&mutself,)"),
        "the async client implements Subscribe",
    );
    assert!(
        d.contains("self.port.subscribe(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,&[::ridl_rt::contract::Ordinal(2u32)],)"),
        "subscribe calls EventSource::subscribe"
    );
    // `next_event` returns a named future; the read itself is the internal
    // `poll_next_event`, which the future's `poll` calls after it registers
    // `Interest::Event` (the async face design, notes F-5 and F-10).
    assert!(
        d.contains("::ridl_rt::face::EventsforClient<P>{typeNext<'a>=NextEvent<'a,P>whereSelf:'a;"),
        "the async client's Events::Next is the named event future",
    );
    assert!(
        d.contains("fnnext_event(&mutself)->NextEvent<'_,P>"),
        "next_event returns the named event future",
    );
    assert!(
        d.contains("pub(crate)fnpoll_next_event<P:::ridl_rt::port::EventSource>(port:&mutP"),
        "the event read is internal plumbing over a bare port",
    );
    assert!(d.contains("port.next("), "polling calls EventSource::next");
    let future = between(&d, "pubstructNextEvent<'a,", "pub(crate)fnsend_set_level");
    assert!(
        future.contains("typeOutput=::core::result::Result<Event,::ridl_rt::port::ReadError>;"),
        "the event future resolves to one occurrence, or the read's failure",
    );
    assert!(
        at(
            &future,
            "this.port.wake_on(::ridl_rt::port::Interest::Event("
        ) < at(&future, "poll_next_event(&mut*this.port)"),
        "the event future registers its interest before it reads the port",
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

    // The public method sends when it is called and returns the call's named
    // future (the async face design, note F-4). The send, the acknowledgment
    // and the reply are internal plumbing over a bare port, so the future,
    // which holds `&mut P` and not the client, can call them (notes F-10 and
    // F-12).
    assert!(
        d.contains("pubfnset_level(&mutself,level:super::Level)->SetLevelCall<'_,P>"),
        "the command method returns its own future",
    );
    let set_level = between(&d, "pubfnset_level(&mutself", "pubfnaverage(&mutself");
    assert!(
        set_level.contains(
            "<super::CabinSetLevelas::ridl_rt::contract::Interaction>::MEMBER.call_deadline()"
        ),
        "the deadline is the member's call deadline",
    );
    assert!(
        set_level.contains("self.port.now().0.saturating_add(max.0)"),
        "the deadline is measured from the port's clock when the method is called",
    );
    assert!(
        set_level.contains("matchsend_set_level(&mutself.port,&__arg){"),
        "the method sends through the internal send",
    );
    assert!(
        set_level.contains("Err(::ridl_rt::port::SendError::Busy)=>SetLevelPhase::Unsent(__arg),"),
        "a busy port keeps the argument for the retry",
    );
    assert!(
        set_level.contains("Err(error)=>SetLevelPhase::Failed(error),"),
        "any other send failure is a future ready with that error",
    );
    assert!(
        d.contains(
            "pub(crate)fnsend_set_level<P:::ridl_rt::port::Caller>(__port:&mutP,level:&super::Level"
        ),
        "the command's send is internal and takes the argument by reference",
    );
    assert!(
        d.contains("->::core::result::Result<SetLevelCorrelation,::ridl_rt::port::SendError>{"),
        "the internal send returns the command's own correlation newtype",
    );
    assert!(
        d.contains("__port.command(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(3u32),bytes,)"),
        "the command calls Caller::command"
    );
    // Scoped to the internal send's body: `dispatch` evaluates the same
    // clause, so an unscoped assertion would be satisfied by the provider
    // side and would not see a consumer that skipped it.
    let send_command = between(
        &d,
        "pub(crate)fnsend_set_level",
        "pub(crate)fnpoll_set_level_ack",
    );
    assert!(
        send_command
            .contains("<super::CabinSetLevelas::ridl_rt::contract::Command>::require(__arg)"),
        "the consumer evaluates the command's require, through the Command trait",
    );
    assert!(
        d.contains("pubfnaverage(&mutself,window:super::Window)->AverageCall<'_,P>"),
        "the query method returns its own future",
    );
    assert!(
        d.contains(
            "pub(crate)fnsend_average<P:::ridl_rt::port::Caller>(__port:&mutP,window:&super::Window"
        ),
        "the query's send is internal and takes the argument by reference",
    );
    assert!(
        d.contains("->::core::result::Result<AverageCorrelation,::ridl_rt::port::SendError>{"),
        "the internal send returns the query's own correlation newtype",
    );
    assert!(
        d.contains("__port.query(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(4u32),bytes,)"),
        "the query calls Caller::query"
    );
    assert!(
        d.contains(
            "pub(crate)fnpoll_average_reply<P:::ridl_rt::port::Caller>(port:&mutP,correlation:AverageCorrelation"
        ),
        "the reply read is internal, typed by its query",
    );
    assert!(
        d.contains("port.reply("),
        "the reply read calls Caller::reply"
    );
    let send_query = between(
        &d,
        "pub(crate)fnsend_average",
        "pub(crate)fnpoll_average_reply",
    );
    assert!(
        send_query.contains("<super::CabinAverageas::ridl_rt::contract::Query>::require(__arg)"),
        "the consumer evaluates the query's require, through the Query trait",
    );

    // The reply's own check failures map to the two call errors, in the same
    // split the dispatch side uses.
    let reply = between(
        &d,
        "pub(crate)fnpoll_average_reply",
        "pub(crate)fnpoll_next_event",
    );
    assert!(
        reply.contains("::ridl_rt::error::Contract::InvalidValue(violation)"),
        "a reply that breaks a typl constraint is an InvalidValue contract error",
    );
    assert!(
        reply.contains("::ridl_rt::error::Transport::Corrupt"),
        "malformed reply bytes are a transport corruption",
    );
    assert!(
        d.contains(
            "pub(crate)fnpoll_set_level_ack<P:::ridl_rt::port::Caller>(port:&mutP,correlation:SetLevelCorrelation"
        ),
        "the acknowledgment read is internal, per command, and takes that command's newtype",
    );
    assert!(
        d.contains("port.ack(correlation.0)"),
        "the acknowledgment read calls Caller::ack through the newtype"
    );
}

/// The state machine of note F-4, as the emitter writes it for a command and
/// for a query: the phases, the register-then-read order of every poll, the
/// two deadline outcomes of note F-3, and the `forget` on drop.
#[test]
fn each_call_returns_a_named_future_that_forgets_its_call_on_drop() {
    let d = dense(&module(&face(), "cabin"));
    let bounds = "::ridl_rt::port::Caller+::ridl_rt::port::Clock+::ridl_rt::port::Wakeable";

    assert!(
        d.contains(&format!(
            "pubstructSetLevelCall<'a,P:{bounds},>{{port:&'amutP,phase:SetLevelPhase,\
             deadline:::core::option::Option<::ridl_rt::sample::Timestamp>,}}"
        )),
        "the command future holds the port, its phase and its deadline, and nothing else",
    );
    // The variants one by one, because each carries a doc comment that the
    // whitespace-stripped source keeps between them.
    let phases = between(&d, "enumSetLevelPhase{", "impl<");
    for variant in [
        "Unsent(super::Level),",
        "Waiting(SetLevelCorrelation),",
        "Failed(::ridl_rt::port::SendError),",
        "Done,}",
    ] {
        assert!(
            phases.contains(variant),
            "the phases are unsent with the argument, waiting with the correlation, \
             failed at the send, and done; `{variant}` is missing"
        );
    }
    let set_level = between(&d, "pubstructSetLevelCall<'a,", "pubstructAverageCall<'a,");
    assert!(
        set_level.contains(&format!(
            "impl<P:{bounds},>::core::future::FutureforSetLevelCall<'_,P>{{\
             typeOutput=::core::result::Result<(),::ridl_rt::error::ClientError>;"
        )),
        "a command's future resolves to the delivery result under ClientError",
    );
    // Register, then read (note F-5): the slot interest before the retry of
    // the send, and the outcome interest before the acknowledgment is read.
    assert!(
        at(
            &set_level,
            "this.port.wake_on(::ridl_rt::port::Interest::Slot,cx.waker());"
        ) < at(&set_level, "send_set_level(&mut*this.port,&__arg)"),
        "the unsent phase registers Interest::Slot before it retries the send",
    );
    assert!(
        at(
            &set_level,
            "this.port.wake_on(::ridl_rt::port::Interest::Outcome(correlation.0),cx.waker(),);"
        ) < at(
            &set_level,
            "poll_set_level_ack(&mut*this.port,correlation,)"
        ),
        "the waiting phase registers Interest::Outcome before it reads the acknowledgment",
    );
    // Note F-3's two deadline outcomes: unsent is `Send(Busy)`; sent is the
    // port's own expired variant for the kind, after a `forget`.
    assert!(
        set_level
            .contains("::ridl_rt::error::ClientError::Send(::ridl_rt::port::SendError::Busy,)"),
        "a call still unsent at its deadline resolves to Send(Busy)",
    );
    assert!(
        set_level.contains("::ridl_rt::error::Transport::Undelivered"),
        "a sent command resolves to Undelivered at its deadline",
    );
    assert!(
        !set_level.contains("::ridl_rt::error::Transport::Timeout"),
        "a command never resolves to Timeout",
    );
    assert!(
        set_level.contains(&format!(
            "impl<P:{bounds},>::core::ops::DropforSetLevelCall<'_,P>{{fndrop(&mutself){{\
             ifletSetLevelPhase::Waiting(correlation)=&self.phase{{\
             self.port.forget(correlation.0);}}}}}}"
        )),
        "dropping the future while it waits forgets the call, and only then",
    );
    assert!(
        at(&set_level, "this.port.forget(correlation.0);")
            < at(&set_level, "::core::ops::DropforSetLevelCall"),
        "a future that leaves the waiting phase in poll forgets the call there, so drop has nothing to forget",
    );

    let average = between(&d, "pubstructAverageCall<'a,", "pubstructNextEvent<'a,");
    assert!(
        average.contains(
            "typeOutput=::core::result::Result<super::Average,::ridl_rt::error::ClientError"
        ),
        "a query's future resolves to the decoded reply under ClientError",
    );
    assert!(
        average.contains("::ridl_rt::error::Transport::Timeout"),
        "a sent query resolves to Timeout at its deadline",
    );
    assert!(
        !average.contains("::ridl_rt::error::Transport::Undelivered"),
        "a query never resolves to Undelivered",
    );
    assert!(
        average.contains("Err(error)=>Err(::ridl_rt::error::ClientError::Read(error)),"),
        "a port failure while the reply is read is ClientError::Read",
    );
    assert!(
        !set_level.contains("ClientError::Read"),
        "an acknowledgment read cannot fail, so a command's future has no Read arm",
    );
    assert!(
        average.contains("::core::ops::DropforAverageCall<'_,P>"),
        "the query future forgets on drop too",
    );
}

/// `serve` of note F-7: the handler by value, the provider by `&mut`, the
/// served set registered when the function is called, the claim interest
/// registered before every pass, at most 32 claims taken in one poll, and the
/// handler's failure as the value the future resolves to.
#[test]
fn serve_returns_a_future_over_the_internal_dispatch_step() {
    let d = dense(&module(&face(), "cabin"));
    let bounds = "H:::ridl_rt::port::Handler+::ridl_rt::port::Wakeable,P:Provider";

    assert!(
        d.contains(&format!(
            "pubfnserve<H,P>(muth:H,p:&mutP)->Serve<'_,H,P>where{bounds},{{"
        )),
        "serve's signature",
    );
    let serve = between(&d, "pubfnserve<H,P>", "pubstructServe<'a,");
    assert!(
        serve.contains("&[::ridl_rt::contract::Ordinal(3u32),::ridl_rt::contract::Ordinal(4u32)]"),
        "serve registers the interface's command and query ordinals, and nothing else",
    );
    assert!(
        serve.contains("Err(error)=>ServeState::Refused(error),"),
        "a refused Handler::serve is a future ready with the refusal",
    );
    assert!(
        d.contains(&format!(
            "pubstructServe<'a,{bounds},>{{handler:H,provider:&'amutP,\
             buf:[u8;super::Cabin::MAX_BUFFER_SIZE],state:ServeState,}}"
        )),
        "the serve future holds the handler by value, the provider by &mut, and the claim buffer inline",
    );
    assert!(
        d.contains(&format!(
            "impl<{bounds},>::core::marker::UnpinforServe<'_,H,P>{{}}"
        )),
        "the serve future is Unpin whatever the handler type, because nothing in it is pinned",
    );
    let future = between(&d, "::core::future::FutureforServe<'_,H,P>", "fnpoll(");
    assert!(
        future.contains(
            "typeOutput=::core::result::Result<::core::convert::Infallible,::ridl_rt::error::ProviderError"
        ),
        "serve never resolves to Ok",
    );
    let poll = d[at(&d, "::core::future::FutureforServe<'_,H,P>")..].to_string();
    assert!(
        at(
            &poll,
            "this.handler.wake_on(::ridl_rt::port::Interest::Claim(\
             <super::Cabinas::ridl_rt::contract::Interface>::NUMBER,),cx.waker(),);"
        ) < at(
            &poll,
            "dispatch(&mutthis.handler,&mut*this.provider,&mutthis.buf,&mutbudget,)"
        ),
        "each poll registers Interest::Claim before it settles claims",
    );
    assert!(
        d.contains("constSERVE_BUDGET:usize=32;"),
        "one poll takes at most 32 claims (driftsys/ridl#568)",
    );
    assert!(
        poll.contains(
            "letmutbudget=SERVE_BUDGET;matchdispatch(&mutthis.handler,&mut*this.provider,\
             &mutthis.buf,&mutbudget,){Ok(_)=>{ifbudget==0{cx.waker().wake_by_ref();}\
             ::core::task::Poll::Pending}"
        ),
        "a poll that spent its budget wakes itself and is Pending; one that found no \
         claim left is Pending without waking, whatever the count",
    );
    assert!(
        poll.contains("::ridl_rt::error::ProviderError::Claim(error)"),
        "the handler port's failure is the value the future resolves to",
    );
    assert!(
        poll.contains("::ridl_rt::error::ProviderError::Serve(error)"),
        "the refusal is the value the future resolves to",
    );
}

/// ADR-0023 decision 8: `Bind::new` of `Client` and of `Publisher`, and
/// `serve`, compare the port's catalog with the interface's `CATALOG` through
/// the module's `check_catalog` before they store or use the port, and the
/// blocking client and `blocking::serve` reach that comparison through the
/// async face rather than repeating it. Each `new` and each `serve` states the
/// panic under `# Panics`.
#[test]
fn bind_new_and_serve_check_the_ports_catalog() {
    let source = module(&face(), "cabin");
    let d = dense(&source);

    assert!(
        d.contains(
            "fncheck_catalog(found:&::ridl_rt::contract::CatalogRef){letexpected=\
             <super::Cabinas::ridl_rt::contract::Interface>::CATALOG;if*found!=*expected{\
             ::core::panic!("
        ),
        "check_catalog compares the whole CatalogRef with the interface's CATALOG and panics",
    );
    let check = "check_catalog(::ridl_rt::port::Attached::catalog(&port));";

    let client = between(&d, "::ridl_rt::face::BindforClient<P>{", "Client{port}");
    assert!(
        client.contains(check),
        "the client's new checks the port before it stores it"
    );
    let publisher = between(
        &d,
        "::ridl_rt::face::BindforPublisher<W>{",
        "Publisher{port}",
    );
    assert!(
        publisher.contains(check),
        "the publisher's new checks the port before it stores it"
    );

    let serve = between(&d, "pubfnserve<H,P>", "pubstructServe<'a,");
    assert!(
        at(
            &serve,
            "check_catalog(::ridl_rt::port::Attached::catalog(&h));"
        ) < at(&serve, "h.serve("),
        "serve checks the handler port before it registers the members",
    );

    let blocking = d[at(&d, "pubmodblocking{")..].to_string();
    assert!(
        !blocking.contains("check_catalog"),
        "the blocking face reaches the check through the async face, once"
    );
    assert_eq!(
        d.matches("check_catalog(").count(),
        4,
        "one definition and three calls: Client::new, Publisher::new and serve"
    );

    // Five rustdocs state the panic: the three `new`s and the two `serve`s.
    assert_eq!(
        source.matches("# Panics").count(),
        5,
        "each new and each serve documents the panic:\n{source}"
    );
    assert!(
        source.contains("port.catalog() == <Cabin as ridl_rt::contract::Interface>::CATALOG"),
        "the rustdoc names the comparison a program can make first"
    );
}

/// The `blocking` module of the async face design's notes F-10 and F-11:
/// under the crate's `std` feature, for an interface that declares an event,
/// a command or a query; its `Client` repeats the async client's bounds; its
/// `serve` exists only with a command or a query.
#[test]
fn the_blocking_module_follows_the_interface() {
    let source = face();
    let bounds = "::ridl_rt::port::SignalReader+::ridl_rt::port::EventSource\
                  +::ridl_rt::port::Caller+::ridl_rt::port::Clock+::ridl_rt::port::Wakeable";

    let cabin = dense(&module(&source, "cabin"));
    assert!(
        cabin.contains(r#"#[cfg(feature="std")]pubmodblocking{"#),
        "cabin's blocking module is under the std feature",
    );
    // The blocking module is the last item of the interface's module, so
    // the text from its header to the end of the module is its own.
    let blocking = &cabin[at(&cabin, "pubmodblocking{")..];
    assert!(
        blocking.contains(&format!(
            "pubstructClient<P:{bounds},>{{inner:super::Client<P>,\
             timeout:::core::option::Option<::std::time::Duration>,}}"
        )),
        "the blocking client repeats the async client's bounds and holds it with the timeout",
    );
    assert!(
        blocking.contains("pubfnserve<H,P>(h:H,p:&mutP,timeout:"),
        "cabin declares calls, so its blocking module has a serve",
    );

    let siren = dense(&module(&source, "siren"));
    let blocking = &siren[at(&siren, "pubmodblocking{")..];
    assert!(
        blocking.contains(
            "pubstructClient<P:::ridl_rt::port::EventSource+::ridl_rt::port::Wakeable>\
             {inner:super::Client<P>,"
        ),
        "an event-only interface's blocking client repeats its async client's two bounds",
    );
    assert!(
        blocking.contains(
            "::ridl_rt::face::EventsforClient<P>{typeNext<'a>=::core::result::Result<\
             ::core::option::Option<super::Event>,::ridl_rt::port::ReadError,>whereSelf:'a;"
        ),
        "an event-only interface has a blocking client, whose Events::Next is the owned result",
    );
    assert!(
        !blocking.contains("pubfnserve"),
        "and no blocking serve, because it declares no call",
    );

    let horn = dense(&module(&source, "horn"));
    assert!(
        !horn.contains("pubmodblocking"),
        "a signal-only interface has no blocking module: nothing in its client waits",
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
        d.contains("self.port.set(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(1u32),bytes,)"),
        "publishing calls SignalWriter::set"
    );
    // `invalidate_<signal>` is a method of the module's `Invalidate` trait,
    // and `commit` of `ridl_rt::face::Publish` (ADR-0023 decision 7).
    assert!(
        d.contains("pubtraitInvalidate{")
            && d.contains(
                "fninvalidate_temperature(&mutself,)\
                 ->::core::result::Result<(),::ridl_rt::port::WriteError>;"
            ),
        "one invalidate method per signal, declared by the per-interface trait",
    );
    assert!(
        d.contains("InvalidateforPublisher<W>{fninvalidate_temperature(&mutself,)"),
        "the publisher implements Invalidate",
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
        d.contains("self.port.raise(<super::Cabinas::ridl_rt::contract::Interface>::NUMBER,::ridl_rt::contract::Ordinal(2u32),bytes,)"),
        "raising calls EventSink::raise"
    );
    assert!(
        d.contains("::ridl_rt::face::PublishforPublisher<W>{")
            && d.contains("fncommit(&mutself){self.port.commit()}"),
        "the publisher commits through Publish, and commit calls SignalWriter::commit"
    );
}

/// ADR-0023 decision 7 (driftsys/ridl#580): the face's fixed methods are
/// trait methods and only the member methods are inherent, so a member may
/// carry a fixed name; the emitter's own calls go through the traits' paths;
/// and each interface module carries a `prelude` that re-exports exactly the
/// traits its types implement.
#[test]
fn the_fixed_methods_are_trait_methods_and_the_prelude_follows_the_interface() {
    let source = face();

    let cabin = dense(&module(&source, "cabin"));
    assert!(
        !cabin.contains("pubfnnew(")
            && !cabin.contains("pubfnnext_event(")
            && !cabin.contains("pubfncommit(")
            && !cabin.contains("pubfnwith_timeout(")
            && !cabin.contains("pubfnset_timeout(")
            && !cabin.contains("pubfnsubscribe_")
            && !cabin.contains("pubfninvalidate_"),
        "no fixed or derived method of the face is inherent",
    );
    assert!(
        cabin.contains("::ridl_rt::face::BindforClient<P>{typePort=P;")
            && cabin.contains("::ridl_rt::face::BindforPublisher<W>{typePort=W;"),
        "both faces bind through Bind",
    );
    let blocking = &cabin[at(&cabin, "pubmodblocking{")..];
    assert!(
        blocking.contains("::ridl_rt::face::BindforClient<P>{typePort=P;")
            && blocking.contains("inner:<super::Client<P>as::ridl_rt::face::Bind>::new(port),"),
        "the blocking client binds through Bind, and builds the async one through Bind's path",
    );
    assert!(
        blocking.contains("::ridl_rt::face::TimeoutforClient<P>{")
            && blocking.contains("fnwith_timeout(mutself,timeout:::std::time::Duration)->Self")
            && blocking.contains(
                "fnset_timeout(&mutself,timeout:::core::option::Option<::std::time::Duration>,)"
            ),
        "the blocking client's timeout methods are Timeout's",
    );
    assert!(
        blocking.contains("super::SubscribeforClient<P>{")
            && blocking.contains("super::Subscribe::subscribe_warning(&mutself.inner)")
            && blocking.contains("::ridl_rt::face::Events::next_event(&mutself.inner)"),
        "the blocking client delegates to the async one through the traits' paths",
    );
    assert!(
        cabin.contains(
            r#"pubmodprelude{pubuse::ridl_rt::face::Bind;pubuse::ridl_rt::face::Events;pubusesuper::Subscribeas_;pubuse::ridl_rt::face::Publish;pubusesuper::Invalidateas_;#[cfg(feature="std")]pubuse::ridl_rt::face::Timeout;}"#
        ),
        "cabin's prelude re-exports every face trait: it has a signal, an event and a blocking module",
    );
    assert!(
        at(&cabin, "pubmodprelude{") < at(&cabin, "pubmodblocking{"),
        "the prelude precedes the blocking module, which stays the module's last item",
    );

    let horn = dense(&module(&source, "horn"));
    assert!(
        horn.contains(
            "pubmodprelude{pubuse::ridl_rt::face::Bind;pubuse::ridl_rt::face::Publish;pubusesuper::Invalidateas_;}"
        ),
        "a signal-only interface's prelude has no Events, no Subscribe and no Timeout",
    );

    let siren = dense(&module(&source, "siren"));
    assert!(
        siren.contains(
            r#"pubmodprelude{pubuse::ridl_rt::face::Bind;pubuse::ridl_rt::face::Events;pubusesuper::Subscribeas_;#[cfg(feature="std")]pubuse::ridl_rt::face::Timeout;}"#
        ),
        "an event-only interface's prelude has no Publish and no Invalidate",
    );

    let valve = dense(&module(&source, "valve"));
    assert!(
        valve.contains(
            r#"pubmodprelude{pubuse::ridl_rt::face::Bind;#[cfg(feature="std")]pubuse::ridl_rt::face::Timeout;}"#
        ),
        "a calls-only interface's prelude has Bind and Timeout alone",
    );
}

/// The two generated traits follow the interface: `Subscribe` is emitted only
/// with an event, `Invalidate` only with a signal, and the rustdoc on a
/// `Client`, a `Publisher` and a `blocking::Client` names only the traits that
/// type implements (ADR-0023 decision 7).
#[test]
fn no_subscribe_without_an_event_and_no_invalidate_without_a_signal() {
    let source = face();

    // Horn declares a signal and no event: no `Subscribe`, no `Events`, and
    // the client's rustdoc names neither.
    let horn = dense(&module(&source, "horn"));
    assert!(
        !horn.contains("Subscribe") && !horn.contains("next_event"),
        "a signal-only interface emits no Subscribe and no next_event, in code or in rustdoc",
    );
    assert!(
        horn.contains("pubtraitInvalidate{"),
        "a signal-only interface emits Invalidate",
    );

    // Siren declares an event and no signal: no `Invalidate`, no `Publish`,
    // and the publisher's rustdoc names neither.
    let siren = dense(&module(&source, "siren"));
    assert!(
        !siren.contains("Invalidate") && !siren.contains("commit"),
        "an event-only interface emits no Invalidate and no commit, in code or in rustdoc",
    );
    assert!(
        siren.contains("pubtraitSubscribe{"),
        "an event-only interface emits Subscribe",
    );

    // Valve declares calls only: neither trait, and the two clients' rustdoc
    // names neither.
    let valve = dense(&module(&source, "valve"));
    assert!(
        !valve.contains("Subscribe")
            && !valve.contains("Invalidate")
            && !valve.contains("next_event")
            && !valve.contains("commit"),
        "a calls-only interface emits neither trait, in code or in rustdoc",
    );

    // The positive side: the prelude's rustdoc lists the methods of the
    // traits it re-exports, and the blocking client's rustdoc names the event
    // traits where the interface declares an event.
    let prelude_doc = |module: &str| {
        let start = at(module, "Thetraitsaconsumerofinterface");
        let end = start + at(&module[start..], "pubmodprelude{");
        module[start..end].to_string()
    };
    let cabin = dense(&module(&source, "cabin"));
    assert!(
        prelude_doc(&cabin).contains(
            "`new`,`next_event`,`subscribe_<event>`,`commit`,`invalidate_<signal>`,`with_timeout`,`set_timeout`"
        ),
        "cabin's prelude rustdoc lists the methods of every trait it re-exports",
    );
    assert!(
        prelude_doc(&horn).contains("`new`,`commit`,`invalidate_<signal>`—"),
        "horn's prelude rustdoc lists new, commit and invalidate_<signal> alone",
    );
    assert!(
        prelude_doc(&siren)
            .contains("`new`,`next_event`,`subscribe_<event>`,`with_timeout`,`set_timeout`"),
        "siren's prelude rustdoc lists the event methods and the timeout methods",
    );
    assert!(
        prelude_doc(&valve).contains("`new`,`with_timeout`,`set_timeout`"),
        "valve's prelude rustdoc lists new and the timeout methods alone",
    );
    let blocking = &cabin[at(&cabin, "pubmodblocking{")..];
    assert!(
        blocking.contains(
            "`with_timeout`and`set_timeout`are`ridl_rt::face::Timeout`'s,`next_event`is`ridl_rt::face::Events`'sand`subscribe_<event>`istheparentmodule's`Subscribe`'s,allinscope"
        ),
        "cabin's blocking client rustdoc names Timeout, Events and Subscribe",
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
