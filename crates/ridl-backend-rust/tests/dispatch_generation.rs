//! The generated `dispatch` over the interaction-face fixture (Lane M stage
//! M3, Task 3). Asserts that the emitted routing, settlement mapping and
//! counting are total and in the order the design's settlement table gives.
//!
//! These assertions read the generated source, for the reason
//! `face_generation.rs` records: nothing in this crate can compile the face
//! yet. Task 5 adds the compiled round trip, which is where the settlements
//! are observed at run time.

use ridl_backend_rust::generate_face;

#[path = "support/ir.rs"]
mod ir;

fn dense(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The whitespace-stripped source of the face module of interface `Cabin`,
/// which is the interface the fixture's calls are declared on.
fn cabin() -> String {
    let package = ir::compile_fixture("interaction_face.ridl");
    let source = generate_face(&package).expect("generate_face").rust_source;
    let start = source
        .find("pub mod cabin {")
        .unwrap_or_else(|| panic!("generated source has no `pub mod cabin`:\n{source}"));
    let rest = &source[start..];
    let end = rest[1..]
        .find("\npub mod ")
        .map_or(rest.len(), |index| index + 1);
    dense(&rest[..end])
}

/// The whitespace-stripped source of `dispatch` alone, so a count is not
/// satisfied by the same token in the client's own clause evaluation.
fn dispatch_source() -> String {
    let module = cabin();
    module[at(&module, "pubfndispatch")..].to_string()
}

/// The byte offset of `needle`, or a failure naming it.
fn at(source: &str, needle: &str) -> usize {
    source
        .find(needle)
        .unwrap_or_else(|| panic!("generated dispatch has no `{needle}`"))
}

#[test]
fn dispatch_has_the_settled_signature() {
    let d = dispatch_source();
    assert!(
        d.contains("pubfndispatch<H,P>(h:&mutH,p:&mutP,buf:&mut[u8])->usize"),
        "dispatch signature",
    );
    assert!(d.contains("H:::ridl_rt::port::Handler"), "handler bound");
    assert!(d.contains("P:Provider"), "provider bound");
}

#[test]
fn dispatch_checks_the_caller_owned_buffer_before_it_polls() {
    let d = dispatch_source();

    // The precondition is the interface's argument-and-reply maximum, and it
    // returns 0 without consuming a claim, so the caller can retry with a
    // correctly sized buffer.
    assert!(
        d.contains("ifbuf.len()<super::Cabin::MAX_BUFFER_SIZE{return0;}"),
        "the buffer precondition returns 0",
    );
    assert!(
        at(&d, "ifbuf.len()<super::Cabin::MAX_BUFFER_SIZE") < at(&d, "h.next_claim("),
        "the precondition is checked before the first poll",
    );
}

#[test]
fn dispatch_settles_an_unknown_route_rather_than_leaving_it_open() {
    let d = dispatch_source();

    // An interface number this interface does not recognise.
    assert!(
        d.contains("ifclaim.iface!=<super::Cabinas::ridl_rt::contract::Interface>::NUMBER"),
        "the interface number is checked",
    );
    // An ordinal no arm matches: the fallback arm of the generated match.
    assert!(
        d.contains("_=>{h.settle(claim.id,Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::UnknownInteraction,),),)}"),
        "the unknown-ordinal fallback arm",
    );
    assert_eq!(
        d.matches("::ridl_rt::error::Contract::UnknownInteraction")
            .count(),
        2,
        "one unknown-route settlement for the interface number and one for the ordinal",
    );
}

#[test]
fn dispatch_maps_both_verify_error_variants() {
    let d = dispatch_source();

    assert!(
        d.contains(
            "Err(::ridl_rt::payload::VerifyError::Structure(_))=>{\
             Err(::ridl_rt::error::CallError::Transport(\
             ::ridl_rt::error::Transport::Corrupt,),)}"
        ),
        "malformed argument bytes settle Transport::Corrupt",
    );
    assert!(
        d.contains(
            "Err(::ridl_rt::payload::VerifyError::Contract(violation))=>{\
             Err(::ridl_rt::error::CallError::Contract(\
             ::ridl_rt::error::Contract::InvalidValue(violation),),)}"
        ),
        "a broken typl constraint settles Contract::InvalidValue carrying the violation",
    );
}

#[test]
fn dispatch_evaluates_require_before_the_provider_and_ensure_after_it() {
    let d = dispatch_source();

    let require = at(
        &d,
        "<super::CabinAverageas::ridl_rt::contract::Query>::require(&window,)",
    );
    let call = at(&d, "p.average(&window)");
    let ensure = at(
        &d,
        "<super::CabinAverageas::ridl_rt::contract::Query>::ensure(&window,&reply,)",
    );
    assert!(
        require < call,
        "require is evaluated before the provider runs"
    );
    assert!(
        call < ensure,
        "ensure is evaluated after the provider returns"
    );

    // A command has a require clause and no ensure clause.
    assert!(
        d.contains("<super::CabinSetLevelas::ridl_rt::contract::Command>::require(&level,)"),
        "the command's require is evaluated",
    );
    assert!(
        d.contains("p.set_level(&level);"),
        "the command reaches the provider",
    );
}

#[test]
fn dispatch_maps_a_failed_clause_to_its_own_contract_error() {
    let d = dispatch_source();

    assert!(
        d.contains("::ridl_rt::error::Contract::PreconditionFailed"),
        "a failed require settles PreconditionFailed",
    );
    assert!(
        d.contains("::ridl_rt::error::Contract::ContractBroken"),
        "a failed ensure settles ContractBroken",
    );
    // One per call for require, one for the query's ensure.
    assert_eq!(
        d.matches("::ridl_rt::error::Contract::PreconditionFailed")
            .count(),
        2,
        "the command and the query each settle a failed require",
    );
    assert_eq!(
        d.matches("::ridl_rt::error::Contract::ContractBroken")
            .count(),
        1,
        "only the query settles a failed ensure",
    );
}

#[test]
fn a_reply_that_does_not_fit_is_an_explicit_invariant_failure() {
    let d = dispatch_source();

    // The buffer precondition makes `EncodeError::Capacity` unreachable for a
    // legal value, so the branch must name the defect rather than manufacture
    // a contract error the caller did not cause.
    assert!(
        d.contains(
            "Err(::ridl_rt::payload::EncodeError::Capacity{needed,available,},)=>{unreachable!("
        ),
        "the capacity branch is an explicit unreachable",
    );
    assert!(
        d.contains("needed,available)}"),
        "the message names the needed and the available size",
    );
    assert!(
        d.contains("encoding`Average`needs{}bytesandthedispatchbufferhas{};"),
        "the message names the type and the dispatch buffer",
    );
    let capacity = at(&d, "EncodeError::Capacity");
    assert!(
        !d[capacity..capacity + 400].contains("::ridl_rt::error::Contract::InvalidValue"),
        "the capacity branch must not manufacture a contract error",
    );
}

#[test]
fn dispatch_counts_only_a_settlement_the_handler_accepted() {
    let d = dispatch_source();

    assert!(
        d.contains("h.settle(claim.id,Ok(bytes))"),
        "a query's reply is settled through the handler",
    );
    assert!(
        d.contains("matchdecoded{Err(error)=>h.settle(claim.id,Err(error)),"),
        "a failing check is settled too, so every claim is settled",
    );
    assert!(
        d.contains("ifsettlement.is_ok(){settled+=1;}"),
        "the count only advances after a successful settlement",
    );
    assert!(
        at(&d, "h.settle(claim.id") < at(&d, "settled+=1;"),
        "the increment follows the settlement",
    );
    // A SettleError does not abort the pass: the loop is not left on it.
    assert!(
        !d.contains("settlement?"),
        "a SettleError must not propagate out of the pass",
    );
}

#[test]
fn the_two_buffer_constants_are_used_where_each_belongs() {
    let d = cabin();

    assert!(
        d.contains("super::Cabin::MAX_BUFFER_SIZE"),
        "dispatch sizes from the argument-and-reply maximum",
    );
    assert!(
        d.contains("[0u8;super::Cabin::EVENT_SOURCE_BUFFER_SIZE]"),
        "the event poll sizes from the separate event-only maximum",
    );
    // The event constant is not what dispatch checks, and the call constant is
    // not what the event poll allocates.
    let dispatch_start = at(&d, "pubfndispatch");
    assert!(
        !d[dispatch_start..].contains("EVENT_SOURCE_BUFFER_SIZE"),
        "dispatch must not size from the event-only maximum",
    );
    assert!(
        !d[..at(&d, "[0u8;super::Cabin::EVENT_SOURCE_BUFFER_SIZE]")].contains("MAX_BUFFER_SIZE"),
        "the event poll must not size from the argument-and-reply maximum",
    );
}

#[test]
fn a_command_is_settled_before_the_application_method_runs() {
    let d = dispatch_source();

    // A command's acknowledgment is a delivery acknowledgment, not a
    // completion one (ridl section 6.1), and `Handler`'s own contract says the
    // generated dispatch settles `Ok(&[])` after the arguments and `require`
    // pass and before application code runs.
    assert!(
        d.contains("letaccepted=h.settle(claim.id,Ok(&[]));p.set_level(&level);accepted"),
        "the command is settled, then the provider runs, and that settlement is what counts",
    );

    // A query is the other way round: its settlement carries the reply, so it
    // cannot precede the provider.
    assert!(
        at(&d, "p.average(&window)") < at(&d, "h.settle(claim.id,Ok(bytes))"),
        "a query is settled after the provider returns",
    );
}
