# Induced Call Payloads Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** The generated interaction face carries every call and every
product-shaped reply, by inducing a named payload type in lowering for a
parameter list of any count and for a tuple or absent reply.

**Architecture:** Lowering gives an interaction-origin `Path` a name, and
registers a synthetic `v2::TupleType` built from the parameter list (and from a
tuple or absent reply) through the existing `tuple_ref` worklist. The codegen
model names it in `InducedTuple.name`, the command and query shapes point at it
with a new `TupleRef` field, and the FlatBuffers projection gives it a table and
a root. The Rust backend consumes it through three existing accessor functions,
which every call site already goes through.

**Tech Stack:** Rust 2024, prost (protobuf for the codegen model), `quote` /
`proc-macro2` for emission, `just` recipes for every gate.

**Design:** `docs/wip/2026-10-13-induced-call-payloads-design.md`. **Issue:**
driftsys/ridl#713. **Ruling:** R-C-14.

---

## Before you start

Read, in this order:

1. `docs/wip/2026-10-13-induced-call-payloads-design.md` — the six decisions.
2. `docs/technotes/walking-skeleton-architecture.md` — which crate owns what.
3. `docs/design/interaction-face.md` — what the face is and what it refuses.
4. ADR-0016 — the naming rule you are extending.

Run `./bootstrap` if you have just cloned or added a worktree.

The gate for a task is the smallest command that covers it. The full gate,
`just verify`, is run once at the end.

## File structure

| File                                                             | Responsibility                                                    | Group   |
| ---------------------------------------------------------------- | ----------------------------------------------------------------- | ------- |
| `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`               | the three new fields, and the prose that states the old behaviour | A       |
| `crates/ridl-ir/src/codegen/lower.rs`                            | naming an interaction path; inducing the params and reply tuples  | B       |
| `crates/ridl-ir/src/codegen/tests.rs`                            | the lowering unit tests, over the existing `Calls` fixture        | A, B, E |
| `crates/ridl-ir/src/codegen/flatbuffers.rs`                      | a table and a root for each induced interaction tuple             | C       |
| `crates/ridl-ir/src/projection/size.rs`                          | `PayloadShape::Tuple`, so a parameter list can be sized           | E       |
| `crates/ridl-ir/src/projection/size/proto3.rs`                   | sizing a tuple as a message                                       | E       |
| `crates/ridl-ir/src/projection/size/flatbuffers.rs`              | sizing a tuple as a root table                                    | E       |
| `crates/ridl-backend-rust/src/descriptors.rs`                    | the three accessors and the two refusal messages                  | D       |
| `crates/ridl-backend-rust/src/codec.rs`                          | the induced tuple's `Payload` impl, reached through the new root  | D       |
| `docs/decisions/ADR-0016-*.md`, `ADR-0023-*.md`                  | the naming rule and the request-shape rule                        | F       |
| `docs/design/interaction-face.md`, `docs/book/generated-code.md` | the prose a reader sees                                           | F       |

---

## Group A — the codegen protocol

### Task 1: Add the request and reply tuple fields

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`

The protocol is frozen with baseline `a3a0ac21` and the freeze check is a
removal check, so an added field passes. `CommandShape` uses field numbers 1–5
and `QueryShape` 1–7 today; the new fields take the next free number in each.

- [ ] **Step 1: Add `request_tuple` to `CommandShape`**

In `message CommandShape`, after the `shape` field:

```proto
// The induced argument struct, when the call's parameters are not exactly
// one named type: an index into `Model.tuples`. Exactly one of `request`
// and `request_tuple` is present for a call this toolchain lowers. The
// struct has one field per declared parameter, and no field when the call
// declares none.
optional TupleRef request_tuple = 6;
```

- [ ] **Step 2: Add `request_tuple` and `reply_tuple` to `QueryShape`**

In `message QueryShape`, after `reply_sizes`:

```proto
// As for a command.
optional TupleRef request_tuple = 8;
// The induced reply struct, when the reply is a tuple or the query
// declares no return type: an index into `Model.tuples`. A query that
// declares no return type induces a struct with no field. A fallible
// `T | E` reply induces nothing — it is a sum, and neither `reply_payload`
// nor this field is present for it.
optional TupleRef reply_tuple = 9;
```

- [ ] **Step 3: Add the tuple root to `FbRoot`**

A tuple needs a root to get a `Payload` impl, and `FbRoot.declaration` indexes
`Model.declarations`. In `message FbRoot`, after `root`:

```proto
// Set when the root is an induced tuple rather than a declaration: an
// index into `Model.tuples`. `declaration` is written as 0 and must be
// ignored when this is present.
optional uint32 tuple = 3;
```

- [ ] **Step 4: Compile the protocol**

Run: `cargo check -p ridl-ir` Expected: PASS. The generated `v1` types gain
`CommandShape::request_tuple: Option<TupleRef>`, `QueryShape::request_tuple`,
`QueryShape::reply_tuple` and `FbRoot::tuple: Option<u32>`.

- [ ] **Step 5: Check the freeze surface still passes**

Run: `just freeze-check` Expected: PASS — the `codegen-protocol` dump gains
lines and loses none. If the committed dump is stale, repair it with
`cargo xtask freeze update` and include the dump in this task's commit.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-ir/proto/ridl/codegen/v1/model.proto freeze/
git commit -m "feat(ridl-ir): add the induced request and reply tuple fields

The codegen model gains CommandShape.request_tuple, QueryShape.request_tuple,
QueryShape.reply_tuple and FbRoot.tuple, all optional and all additive, so a
reader that ignores them sees the model it saw before.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Group B — lowering

### Task 2: Name an interaction-origin path

**Files:**

- Modify: `crates/ridl-ir/src/codegen/lower.rs:117-175` (`Path`,
  `Path::interaction`)
- Modify: `crates/ridl-ir/src/codegen/lower.rs:960-985` (`Lowering::params`)
- Modify: `crates/ridl-ir/src/codegen/lower.rs:855-865` (the query reply path)
- Modify: `crates/ridl-ir/src/codegen/lower.rs:903-906` (the fixed payload path)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

Today `Path::interaction` starts with empty `rust` and `wire` and
`named:
false`, and the doc comment on `Path::named` says "every interaction
position today". Both change: the path starts at the interface name plus the
interaction name, and it is named.

- [ ] **Step 1: Add the two test helpers**

`call_package` builds its `Calls` interface with local closures, so a test that
extends it needs its own. Add these two at module scope in
`crates/ridl-ir/src/codegen/tests.rs`, next to `call_package`:

```rust
/// One public interaction declaration, for a test that extends
/// `call_package`'s `Calls` interface with a shape the fixture does not hold.
fn call_decl(name: &str, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        kind: Some(kind),
        ..Default::default()
    }
}

/// `call_package`'s `Calls` interface, to push an interaction onto.
fn calls_interface(package: &mut v2::Package) -> &mut v2::Interface {
    package
        .interfaces
        .iter_mut()
        .find(|interface| interface.name == "Calls")
        .expect("the Calls interface")
}
```

- [ ] **Step 2: Write the failing test**

Append to `crates/ridl-ir/src/codegen/tests.rs`. The `call_package()` fixture
declares `Calls` with a `two` command whose parameters are both the named type
`Pair`; a tuple-typed parameter is what this test adds.

```rust
/// A tuple reached from a parameter is named from the interface and the
/// interaction, so it is emitted rather than skipped (ADR-0016).
#[test]
fn a_tuple_in_a_parameter_is_named_from_its_interaction() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "withSpan",
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![v2::Param {
                name: "span".to_string(),
                r#type: Some(v2::FieldType {
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![v2::TupleField {
                            name: "lo".to_string(),
                            r#type: Some(named("Pair")),
                        }],
                    })),
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
    ));
    let model = lower(&package, &[]);
    let names: Vec<&str> = model
        .tuples
        .iter()
        .filter_map(|tuple| tuple.name.as_ref())
        .map(|name| name.rust.as_str())
        .collect();
    assert!(
        names.contains(&"CallsWithSpanSpan"),
        "the induced name is spelled from the interface and the interaction: {names:?}"
    );
}
```

- [ ] **Step 3: Run it and watch it fail**

Run:
`cargo test -p ridl-ir a_tuple_in_a_parameter_is_named_from_its_interaction`
Expected: FAIL — the name list holds an empty string for that tuple.

- [ ] **Step 4: Give the path its prefix**

`Path::interaction` gains the two names. Replace it with:

```rust
/// An interaction position. The induced name starts at the interface's
/// name and the interaction's, which is unique in the package because
/// an induced struct is emitted at the package's own scope: an interface
/// name is unique in a package, and an interaction name is unique in an
/// interface (ADR-0016).
fn interaction(
    interface: u32,
    slot: u32,
    visibility: i32,
    owner: &str,
    member: &str,
    segments: Vec<String>,
) -> Self {
    Self {
        base: Base::Interaction(interface, slot),
        segments,
        rust: format!("{}{}", camel_case(owner), camel_case(member)),
        wire: format!("{}{}", joined_camel(owner), joined_camel(member)),
        visibility,
        named: true,
    }
}
```

Update the doc comment on `Path::named` (line 121–123), which says no backend
has a rule for an interaction position:

```rust
/// False where no backend has a naming rule for the origin, which is a
/// position inside a declaration of another package.
```

- [ ] **Step 5: Thread the two names through the call sites**

`Lowering::params`, the reply path in `Lowering::interaction`, and the fixed
payload path each construct a `Path::interaction`. Each needs the owner and the
member. `Lowering::interaction` already has `decl.name` for the member; add an
`owner: &str` parameter to `interaction` and to `params`, and pass it from
`Lowering::interface`, which has the identity:

In `Lowering::interface`, before the `slots` loop:

```rust
let owner = match shape.service {
    Some(service) => service.name.clone(),
    None => interface.name.clone(),
};
```

and pass `&owner` into
`self.interaction(home, decl, index, slot as u32, this_row, visibility, &owner)`.

In `Lowering::interaction`, the three construction sites become:

```rust
// the query reply
let at = Path::interaction(
    interface,
    slot,
    visibility,
    owner,
    decl.name.as_str(),
    vec!["reply".to_string()],
);
```

```rust
// the fixed payload
let at = Path::interaction(
    interface,
    slot,
    visibility,
    owner,
    decl.name.as_str(),
    vec!["payload".to_string()],
);
```

and
`self.params(home, &command.params, interface, slot, visibility, owner, decl.name.as_str())`,
whose body uses:

```rust
let at = Path::interaction(
    interface,
    slot,
    visibility,
    owner,
    member,
    vec!["param".to_string(), param.name.clone()],
);
```

`Lowering::interaction` already carries `#[allow(clippy::too_many_arguments)]`;
add the same attribute to `Lowering::params`.

- [ ] **Step 6: Run the test and the crate's suite**

Run: `cargo test -p ridl-ir` Expected: the new test PASSES. Existing tests that
assert an empty induced name for an interaction position now fail — those
assertions state the behaviour this task changes. Update each to assert the
spelled name, and keep the empty-name assertion only for a foreign position.

- [ ] **Step 7: Commit**

```bash
git add crates/ridl-ir/src/codegen/lower.rs crates/ridl-ir/src/codegen/tests.rs
git commit -m "feat(ridl-ir): name a tuple induced from an interaction position

An interaction path starts at the interface's name and the interaction's, so
a tuple reached from a parameter, a reply or a fixed payload has a name every
backend can read, as a tuple reached from a struct field already does.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 3: Induce the argument struct for a command

**Files:**

- Modify: `crates/ridl-ir/src/codegen/lower.rs:825-852` (the command arm)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

The rule: a call whose parameters are exactly one named type keeps `request` and
induces nothing; every other parameter list, zero included, induces a struct.

- [ ] **Step 1: Write the failing tests**

```rust
/// A call with two parameters induces one argument struct, with one field
/// per parameter in declaration order (driftsys/ridl#713).
#[test]
fn a_two_parameter_command_induces_an_argument_struct() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 0) else {
        panic!("the `two` command");
    };
    let reference = command
        .request_tuple
        .as_ref()
        .expect("a two-parameter call induces an argument struct");
    assert!(command.request.is_none(), "and carries no named request");
    let induced = &model.tuples[reference.index as usize];
    assert_eq!(
        induced.name.as_ref().expect("a name").rust,
        "CallsTwoParams"
    );
    let fields: Vec<&str> = induced
        .fields
        .iter()
        .map(|field| field.name.as_ref().expect("a name").declared.as_str())
        .collect();
    assert_eq!(fields, vec!["a", "b"]);
}

/// A call with exactly one named parameter keeps the payload it has today
/// and induces nothing, so no existing face renames a type.
#[test]
fn a_one_parameter_command_induces_nothing() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 1) else {
        panic!("the `one` command");
    };
    assert!(command.request.is_some());
    assert!(command.request_tuple.is_none());
}

/// A call with no parameter induces an argument struct with no field: the
/// shape that skipped `readTicks` and `fuelLevel` before this change.
#[test]
fn a_zero_parameter_command_induces_an_empty_argument_struct() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "ping",
        v2::decl::Kind::CommandDef(v2::CommandDef::default()),
    ));
    let model = lower(&package, &[]);
    let induced = model
        .tuples
        .iter()
        .find(|tuple| {
            tuple.name.as_ref().is_some_and(|name| name.rust == "CallsPingParams")
        })
        .expect("the induced argument struct");
    assert!(induced.fields.is_empty());
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p ridl-ir induces_an_argument_struct induces_nothing`
Expected: FAIL — `request_tuple` is `None` and no such tuple exists.

- [ ] **Step 3: Induce the tuple**

Add a helper next to `single_param_type` at the bottom of `lower.rs`:

```rust
/// The parameter list as a tuple: the shape of the induced argument struct,
/// one field per declared parameter in declaration order. A parameter with no
/// declared type, and a stream parameter, have no struct field, so a list
/// holding either induces nothing (interaction face design, the request-shape
/// rule).
fn params_as_tuple(params: &[v2::Param]) -> Option<v2::TupleType> {
    let mut fields = Vec::with_capacity(params.len());
    for param in params {
        let ty = param.r#type.as_ref()?;
        if matches!(ty.kind.as_ref()?, v2::field_type::Kind::Stream(_)) {
            return None;
        }
        fields.push(v2::TupleField {
            name: param.name.clone(),
            r#type: Some(ty.clone()),
        });
    }
    Some(v2::TupleType { fields })
}
```

In the command arm of `Lowering::interaction`, after `request`:

```rust
let request_tuple = match single_param_type(&command.params) {
    Some(_) => None,
    None => params_as_tuple(&command.params).map(|tuple| {
        // `Path::step` appends the role word to the segments
        // and to both induced names at once, which gives
        // segments `["Params"]` and the Rust name
        // `<Interface><Interaction>Params`.
        let at = Path::interaction(
            interface,
            slot,
            visibility,
            owner,
            decl.name.as_str(),
            Vec::new(),
        )
        .step("Params");
        v1::TupleRef {
            index: self.tuple_ref(home, &tuple, &at),
        }
    }),
};
```

and add `request_tuple` to the `v1::CommandShape` literal.

- [ ] **Step 4: Run the tests**

Run:
`cargo test -p ridl-ir induces_an_argument_struct induces_nothing induces_an_empty_argument_struct`
Expected: PASS.

- [ ] **Step 5: Run the crate's suite**

Run: `cargo test -p ridl-ir` Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-ir/src/codegen/lower.rs crates/ridl-ir/src/codegen/tests.rs
git commit -m "feat(ridl-ir): induce an argument struct for a call

A call whose parameters are not exactly one named type induces a struct with
one field per declared parameter, zero parameters included, so every call has
one payload type to send.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 4: Induce the argument struct for a query

**Files:**

- Modify: `crates/ridl-ir/src/codegen/lower.rs:854-900` (the query arm)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
/// A query's request follows the same rule as a command's: the `none` query
/// of the fixture declares no parameter, so it induces an empty struct.
#[test]
fn a_query_request_follows_the_command_rule() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Query(query) = call_shape(&model, 2) else {
        panic!("the `none` query");
    };
    let reference = query
        .request_tuple
        .as_ref()
        .expect("a zero-parameter query induces an argument struct");
    let induced = &model.tuples[reference.index as usize];
    assert_eq!(
        induced.name.as_ref().expect("a name").rust,
        "CallsNoneParams"
    );
    assert!(induced.fields.is_empty());
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p ridl-ir a_query_request_follows_the_command_rule` Expected:
FAIL — `request_tuple` is `None`.

- [ ] **Step 3: Induce it**

In the query arm, after `request`, add the same block the command arm uses,
reading `query.params`, and add `request_tuple` to the `v1::QueryShape` literal.

- [ ] **Step 4: Run the test and the suite**

Run: `cargo test -p ridl-ir` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir/src/codegen/lower.rs crates/ridl-ir/src/codegen/tests.rs
git commit -m "feat(ridl-ir): induce an argument struct for a query request

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 5: Induce the reply struct

**Files:**

- Modify: `crates/ridl-ir/src/codegen/lower.rs:854-900` (the query arm)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

Two reply forms induce a struct: a tuple reply, and no declared return type. A
fallible `T | E` reply induces nothing, and a named reply keeps `reply_payload`.

- [ ] **Step 1: Write the failing tests**

```rust
/// A tuple reply induces a reply struct, one field per tuple field.
#[test]
fn a_tuple_reply_induces_a_reply_struct() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "bounds",
        v2::decl::Kind::QueryDef(v2::QueryDef {
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                        fields: vec![
                            v2::TupleField {
                                name: "min".to_string(),
                                r#type: Some(named("Pair")),
                            },
                            v2::TupleField {
                                name: "max".to_string(),
                                r#type: Some(named("Pair")),
                            },
                        ],
                    })),
                })),
            }),
            ..Default::default()
        }),
    ));
    let model = lower(&package, &[]);
    let induced = model
        .tuples
        .iter()
        .find(|tuple| {
            tuple.name.as_ref().is_some_and(|name| name.rust == "CallsBoundsReply")
        })
        .expect("the induced reply struct");
    let fields: Vec<&str> = induced
        .fields
        .iter()
        .map(|field| field.name.as_ref().expect("a name").declared.as_str())
        .collect();
    assert_eq!(fields, vec!["min", "max"]);
}

/// A fallible reply is a sum, not a product, so it induces nothing: the
/// `risky` query of the fixture returns `Pair | Pair`.
#[test]
fn a_fallible_reply_induces_nothing() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Query(query) = call_shape(&model, 3) else {
        panic!("the `risky` query");
    };
    assert!(query.reply_tuple.is_none());
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p ridl-ir reply_induces` Expected: the tuple test FAILS (no
such tuple); the fallible test PASSES already, which is the point of keeping it
— it guards the carve-out.

- [ ] **Step 3: Induce the reply struct**

Add next to `params_as_tuple`:

```rust
/// The reply as a tuple, for the two product-shaped replies: a tuple reply is
/// itself, and a query with no declared return type is the empty tuple. A
/// fallible `T | E` reply, a named reply, and every other return form give
/// `None` — a named reply keeps `reply_payload`, and a fallible reply is a
/// sum this toolchain does not induce.
fn reply_as_tuple(ret: Option<&v2::ReturnType>) -> Option<v2::TupleType> {
    let Some(ret) = ret else {
        return Some(v2::TupleType::default());
    };
    match ret.kind.as_ref()? {
        v2::return_type::Kind::Value(ty) => match ty.kind.as_ref()? {
            v2::field_type::Kind::Tuple(tuple) => Some(tuple.clone()),
            _ => None,
        },
        _ => None,
    }
}
```

In the query arm, after `reply_payload`:

```rust
let reply_tuple = match reply_named {
    Some(_) => None,
    None => reply_as_tuple(query.return_type.as_ref()).map(|tuple| {
        let at = Path::interaction(
            interface,
            slot,
            visibility,
            owner,
            decl.name.as_str(),
            Vec::new(),
        )
        .step("Reply");
        v1::TupleRef {
            index: self.tuple_ref(home, &tuple, &at),
        }
    }),
};
```

and add `reply_tuple` to the `v1::QueryShape` literal.

The reply path built at line 855 for `self.reply` keeps its `["reply"]`
segments: it names a tuple written _inside_ the return type, which is the same
tuple this task induces. Replace that path's segments with `Vec::new()` and
`.step("Reply")` so both reach one name and `tuple_ref` reuses the claim rather
than recording a collision.

- [ ] **Step 4: Run the tests and the suite**

Run: `cargo test -p ridl-ir` Expected: PASS, with exactly one tuple named
`CallsBoundsReply` in the model.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir/src/codegen/lower.rs crates/ridl-ir/src/codegen/tests.rs
git commit -m "feat(ridl-ir): induce a reply struct for a product-shaped reply

A tuple reply and a query with no declared return type each induce one named
reply struct. A fallible reply is a sum and induces nothing.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 6: Carry a parameter's doc onto its induced field

**Files:**

- Modify: `crates/ridl-ir/src/codegen/lower.rs:641-695` (`drain_tuples`)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

A tuple field carries no doc in the IR, so `drain_tuples` writes empty doc
fields. A parameter does carry one, and ADR-0028 says a generated item renders
the doc its source item has.

- [ ] **Step 1: Write the failing test**

```rust
/// A parameter's doc reaches the field of the induced argument struct, so
/// the generated struct documents its fields (ADR-0028).
#[test]
fn a_parameter_doc_reaches_the_induced_field() {
    let mut package = call_package();
    calls_interface(&mut package).interactions.push(call_decl(
        "documented",
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![
                v2::Param {
                    name: "first".to_string(),
                    r#type: Some(named("Pair")),
                    doc: "The first one.".to_string(),
                    ..Default::default()
                },
                v2::Param {
                    name: "second".to_string(),
                    r#type: Some(named("Pair")),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
    ));
    let model = lower(&package, &[]);
    let induced = model
        .tuples
        .iter()
        .find(|tuple| {
            tuple
                .name
                .as_ref()
                .is_some_and(|name| name.rust == "CallsDocumentedParams")
        })
        .expect("the induced argument struct");
    assert_eq!(induced.fields[0].doc, "The first one.");
    assert_eq!(induced.fields[1].doc, "");
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p ridl-ir a_parameter_doc_reaches_the_induced_field` Expected:
FAIL — the doc is empty.

- [ ] **Step 3: Patch the docs after the worklist drains**

`drain_tuples` cannot see the parameters, and the model is walked once more to
patch the FlatBuffers tables, so patch the docs the same way. Add to `Lowering`,
called from the same place `patch_flatbuffers` is called:

```rust
/// Copies a parameter's doc, its links, its `@see` targets and its `@since`
/// versions onto the matching field of the induced argument struct. A tuple
/// field carries no doc in the IR, so `drain_tuples` writes none; a parameter
/// does carry one, and the generated struct renders it (ADR-0028). Matched by
/// position: the struct's fields are built from the parameter list in
/// declaration order.
fn patch_param_docs(model: &mut v1::Model) {
    let mut docs: Vec<(u32, Vec<v1::Param>)> = Vec::new();
    for interface in &model.interfaces {
        for slot in &interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                slot.occupant.as_ref()
            else {
                continue;
            };
            let (reference, params) = match interaction.shape.as_ref() {
                Some(v1::interaction::Shape::Command(command)) => {
                    (command.request_tuple.as_ref(), &command.params)
                }
                Some(v1::interaction::Shape::Query(query)) => {
                    (query.request_tuple.as_ref(), &query.params)
                }
                _ => continue,
            };
            if let Some(reference) = reference {
                docs.push((reference.index, params.clone()));
            }
        }
    }
    for (index, params) in docs {
        let Some(tuple) = model.tuples.get_mut(index as usize) else {
            continue;
        };
        for (field, param) in tuple.fields.iter_mut().zip(params) {
            field.doc = param.doc;
            field.links = param.links;
            field.see = param.see;
            field.since = param.since;
        }
    }
}
```

- [ ] **Step 4: Run the test and the suite**

Run: `cargo test -p ridl-ir` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir/src/codegen/lower.rs crates/ridl-ir/src/codegen/tests.rs
git commit -m "feat(ridl-ir): carry a parameter's doc onto its induced field

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Group C — the FlatBuffers projection

### Task 7: Give each induced interaction tuple a table

**Files:**

- Modify: `crates/ridl-ir/src/codegen/flatbuffers.rs`
- Test: `crates/ridl-ir/src/codegen/tests.rs`

`patch_flatbuffers` reads `projected.tuple_tables`, keyed by the tuple's wire
name. A tuple the projection does not walk has no entry, so
`InducedTuple.flatbuffers_table` stays `None` and the Rust codec refuses it with
"the lowered model names a tuple it does not carry a table for".

- [ ] **Step 1: Write the failing test**

```rust
/// Every named induced tuple has a FlatBuffers table, so the codec can emit
/// its encode, verify and decode functions.
#[test]
fn every_named_induced_tuple_has_a_flatbuffers_table() {
    let model = lower(&call_package(), &[]);
    for tuple in &model.tuples {
        let Some(name) = tuple.name.as_ref() else {
            continue;
        };
        if name.rust.is_empty() {
            continue;
        }
        assert!(
            tuple.flatbuffers_table.is_some(),
            "`{}` has no FlatBuffers table",
            name.rust
        );
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p ridl-ir every_named_induced_tuple_has_a_flatbuffers_table`
Expected: FAIL — the induced argument structs have none.

- [ ] **Step 3: Walk the interaction tuples in the projection**

The walk in `crates/ridl-ir/src/codegen/flatbuffers.rs` reads `package.decls`
only (`Walk::declared`, line 131). The induced tuples are synthesized, so they
do not live in the package and cannot be borrowed for `'a`. Change
`Spec::Tuple`'s source to own them:

```rust
Tuple {
    tuple: Option<u32>,
    source: std::borrow::Cow<'a, v2::TupleType>,
    wire: String,
},
```

`Table` derives `Clone` and `Cow` is `Clone`, so the two existing construction
sites change only by wrapping: `source: Cow::Borrowed(tuple)` in
`Walk::discover`, and `source.fields` in `Walk::drain` and in `Filler::table`
read through the `Cow` unchanged.

Add the second source of tuples, and call it from `project` between
`walk.declared()` and `walk.drain()`:

```rust
    /// The induced argument and reply structs of every interaction. They are
    /// synthesized rather than written, so the walk builds each one the way
    /// the lowering builds it and names it with the same wire name: the
    /// table and the model's `InducedTuple` have to agree, because
    /// `patch_flatbuffers` joins them by that name.
    fn interactions(&mut self) {
        let package = self.scope.package;
        for shape in package.shapes() {
            let owner = match shape.service {
                Some(service) => service.name.clone(),
                None => shape.interface.name.clone(),
            };
            for decl in &shape.interface.interactions {
                let member = decl.name.as_str();
                let (params, ret) = match decl.kind.as_ref() {
                    Some(v2::decl::Kind::CommandDef(command)) => {
                        (Some(&command.params), None)
                    }
                    Some(v2::decl::Kind::QueryDef(query)) => {
                        (Some(&query.params), Some(query.return_type.as_ref()))
                    }
                    _ => continue,
                };
                let prefix = format!("{}{}", joined_camel(&owner), joined_camel(member));
                if let Some(params) = params
                    && single_param_type(params).is_none()
                    && let Some(tuple) = params_as_tuple(params)
                {
                    self.queue_induced(package, tuple, format!("{prefix}Params"));
                }
                if let Some(ret) = ret
                    && query_reply_type_of(ret).is_none()
                    && let Some(tuple) = reply_as_tuple(ret)
                {
                    self.queue_induced(package, tuple, format!("{prefix}Reply"));
                }
            }
        }
    }

    /// Queues one induced struct's table under `wire`, which is both its
    /// FlatBuffers table name and the key `patch_flatbuffers` reads.
    fn queue_induced(&mut self, home: &'a v2::Package, tuple: v2::TupleType, wire: String) {
        self.queue.push(Table {
            name: wire.clone(),
            spec: Spec::Tuple {
                tuple: self.tuple_index.get(&wire).copied(),
                source: std::borrow::Cow::Owned(tuple),
                wire,
            },
            home,
        });
    }
```

`params_as_tuple`, `reply_as_tuple` and `single_param_type` are the functions
Tasks 3–5 wrote in `lower.rs`; make them `pub(crate)` there and import them, so
the shape rule is written once. `query_reply_type_of` is `lower.rs`'s existing
`query_reply_type`, which answers whether the reply is one named type; export it
the same way.

- [ ] **Step 4: Run the test and the suite**

Run: `cargo test -p ridl-ir` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir/src/codegen/flatbuffers.rs crates/ridl-ir/src/codegen/tests.rs
git commit -m "feat(ridl-ir): project a table for an induced interaction tuple

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 8: Give each induced request and reply a root

**Files:**

- Modify: `crates/ridl-ir/src/codegen/flatbuffers.rs:677-705` (`roots`)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

The Rust codec emits the `Payload` impl — and with it `MAX_SIZE`, which every
descriptor's reservation row names — from an `FbRoot`. An argument struct
crosses the wire on its own, so it is a root.

- [ ] **Step 1: Write the failing test**

```rust
/// An induced argument struct crosses the wire on its own, so it is a
/// FlatBuffers root: the Rust codec emits its `Payload` impl from the root,
/// and every descriptor's reservation row names that impl's `MAX_SIZE`.
#[test]
fn an_induced_argument_struct_is_a_flatbuffers_root() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 0) else {
        panic!("the `two` command");
    };
    let index = command.request_tuple.as_ref().expect("a request tuple").index;
    let projection = model.flatbuffers.as_ref().expect("a projection");
    assert!(
        projection
            .roots
            .iter()
            .any(|root| root.tuple == Some(index)),
        "the induced argument struct is a root"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p ridl-ir an_induced_argument_struct_is_a_flatbuffers_root`
Expected: FAIL — no root names a tuple.

- [ ] **Step 3: Emit the roots**

`roots` (line 677) maps `package.decls` to one root each. Give it the induced
structs as a second source. The bound is `fb::max_size` over a declaration, so a
tuple needs the tuple-shaped sizer the projection already has —
`tuple_table_bound` — reached through a public wrapper; add one next to
`fb::max_size`:

```rust
/// The largest FlatBuffers buffer a tuple's own table encodes to, the bound
/// `max_size` gives a declaration. `None` when no finite bound exists.
pub fn tuple_max_size(packages: Packages<'_>, tuple: &v2::TupleType) -> Option<u64> {
    Sizer::new(packages).tuple_table_bound(packages.package, tuple)
}
```

Read `max_size` (line 454) and build the `Sizer` the way it does; the wrapper
differs only in which bound it asks for.

Then in `roots`, after the declaration roots:

```rust
let induced = induced_interaction_tuples(package);
let tuple_roots = induced.into_iter().map(|(index, tuple)| {
    let bound = match fb::tuple_max_size(packages, &tuple) {
        Some(size) => v1::fb_root::Bound::MaxSize(u32::try_from(size).unwrap_or(u32::MAX)),
        None => v1::fb_root::Bound::Unbounded(v1::FbUnbounded {
            cause: v1::FbUnboundedCause::Member as i32,
            ..Default::default()
        }),
    };
    v1::FbRoot {
        // An induced struct has no declaration; `tuple` names it.
        declaration: 0,
        tuple: Some(index),
        root: v1::FbRootKind::Own as i32,
        bound: Some(bound),
    }
});
```

`induced_interaction_tuples(package)` returns `(model tuple index, tuple)` for
each induced request and reply, built from the same three functions
`Walk::interactions` uses, with the model index read from the `tuple_index` map
`project` already receives. Thread that map into `roots`, which takes only
`scope` today.

- [ ] **Step 4: Run the test and the suite**

Run: `cargo test -p ridl-ir` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir/src/codegen/flatbuffers.rs crates/ridl-ir/src/codegen/tests.rs
git commit -m "feat(ridl-ir): root an induced request and reply struct

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 9: Emit the codec for a tuple root

**Files:**

- Modify: `crates/ridl-backend-rust/src/codec.rs:820-867` (`Codec::items`,
  `Codec::reachable_tuples`)
- Modify: `crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl`
- Test: `crates/ridl-backend-rust/tests/descriptor_generation.rs`

`payload_impl(root, owner)` takes the owner's Rust name. For a tuple root the
owner is the induced name, which `tuple_name` gives.

- [ ] **Step 1: Extend the backend fixture**

`crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl` has no call with
zero parameters and none with two. Add both to `interface Valve`, which exists
for call-shape tests and whose interactions are not pinned by a descriptor test:

```ridl
interface Valve {
  command open(level: Level)
  query pressure(window: Window): Average
  /// A call with two parameters: its argument type is induced.
  command setBoth(level: Level, window: Window)
  /// A call with none: its argument type is an induced struct with no field.
  query status(): Health
}
```

`the_fixture_compiles_clean` asserts `package.shapes().count() == 7`, which
counts interfaces, not interactions, so it is unaffected.

- [ ] **Step 2: Write the failing test**

Append to `crates/ridl-backend-rust/tests/descriptor_generation.rs`, which
already holds `dense` and `ir::compile_fixture`:

```rust
/// A tuple root gets a `Payload` impl under the induced struct's own name, so
/// a descriptor's reservation row, which names
/// `<T as Payload<FlatBuffers>>::MAX_SIZE`, resolves
/// (driftsys/ridl#713).
#[test]
fn an_induced_argument_struct_has_a_payload_impl() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let source = generate_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_with")
        .rust_source;
    let d = dense(&source);
    assert!(
        d.contains("implridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>forValveSetBothParams")
            || d.contains("impl::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>forValveSetBothParams"),
        "the induced argument struct has a Payload impl"
    );
    assert!(
        d.contains("pubstructValveStatusParams"),
        "and a zero-parameter call induces an empty struct"
    );
}
```

- [ ] **Step 3: Run it and watch it fail**

Run:
`cargo test -p ridl-backend-rust an_induced_argument_struct_has_a_payload_impl`
Expected: FAIL — no such impl and no such struct.

- [ ] **Step 4: Emit the `Payload` impl for a tuple root**

`Codec::items` (line 820) sorts the roots into bounded and withheld, emits the
declaration items for the bounded ones, and then emits the table items of every
reachable tuple — encode, verify, decode and the view, but no `Payload` impl,
which `decl_items` emits for a declaration at lines 1037–1053.

Two edits. First, `check_flatbuffers_bound(self.ctx, root.declaration)` in the
withheld arm reads a declaration index, which a tuple root does not have:

```rust
_ => {
    if root.tuple.is_none() {
        check_flatbuffers_bound(self.ctx, root.declaration)?;
    }
    withheld.push(root);
}
```

and the bounded loop skips a tuple root, which `declaration_of` cannot resolve:

```rust
for root in &roots {
    if root.tuple.is_some() {
        continue;
    }
    let visibility = self.declaration_of(root)?.visibility;
```

Second, in the tuple loop, emit the `Payload` impl when a root names the tuple:

```rust
for index in self.reachable_tuples(&roots) {
    let induced = self.ctx.tuple(index).ok_or_else(|| GenerateError {
        message: "the lowered model names a tuple it does not carry".to_string(),
    })?;
    let table = self.tuple_table(induced)?;
    items.extend(
        self.table_items(tuple_name(induced), induced.visibility, &table)?
            .into_iter()
            .map(|item| allow_dead_code_if_internal(induced.visibility, item)),
    );
    // A tuple a root names crosses the wire on its own, so it
    // carries the `Payload` impl a declaration root carries: the
    // descriptor's reservation row names its `MAX_SIZE`.
    if let Some(root) = roots.iter().find(|root| root.tuple == Some(index)) {
        items.push(self.payload_impl(root, tuple_name(induced))?);
    }
}
```

Third, seed reachability: `reachable_tuples(&roots)` walks from each root's
declaration, so a tuple root reaches nothing today. Add its own index as a seed
at the top of `reachable_tuples`, before the declaration walk, so the tuple and
everything nested inside it is emitted.

- [ ] **Step 5: Run the test**

Run:
`cargo test -p ridl-backend-rust an_induced_argument_struct_has_a_payload_impl`
Expected: PASS.

- [ ] **Step 6: Run the crate's suite**

Run: `cargo test -p ridl-backend-rust` Expected: PASS — except the snapshot and
golden tests, which Task 11 regenerates.

- [ ] **Step 7: Commit**

```bash
git add crates/ridl-backend-rust/src/codec.rs crates/ridl-backend-rust/tests
git commit -m "feat(ridl-backend-rust): emit the codec of a tuple root

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Group D — the Rust backend's face

### Task 10: Read the induced type in the three accessors

**Files:**

- Modify: `crates/ridl-backend-rust/src/descriptors.rs:590-705`
- Modify: `crates/ridl-backend-rust/src/face.rs:285-297`
- Modify: `crates/ridl-backend-rust/src/lib.rs:439-445`
- Test: `crates/ridl-backend-rust/tests/descriptor_generation.rs`

`single_param_type`, `query_param_type` and `query_reply_type` are the only
three places the backend asks what a call's payload is: `face.rs:285-297` and
`lib.rs:439-445` both go through them. Changing these three changes the face,
the descriptors and the skip decision at once.

- [ ] **Step 1: Write the failing test**

```rust
/// A two-parameter call carries a face: its descriptor's `Args` is the
/// induced argument struct, and the interface is not skipped
/// (driftsys/ridl#713).
#[test]
fn a_two_parameter_call_carries_a_face() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = generate_face_with(&package, &[WireEncoding::FlatBuffers])
        .expect("generate_face_with")
        .rust_source;
    let d = dense(&face);
    assert!(
        d.contains("typeArgs=ValveSetBothParams;"),
        "the call's Args is the induced argument struct"
    );
    assert!(
        d.contains("typeArgs=ValveStatusParams;"),
        "a zero-parameter call's Args is the induced empty struct"
    );
    assert!(
        !d.contains("__RIDL_NO_FACE_Valve"),
        "and the interface is not skipped"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p ridl-backend-rust a_two_parameter_call_carries_a_face`
Expected: FAIL — the interface is skipped.

- [ ] **Step 3: Prefer the induced name**

`single_param_type` returns a `&str`, so the induced name must outlive the call.
`tuple_name(induced)` borrows from the model, which the `Ctx` holds, so the
signature gains the context:

```rust
/// The call's argument type: the single declared named parameter, or the
/// induced argument struct the lowering named for every other parameter list
/// (interaction face design, the request-shape rule). Refuses only a shape
/// that has no struct field — an untyped parameter, and a stream.
pub(crate) fn single_param_type<'a>(
    ctx: &Ctx<'a>,
    command: &'a v1::CommandShape,
    member: &str,
) -> Result<&'a str, GenerateError> {
    if let Some(payload) = command.request.as_ref() {
        return Ok(payload_reference(Some(payload)));
    }
    induced_name(ctx, command.request_tuple.as_ref())
        .ok_or_else(|| no_single_param(&command.params, member))
}

/// The Rust name of the induced tuple `reference` names, when it names one.
fn induced_name<'a>(ctx: &Ctx<'a>, reference: Option<&v1::TupleRef>) -> Option<&'a str> {
    let induced = ctx.tuple(reference?.index)?;
    let name = tuple_name(induced);
    (!name.is_empty()).then_some(name)
}
```

`query_param_type` takes the same shape over `query.request` and
`query.request_tuple`; `query_reply_type` over `query.reply_payload` and
`query.reply_tuple`. Thread `ctx` through the four call sites in `face.rs` and
`lib.rs`, each of which already has one.

- [ ] **Step 4: Narrow the two refusal messages**

`no_single_param` names the parameter count, which is no longer a reason.
Replace it with:

```rust
/// Why a call has no argument type. Every parameter list induces one, so the
/// two causes left are a parameter the struct cannot hold.
fn no_single_param(params: &[v1::Param], member: &str) -> GenerateError {
    let untyped = params.iter().any(|param| param.r#type.is_none());
    let message = if untyped {
        format!("interaction `{member}` declares a parameter with no type")
    } else {
        format!("interaction `{member}` declares a stream parameter, which has no payload type")
    };
    GenerateError { message }
}
```

and in `query_reply_payload`, the `Value` arm's message becomes:

```rust
Some(v1::reply::Kind::Value(_)) => Err(GenerateError {
    message: format!("query `{member}`'s reply must be a named type or a tuple"),
}),
```

while the fallible arm keeps its message, which is now the only reply refusal a
corpus interface draws:

```rust
message: format!(
    "query `{member}`'s reply is a fallible `T | E`, which the face does not carry"
),
```

- [ ] **Step 5: Run the test and the crate's suite**

Run: `cargo test -p ridl-backend-rust` Expected: PASS, with snapshot tests
failing where the generated output has grown. Review each snapshot diff: a new
`Client`, `Provider` and `dispatch` for a previously skipped interface is the
change this task makes; anything else is a defect. Accept the snapshots only
after reading them.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-backend-rust/src crates/ridl-backend-rust/tests
git commit -m "feat(ridl-backend-rust): carry a call of any parameter count

The three payload accessors read the induced argument and reply structs the
lowering names, so a call with no parameter, with several, or with one that is
not a named type carries a face. The refusal messages name the two shapes that
are still refused.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 11: Regenerate the checked-in output

**Files:**

- Modify: `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
- Modify: `freeze/generated/veh-cluster/veh.cluster.rs`
- Modify: `crates/ridl-backend-rust/src/snapshots/` as the snapshot tests direct

- [ ] **Step 1: Regenerate**

Run: `cargo xtask freeze update` Expected:
`freeze/generated/veh-cluster/veh.cluster.rs` loses
`__RIDL_NO_FACE_WheelDiagnostics` and gains the interface's descriptors.

- [ ] **Step 2: Read the diff before accepting it**

Run: `git --no-pager diff --stat freeze/` Then read the removed and added hunks
around each `__RIDL_NO_FACE_`. Expected: `__RIDL_NO_FACE_WheelDiagnostics` gone;
`__RIDL_NO_FACE_VehicleStatus` and `__RIDL_NO_FACE_DoorControl` still present —
the contract-clause translator (driftsys/ridl#704) and the fallible reply, both
out of scope.

- [ ] **Step 3: Check the generated crate still compiles and runs**

Run: `just demo` Expected: PASS — each round trip's value is matched, the
generated crates check for `thumbv7em-none-eabihf` with their default features
off, and clippy is clean under `-D warnings`.

- [ ] **Step 4: Check the whole workspace**

Run: `just test` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add freeze/ crates/ridl-backend-rust/tests/generated crates/ridl-backend-rust/src/snapshots
git commit -m "test(ridl-backend-rust): regenerate the checked-in output

WheelDiagnostics carries a face now that a zero-parameter call has an argument
type. VehicleStatus and DoorControl keep their notes: the contract-clause
translator (driftsys/ridl#704) and the fallible reply.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Group E — sizes and reservations

The face works without this group: the Rust descriptor writes a reservation as a
`<T as Payload<FlatBuffers>>::MAX_SIZE` path, which Task 9 made resolve. This
group makes the _model's_ own `request_sizes`, `reply_sizes` and
`Interaction.reservation` state a number instead of "no codec defines the
encoding of this request shape", which is what a plugin reads.

### Task 12: Size a parameter list

**Files:**

- Modify: `crates/ridl-ir/src/projection/size.rs:118-145` (`PayloadShape`,
  `payload_name`)
- Modify: `crates/ridl-ir/src/projection/size.rs:259-273` (`size_state`)
- Modify: `crates/ridl-ir/src/projection/size/proto3.rs:158` (`state`)
- Modify: `crates/ridl-ir/src/projection/size/flatbuffers.rs:23` (`state`)
- Test: `crates/ridl-ir/src/codegen/tests.rs`

Both sizers are keyed on a declaration's name today. Sizing an induced struct
needs a second entry in each: proto3 already has
`tuple_size(def, home, walk,
depth)`; FlatBuffers already has
`tuple_table_bound(home, tuple)`.

- [ ] **Step 1: Write the failing test**

```rust
/// A call with several parameters states a size: the induced argument struct
/// is a message like any other (driftsys/ridl#713).
#[test]
fn a_two_parameter_command_states_a_request_size() {
    let model = lower(&call_package(), &[]);
    let v1::interaction::Shape::Command(command) = call_shape(&model, 0) else {
        panic!("the `two` command");
    };
    assert!(
        matches!(
            state(&command.request_sizes, true),
            v1::size_state::State::Bounded(_)
        ),
        "proto3 states a bound for the induced argument struct"
    );
    assert!(
        matches!(
            state(&command.request_sizes, false),
            v1::size_state::State::Bounded(_)
        ),
        "and so does FlatBuffers"
    );
}
```

The existing tests at `tests.rs:1628` and `tests.rs:1652` assert `undefined()`
for exactly these shapes. They state the behaviour this task changes: update
them in the same step, keeping `undefined()` only where a shape still has no
encoding — a stream parameter and an untyped one.

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p ridl-ir a_two_parameter_command_states_a_request_size`
Expected: FAIL — both states are `Absent(EncodingUndefined)`.

- [ ] **Step 3: Add the tuple shape**

In `size.rs`:

```rust
pub enum PayloadShape<'a> {
    /// A signal or event payload: a type name.
    Named(&'a str),
    /// A fixed payload.
    Field(&'a FieldType),
    /// A command or query request: the parameters.
    Params(&'a [Param]),
    /// A query response.
    Return(&'a ReturnType),
    /// An induced argument or reply struct: the tuple the lowering built
    /// from the parameter list or the reply, sized as the message each
    /// encoding gives it.
    Tuple(&'a TupleType),
}
```

`payload_name` cannot answer for it, so `size_state` branches before calling
`payload_name`:

```rust
if let PayloadShape::Tuple(def) = shape {
    return match encoding {
        Encoding::Proto3 => proto3::tuple_state(home, def, ctx),
        Encoding::FlatBuffers => flatbuffers::tuple_state(home, def, ctx),
        Encoding::ReprC => SizeState::Absent(AbsentCause::EncodingUndefined),
    };
}
```

Add `tuple_state` to each sizer, wrapping the per-encoding function that already
exists — `tuple_size` in `proto3.rs`, `tuple_table_bound` in the FlatBuffers
projection — and narrowing the `u64` the way the existing `state` narrows it,
with `AbsentCause::Overflow` when it does not fit a `u32`.

- [ ] **Step 4: Point the lowering at the new shape**

In `lower.rs`, the command and query arms compute `request_sizes` from
`PayloadShape::Params`. Where the call induces a struct, size that struct
instead:

```rust
let request_shape = match params_as_tuple(&command.params) {
    Some(tuple) if single_param_type(&command.params).is_none() => {
        PayloadShape::Tuple(tuple)
    }
    _ => PayloadShape::Params(&command.params),
};
```

`PayloadShape::Tuple` borrows, so bind the tuple before the shape. Do the same
for the query's request and for its reply.

- [ ] **Step 5: Run the test and the crate's suite**

Run: `cargo test -p ridl-ir` Expected: PASS.

- [ ] **Step 6: Run the compiler's suite, which reads these sizes**

Run: `cargo test -p ridlc` Expected: PASS. `crates/ridlc/tests/layout.rs` reads
`request_sizes` and `reply_sizes`; a failure there is a real disagreement about
a number, not a snapshot.

- [ ] **Step 7: Commit**

```bash
git add crates/ridl-ir/src
git commit -m "feat(ridl-ir): size an induced argument and reply struct

The model states a bound for a request and a reply that is an induced struct,
so an interaction's reservation is a number rather than unsized.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 13: Correct the protocol's prose about absent sizes

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`

Three doc comments state the behaviour Task 12 changed:
`CommandShape.request_sizes`, `QueryShape.reply_sizes`, and
`ReservationState.unsized`, which enumerates the parameter-list form `(a, b)`.

- [ ] **Step 1: Rewrite `CommandShape.request_sizes`**

```proto
// The size state of the request under each wire encoding, for every
// command. Where the request is exactly one named type, the state is that
// type's own, which may be absent under one encoding and present under the
// other: a struct with a member the proto backend refuses, such as an
// optional array, is `ABSENT_CAUSE_REFUSED_MEMBER` under proto3 whatever
// its FlatBuffers state is. A named scalar, an enum and an enum set are
// sized through their proto3 root box message and their FlatBuffers box
// table. Every other parameter list is sized as the induced argument
// struct `request_tuple` names. It is absent under both encodings only
// where no struct exists: a parameter with no declared type, and a stream
// parameter, are each `ABSENT_CAUSE_ENCODING_UNDEFINED`.
```

- [ ] **Step 2: Rewrite `QueryShape.reply_sizes`**

```proto
// The size state of the reply under each wire encoding, for every query. A
// reply that is exactly one named type is sized as `request_sizes`
// describes; a tuple reply and a query with no declared return type are
// sized as the induced reply struct `reply_tuple` names. A fallible
// `T | E` reply is `ABSENT_CAUSE_ENCODING_UNDEFINED`: it is a sum, and no
// codec defines its encoding. A stream reply and an untyped one are
// absent for the same cause.
```

- [ ] **Step 3: Correct `ReservationState.unsized`**

In the `<payload>` enumeration, remove the clause that lists a parameter list
`(a, b)` as a shape with no size, and leave the shapes that are still unsized: a
stream, an untyped payload, and a fallible reply.

- [ ] **Step 4: Check the protocol and the freeze**

Run: `cargo check -p ridl-ir && just freeze-check` Expected: PASS — a doc
comment is not part of the frozen surface, so the dump is unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-ir/proto/ridl/codegen/v1/model.proto
git commit -m "docs(ridl-ir): correct the protocol's prose about absent sizes

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Group F — the records

### Task 14: Amend ADR-0016 and ADR-0023

**Files:**

- Modify: `docs/decisions/ADR-0016-schema-projection-and-the-name-transform.md`
- Modify: `docs/decisions/ADR-0023-interaction-face-generation.md`

- [ ] **Step 1: Add the naming expression to ADR-0016**

ADR-0016 gives the naming expression for a tuple reached from a struct field.
Add the interaction case: an interaction-origin induced type is named
`camel_case(<interface or service>) + camel_case(<interaction>) + <role>`, where
the role is `Params` or `Reply`, and a position inside it extends that prefix by
the same rules a declaration path uses. State that the name is unique at the
package's own scope because an interface name is unique in a package and an
interaction name is unique in an interface.

- [ ] **Step 2: Add the request-shape rule to ADR-0023**

State the rule once: the face's request is always a named type — the single
declared named parameter, or the induced argument struct for every other
parameter list, zero included. State the two reply forms that induce a struct,
and the three shapes still refused: an untyped position, a stream, and a
fallible `T | E` reply, the last with its own issue linked.

- [ ] **Step 3: Check the records**

Run: `just check && just link-check && just doc-path-check` Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add docs/decisions
git commit -m "docs(adr): record the induced call payload rules

ADR-0016 gains the naming expression for an interaction-origin induced type.
ADR-0023 states the request-shape rule once and names the three shapes the
face still refuses.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 15: Update the design reference and the book

**Files:**

- Modify: `docs/design/interaction-face.md`
- Modify: `docs/book/generated-code.md`

- [ ] **Step 1: Update the design reference**

`docs/design/interaction-face.md` is the design the Rust backend cites in its
refusal messages. Replace the passage that makes one named parameter the only
call shape with the rule ADR-0023 now holds, and link to ADR-0023 rather than
restating it in full — a fact is stated once, in the record that owns it.

- [ ] **Step 2: Update the book**

`docs/book/generated-code.md` describes what a consumer sees. Add the induced
argument struct: its name, that its fields are the parameters in declaration
order and carry their docs, and that a call with no parameter sends a struct
with no field. Every `ridl` fence in `docs/book/` is compiled, so a new example
must declare its own `package` and draw no diagnostic its fence does not name.

- [ ] **Step 3: Check the book**

Run:
`just book-check && just link-check && just doc-path-check && just story-id-check`
Expected: PASS.

- [ ] **Step 4: Run the book's example harness**

Run: `cargo test -p ridl-cli --test book_examples` Expected: PASS — every fence
in the book compiles.

- [ ] **Step 5: Commit**

```bash
git add docs/design docs/book
git commit -m "docs(ridl): describe the induced argument struct

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 16: Open the follow-up issue and close the loop

- [ ] **Step 1: Open the fallible-reply issue**

```bash
gh issue create \
  --title "feat(ridl-ir): induce a reply enum for a fallible T | E reply" \
  --body "A query whose reply is \`T | E\` has no single payload type, so the Rust backend skips the whole interface and writes \`__RIDL_NO_FACE_<Interface>\`. \`DoorControl\` in the services-workspace corpus is the case.

driftsys/ridl#713 induced a named payload for every product-shaped request and reply. A fallible reply is a sum, so it needs an induced enum rather than a struct, and it touches how \`ridl-rt\` carries a reply and ADR-0023 decision 6. That is why it was left out of #713.

Design: docs/wip/2026-10-13-induced-call-payloads-design.md, decision D-5." \
  --label enhancement
```

- [ ] **Step 2: Run the full gate**

Run: `just verify` Expected: PASS. This is the first run of the whole gate; it
covers the members the per-task commands did not — `wasm-check`, `compat-check`,
`install-check`, `gate-parity` and the commit lint.

- [ ] **Step 3: Open the pull request**

```bash
git push -u origin feat/713-induced-call-payloads
gh pr create --fill
```

State in the description which two markers the change removes, which two remain
and why, and link driftsys/ridl#713 and the follow-up issue.

---

## Notes for the implementer

**The three accessors are the chokepoint.** `single_param_type`,
`query_param_type` and `query_reply_type` in
`crates/ridl-backend-rust/src/descriptors.rs` are the only places the Rust
backend asks what a call's payload is. `face.rs` and `lib.rs` both go through
them. Do not add a fourth path.

**Two functions are called `single_param_type`.** `lower.rs`'s answers whether
the parameter list is exactly one named type; `descriptors.rs`'s answers what
the Rust backend should send. They live in different crates and Task 7 imports
the first into the projection. Keep them apart when reading a stack trace.

**A backend never invents a name.** If you find yourself spelling
`format!("{interface}{member}Params")` in a backend, stop: the name belongs to
`InducedName.rust`, which lowering writes and ADR-0016 governs.

**The corpus is the test set.** Every shape in the design's test table is
already in `crates/ridlc/tests/corpus` or `evals/corpus`. If you think you need
a new fixture, check that table first.

**Two markers must survive.** `__RIDL_NO_FACE_VehicleStatus` (driftsys/ridl#704)
and `__RIDL_NO_FACE_DoorControl` (the fallible reply) are out of scope. A diff
that removes either one is a defect in this change.
