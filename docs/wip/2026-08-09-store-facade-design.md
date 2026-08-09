# The Store Facade — the generated interaction API in Rust and over the C ABI

| Field     | Value                                                                                                                        |
| --------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Status    | design, for review — nothing ratified                                                                                        |
| Date      | 2026-08-09                                                                                                                   |
| Origin    | what a consumer and a provider actually call, once a signal carries its envelope and a coherent set is observable            |
| Scope     | the layering, the cell, the envelope fields per kind, provenance, numeric addressing, and the operation vocabulary           |
| Companion | `2026-08-03-rpc-response-bound-design.md` §4.4 and `2026-08-03-schema-projection-design.md` §4 — the store this note faces   |

A bare section reference — §3.1, §4.4, §11 — is to the **ridl Language
Reference**. References to this document are marked _above_ or _below_.

**What this note does not do.** It does not propose a runtime. AGENTS.md records
that there is no runtime in this workspace, and ADR-0013 open item 2 already
names `ridl-rt` as the candidate home if a store is built. This note specifies
the API surface and the generated artifacts. Whether the store is built here is
a separate decision, and several findings below are worth acting on before it
is.

## 1. The question

ADR-0015 decision 11 makes a provided interface the generation unit: its signals
become one store behind a single generation counter, its interactions become one
dispatcher. Neither exists. What does exist is `ridl-backend-rust`, which emits
a consumer trait and a provider trait per interface, and an extern-C header that
declines the interaction layer entirely.

Two questions follow. What does a consumer of a signal actually hold — value
alone, or value with the envelope §3.1 promises? And what shape survives
projection onto a C ABI, where there are no traits and no generics?

## 2. Two drifts found before designing anything

Both are in shipped code, and both are worth repairing whether or not the rest
of this note is adopted.

**The envelope reaches no generated code.** §3.1 states plainly that "generated
subscriber/caller APIs expose the envelope alongside the value (value +
provenance + envelope)". `SignalHandle::read` returns `(T, Provenance)` and
`EventHandle::subscribe` hands over a bare `&T`. The timestamp and the sequence
number are specified, and unreachable.

**A consumer cannot read two signals.** The generated accessor is

```rust
fn current_speed(&mut self) -> &mut dyn SignalHandle<Speed>;
```

so one handle borrows the whole consumer face mutably. Two signals cannot be
held at once, one signal's accessor cannot be given to a task, and a coherent
multi-signal read — the thing the generation counter exists for — is not
expressible through the API at all.

## 3. The model

The facade's counterparty is always a **store**, never a socket and never a bus.

- **Read** is always a local cell read: cheap, infallible, no round trip. §4.4
  already places the last-value cache in the binding, so this holds however the
  store is fed — a remote peer never turns a read into a request.
- **Write** lands in the store cell first. What happens next is the binding's
  business: where the store is shared, the write is the propagation; where it is
  not, the binding sends the value to a peer store.
- **The transport appears in no signature.** Shared block or socket is a
  deployment choice, invisible to the generated API.

In the remote case there are two stores, and the binding replicates between
them. This is ADR-0015 decision 1's absorption principle applied to the
generated face rather than to the contract: the facade states the obligation,
the binding supplies the mechanism.

A seqlock is therefore not part of this design's API. It is how a **shared**
store guards a cell against a concurrent writer; a process-local store fed by a
socket pump needs none.

## 4. Layering

The numeric core is written once, by hand. Only the typed skins are generated.

```text
  generated Rust facade              generated C header
  (typed traits, structs)      (id constants, static inline wrappers)
          |                                   |
          |                        ridl_* extern "C" — hand-written
          |                                   |
          +--------- the store ---------------+
                 cells, table, generation, binding
```

- **The store** — hand-written, generic, addressed by numeric id. It does not
  change when a contract changes.
- **The `extern "C"` layer** — hand-written, the function set of §9 below. It
  does not change when a contract changes either, which is the point of numeric
  addressing.
- **The generated part** — per contract, and only constants, the identity table,
  payload types, and typed wrappers. On the C side those wrappers are
  `static inline` and compile away.

**The Rust facade does not route through the C ABI.** It calls the store's typed
Rust API directly, with no pointer or size erasure. The C ABI is a sibling face
that erases types, not a layer everything passes through. What survives from
"design the C side first" is a design constraint rather than a call path: the
store's operation set must be expressible in C terms — numeric addressing,
copy-out, no generics in the primitives — which is what stops a Rust API being
built that cannot project.

**This resolves ADR-0013 open item 1 for the C header**, in the direction
decision 1's own test points. A backend's ceiling follows from what its target
can represent, and C represents an opaque handle, a cell, and a function pointer.
The header's present line — `not represented in the C ABI — interactions are a
binding concern, not a data layout` — is replaced. The genuine limit stays and
is stated in §10 below.

## 5. The cell

### 5.1 Metadata is out of band

The envelope and the provenance sit beside the payload bytes, never inside them.

**§3.1 has already decided the principle.** "Because the envelope always exists,
contract payloads should not re-declare it: a payload field carrying publication
time or a frame counter draws an info lint (RIDL-406)." An author is linted for
putting a timestamp or a counter in a payload; generated code putting them there
would do exactly what the language forbids the author to do.

Three further reasons:

- **Provenance cannot live in the payload without corrupting it.** §4.5 requires
  an invalid cell to keep the last good value. If the invalid marker were a
  payload field, marking it means rewriting bytes of the value the store is
  obliged to preserve.
- **The envelope must evolve independently of every wire schema.** In band,
  adding an envelope field is a schema change to every contract on every
  backend, and proto and FlatBuffers would each need field numbers for platform
  vocabulary.
- **A check need not copy.** Metadata is fixed and small; a payload may be
  kilobytes. Separated, a consumer polling twenty signals for change reads
  twenty sequence numbers and copies only what moved.

The cell layout is therefore `[seqlock][meta][payload]`, with the seqlock
bracketing **both** meta and payload — otherwise a reader can pair new metadata
with an old payload, which is the one risk that separating them introduces.

The metadata is one struct for every kind, with the provenance field carrying
`NONE` where it does not apply (§7):

```c
#define RIDL_PROV_NONE     0   /* events — occurrences are not state (§4.4) */
#define RIDL_PROV_INIT     1
#define RIDL_PROV_LIVE     2
#define RIDL_PROV_INVALID  3

typedef struct {
    uint8_t  provenance;
    uint64_t sequence;          /* of the value held                     */
    uint64_t generation;        /* step that wrote the value held        */
    int64_t  timestamp_us;      /* when the held value was published     */
    int64_t  invalid_since_us;  /* 0 unless provenance is INVALID        */
} ridl_cell;
```

**The single exception is a transport with no metadata channel.** §4.5 already
says that on CAN and AUTOSAR, invalidity is carried "in-band as the SNA/invalid
sentinel", and ADR-0013 decision 7 generalises the shape: a target that can
carry the fact but not the structure realises it in band and **must not surface
that value in the generated application API**. So the rule is one sentence: out
of band in the store and in every API; in band only as a transport realisation
where the transport has no alternative, and never visible to the application.

### 5.2 The payload encoding is open

Two candidates, recorded rather than settled — see §11 open item 1.

**Fixed layout (`#[repr(C)]`, as the backend emits today).** A read is a
`memcpy`. Cannot hold a payload with a string, an optional, or a collection in a
shared block, which is the same restriction the C header already applies.

**FlatBuffers as the cell payload.** typl's glossary defines `bounded` as "the
invariant that every typl composite has a statically known maximum wire size",
which is why collections require bounds and recursion is an error. A cell can
therefore be a fixed-capacity buffer sized to the payload's worst case, and
variable-layout payloads become storable in a shared block. FlatBuffers' own two
tiers map onto typl's split: a `struct` for scalar and all-fixed composites,
inline and with no vtable, and a `table` for anything carrying a string, an
optional, or a collection. The gain is that the cell bytes and the propagated
bytes become the same bytes, so store-to-store propagation is a raw copy with no
encode or decode at either end.

Three costs, and one precondition. Cells in a shared block are sized to the
worst case, so the block is the sum of worst cases. FlatBuffers has alignment
requirements the block layout must satisfy. The generated Rust stops being a
public-field struct and becomes accessors. And **ADR-0013 decision 6 makes typl
§17.11 a precondition for any FlatBuffers backend** — widening `[0..255]` to
`[0..300]` flips `uint8` to `uint16` with no edit to any declaration, which on
FlatBuffers is a hard wire break. The digest of §8.3 converts a silent misread
into a refusal to attach, which is the right failure, but it does not supply the
compatible-evolution path a width floor would.

Note that this is a different projection from the wire schema, not a replacement
for it. `ridl-backend-proto` already projects the typl surface onto proto3
(E9.8). The cell layout and the wire schema are two projections of one IR for
two jobs.

## 6. The envelope, per kind

Three numbers, and they are not interchangeable.

| Field         | Grain     | Job                                              |
| ------------- | --------- | ------------------------------------------------ |
| `timestamp`   | instance  | freshness, TTL, latency, `dt` in consumer math   |
| `sequence`    | channel   | loss detection, deduplication, E2E protection    |
| `generation`  | interface | which publication step this value belonged to    |

**`generation` is new.** It is the store-side counter ADR-0015 decision 11 and
`2026-08-03-schema-projection-design.md` §4 already place on the inner table,
carried on the cell so that it survives propagation. Without it, a receiving
store fed one channel at a time can only number its own arrival order, and a
coherent set becomes a reassembly. With it, cells that carry step 41 belonged to
step 41 whatever delivered them.

One precision. **Comparing two cells' generations does not answer whether their
values coexisted.** ADR-0015 decision 9 is explicit that in a given step some
cells are written and others are not, and an unwritten cell still holds a value
the provider held. So `generation` answers which step last wrote this cell, and
coexistence is answered by the snapshot of §9, not by comparing cells.

**None of the three is the seqlock counter.** A seqlock counter counts writes
into one block of memory. When propagation drops a sample the sender's sequence
advances and a receiving store's seqlock does not, so a reader trusting the
seqlock as the frame number sees a contiguous run and concludes nothing was
lost — which is the detection §3.1 promises and AUTOSAR E2E consumes.

The jobs differ per kind:

| Kind      | What the sequence is for           | `generation` | Application-visible  |
| --------- | ---------------------------------- | ------------ | -------------------- |
| `signal`  | loss detection, change detection   | yes          | yes                  |
| `event`   | loss detection — the only evidence | yes          | yes                  |
| `command` | deduplication and retry            | —            | no (§6.1)            |
| `query`   | request and reply correlation      | —            | reply timestamp only |

**On an event the sequence carries more weight than on a signal.** A signal that
loses a sample is repaired by the next one, because latest-value means the gap
heals. §5 makes an occurrence individually meaningful and explicitly not
coalesced, so a lost occurrence is gone and the sequence gap is the only
evidence it existed. ADR-0015 decision 21 pairs with this: ring depth is
`ceil(max / min)`, so a consumer knows how many occurrences can be alive, and
the sequence tells it which it received.

**On an RPC the sequence is runtime-internal**, and the reference says so twice.
§6.1: the delivery acknowledgment "exists so the runtime can implement retries,
delivery supervision, and duplicate suppression (envelope sequence numbers,
§3.1)" and "never reaches the contract surface, and is not application-visible
as a return value". §3.1 says the same of request and reply correlation. §7.2 is
what makes correlation mandatory rather than convenient: providers "may answer
concurrently and out of order", so a reply cannot be matched by arrival. The
reply's **timestamp** is worth exposing, because that is what a caller measures
against the §9.3 response bound.

## 7. Provenance

Provenance answers how much to trust the value in hand. §4.4 makes the channel
never empty, so a read always yields something and a second field must say what
it is: `init` (seeded from the contract, never published), `live` (published),
`invalid` (the channel violated its constraints; the last good value is still
held). §4.4 ends with "events carry **none** of this — occurrences are not
state", so provenance is signal-only.

**Invalid does not overwrite the value.** §4.5 requires the last good value to
remain accessible, so a rejected publish keeps the bytes and changes the
channel's state. That forces two timestamps: if the transition overwrote the
cell's timestamp, the age of the last good value would be lost, and that age is
what §9's `max` is evaluated against and what a consumer needs to decide whether
to hold or fail over.

**Init has no envelope.** §3.1 stamps at the sender on publication and an init
value was never published, so sequence, generation and timestamp are unset while
provenance is `init`. A store must not report an init cell as fresh because it
was seeded a moment ago; freshness is undefined until the first publication, and
provenance is what says so.

**Validation happens at publish.** The provider's store checks the payload
against the typl constraints and, on violation, keeps the value, marks the
channel invalid, and stamps the transition. §4.5 gives no error channel back to
the publisher — it is "a provider bug surfacing through telemetry" — so `publish`
still succeeds and the observability hook is where it surfaces. A `watch`
callback fires on the transition, because §4.5 propagates invalidity "to every
subscriber like any other state change"; the callback contract must say so,
since the value is unchanged and the wakeup otherwise looks spurious.

### 7.1 The Rust reading is a sum type, not a pair

```rust
pub enum Reading<T> {
    /// Seeded from the contract; no provider has published (§4.4).
    /// No envelope — it was never sent, so freshness is undefined.
    Init(T),
    /// A published value (§4.3).
    Live(T, Envelope),
    /// The channel violated its constraints (§4.5). The last good value
    /// and its envelope are still here; `since` is the transition.
    Invalid { last_good: T, envelope: Envelope, since: Timestamp },
}
```

The shipped `fn read(&self) -> (T, Provenance)` permits `let (speed, _) =
h.read();`, which is the silent-stale-data failure §4.5 exists to prevent,
available in one keystroke. A sum type makes the value unreachable without
naming the case, so §4.4's "consumers that must not act on a mere init value can
tell" becomes must tell rather than may tell. It also gives each state exactly
its own data, which removes the fields that would otherwise sit unused: no
transition timestamp on a live cell, no envelope on an init value.

**The C side cannot express this**, and the asymmetry is honest — the ABI
erases, the Rust skin refines. C receives a flat struct and a documented
validity rule per provenance value: on `INIT` the envelope fields are zero and
mean never published; on `INVALID`, `timestamp_us` belongs to the last good
value and `invalid_since_us` to the transition. `RIDL_PROV_NONE = 0` is what an
event's metadata carries, so a reader gets an honest "not applicable" rather
than a plausible-looking `Init`.

### 7.2 Freshness is not provenance

A value can be `live` and long past its §9 `max` refresh ceiling, and no
provenance value says so. Provenance answers where a value came from; freshness
answers how old it is, and they are independent — folding them into one enum
would make "invalid and also long overdue" inexpressible.

ADR-0015 decision 11 already has the store evaluating each cell "against its own
`max` staleness bound", so the platform computes this. It has nowhere to report
it. Leaving it to each consumer means every consumer reimplements a comparison
it can get wrong in three ways: the wrong clock domain, the wrong bound, or
treating an init cell's absent timestamp as infinitely stale. Recorded as a gap
in §11.

The timestamp is still exposed, because it is an input to the consumer's own
arithmetic rather than to its trust decision. A controller needs the real
interval between samples, not the nominal period — §4's "the latest sample is
the truth; intermediate samples may be missed" means the observed interval is
often not the declared one, and integrating with the declared period is wrong by
exactly the amount the timestamp would have told you. §3.1 makes the arithmetic
sound: `int64` microseconds on the PTP epoch, TAI, "continuous, leap-second-free,
monotonic", stamped at the sender and never re-stamped.

Two consequences for a consumer computing an interval. `Init` carries no
envelope, so the first reading after subscribing has no predecessor and the
first interval must be skipped — the sum type of §7.1 makes this a compile error
rather than a convention. And `Invalid` holds the previous timestamp, so a naive
loop across an invalid period computes a zero interval, which is a division by
zero in any derivative; the sum type forces that branch too.

One limit is not this design's to fix. The stamp marks publication, not
measurement. ADR-0012 flags it: for a transducer with a response time, "every
downstream model computing with the time of the cause is silently wrong by the
transducer's lag". Between two samples of one signal the lag cancels. Across two
sensors with different lags it does not, and that is what ADR-0012 decision 3's
**latency of correspondence** obligation exists to declare. It is E3 work.

## 8. Addressing

### 8.1 One packed identifier

```c
typedef uint32_t ridl_id;                      /* (interface << 16) | ordinal */
#define RIDL_ID(iface, ord)  (((uint32_t)(iface) << 16) | (uint16_t)(ord))
```

The two halves already exist in the model: the interface id of ADR-0015 decision
15 (1-based by declaration order, an inline shape at slot 1) and the interaction
ordinal of §11 (1-based, one sequence per interface, kind-blind). ADR-0013
decision 3 already requires every backend to emit that identity table; numeric
addressing makes the table the mechanism rather than documentation.

Three things stay out of it.

**The service.** `ridl_attach` binds it, so the identifier is service-relative —
which matches ADR-0015's split of the service as the addressing unit and the
interface as the generation unit.

**The kind.** §11 contemplates an interaction's kind changing, and `ridl-diff`
detects it. Packing the kind into the identifier would make a kind change
rewrite the identifier, and the identifier is the one thing meant to be stable:
an old peer would get "unknown id" instead of "ordinal 3 is a signal now, you
asked for an event". The identifier must survive the change in order to report
it. There is also no performance case for packing, since a call must look the
entry up anyway to validate the payload size.

**A second numbering.** ADR-0013 decision 3 is explicit that per-kind
identifiers such as SOME/IP's method and event ids are "binding transformations
applied over that identity, never a second numbering".

### 8.2 The identity table carries more than the numbering

```c
typedef enum {
    RIDL_KIND_RETIRED = 0,   /* tombstone — slot held, never reused (§11) */
    RIDL_KIND_SIGNAL  = 1,
    RIDL_KIND_EVENT   = 2,
    RIDL_KIND_COMMAND = 3,
    RIDL_KIND_QUERY   = 4,
    RIDL_KIND_FIXED   = 5,
} ridl_kind;

typedef struct {
    ridl_id     id;
    const char *name;     /* source name — §11 pairs by name */
    uint8_t     kind;
    uint8_t     family;   /* ADR-0012 decision 6 */
    uint32_t    size;     /* payload width; what the size check compares */
} ridl_entry;
```

All five kinds share the one ordinal sequence, `fixed` included — the backend
already emits `fixed capabilities — ordinal 11`. `RIDL_KIND_RETIRED = 0` earns
its slot because ADR-0013 decision 3 requires retired ordinals be held against
reuse, and a table entry means a peer asking for a retired identifier gets
_retired_ rather than _unknown_: different mistakes, different remedies.
`family` is populated rather than reserved — every declaration today is
`dispatch`, and E3's spellings fill the rest in without an ABI change.

The store also needs the **seed bytes** per signal, which is new generated
output: §4.4 seeds each cell at creation from the type's init or the signal's
`= value` override, and RIDL-109 makes a payload with no derivable init a
compile error, so the value always exists. Today it appears only in a doc
comment.

### 8.3 A digest, because numeric addressing has no compiler behind it

Addressing by number means ordinal 3 means whatever the producer's contract said
it meant. A peer built from another revision reads the wrong cell with no
symptom. So `ridl_attach` carries a digest over the identity table — names,
identifiers, kinds, and payload sizes — and a mismatch refuses.

This does not reopen §11's rejection of an in-language version block. §11
rejected a **hand-maintained** version pair inside the source as "a second source
of truth that drifts". A digest derived by the backend from the contract is the
move `ridl-diff` already makes.

The digest is **per interface**, and the addressing shape decides it rather than
a separate argument: the identifier's high half selects the interface, so a
store holds a digest per interface slot and checks on first use of any identifier
in that slot. Interfaces version independently, and ADR-0015 makes the interface
the generation unit.

## 9. The operation vocabulary

The verbs come from the reference, not from convention. §4.2 and §4.3 have the
provider **publish** a signal; §5.1 and §5.2 have occurrences **raised** ("the
provider must not raise occurrences faster than `min`"); §5.1 has consumers
**subscribe**. The backend already emits `publish_current_speed` and
`raise_door_opened`.

| Kind      | Consumer          | Provider  |
| --------- | ----------------- | --------- |
| `signal`  | `read`, `watch`   | `publish` |
| `event`   | `subscribe`       | `raise`   |
| `command` | `send`            | `serve`   |
| `query`   | `request`         | `serve`   |
| `fixed`   | `read`            | —         |

Four notes on the choices.

**`read`, not `get`.** `get` is already the corpus's query verb — `getFaultPage`
in Appendix A is a query — and it implies fetching from elsewhere, when §4.4's
point is that the value is already here.

**`watch` and `subscribe` are deliberately different words.** §4.4 makes
subscribing to a signal deliver a value **immediately** — init, then latest —
while §5.1 makes subscribing to an event deliver "only occurrences raised after
the subscription. No cache, no replay." One replays current state and the other
never does, and two words put that at the call site. This is ADR-0012 decision
4's constraint-bundling test.

**`dispatch` is rejected for `command`.** ADR-0012 names an entire interaction
family `dispatch`, and ADR-0015 decision 11 names the generated router the
dispatcher. A third meaning as a verb is the collision test that rejected
`control`, `observe` and `apply` as keywords.

**`fixed` costs no new verb.** §8 says bindings expose it "as a plain accessor",
so it rides `read` with no watch and no publish side.

**The verbs are the C core's, not the typed layer's.** At the typed layer the
interaction name already is the method: the backend generates `async fn
set_gear(...)` on the consumer and `async fn on_set_gear(...)` on the provider,
with no verb prefix.

```c
typedef uint64_t ridl_sub;

typedef void (*ridl_signal_fn)(const void *value, size_t size,
                               const ridl_cell *meta, void *user);
typedef void (*ridl_event_fn) (const void *value, size_t size,
                               const ridl_cell *meta, void *user);
typedef void (*ridl_command_fn)(const void *args, size_t args_size,
                                void *user);
typedef int32_t (*ridl_query_fn)(const void *args,  size_t args_size,
                                 void       *reply, size_t reply_size,
                                 void *user);

ridl_store *ridl_attach(const char *service, uint64_t digest);

int32_t ridl_read      (const ridl_store *s, ridl_id id,
                        void *out_value, size_t value_size,
                        ridl_cell *out_meta);            /* signal, fixed  */
int32_t ridl_peek      (const ridl_store *s, ridl_id id,
                        ridl_cell *out_meta);            /* metadata only  */
int32_t ridl_read_all  (const ridl_store *s, uint16_t iface,
                        void *out_block, size_t block_size,
                        uint64_t *out_generation);       /* coherent block */
int32_t ridl_watch     (const ridl_store *s, ridl_id id,
                        ridl_signal_fn f, void *user, ridl_sub *out_sub);
int32_t ridl_publish   (ridl_store *s, ridl_id id,
                        const void *value, size_t value_size);
int32_t ridl_subscribe (const ridl_store *s, ridl_id id,
                        ridl_event_fn f, void *user, ridl_sub *out_sub);
int32_t ridl_raise     (ridl_store *s, ridl_id id,
                        const void *value, size_t value_size);
int32_t ridl_send      (ridl_store *s, ridl_id id,
                        const void *args, size_t args_size);
int32_t ridl_request   (ridl_store *s, ridl_id id,
                        const void *args, size_t args_size,
                        void *out_reply, size_t reply_size);
int32_t ridl_serve_command (ridl_store *s, ridl_id id,
                            ridl_command_fn f, void *user);
int32_t ridl_serve_query   (ridl_store *s, ridl_id id,
                            ridl_query_fn f, void *user);
int32_t ridl_cancel    (ridl_store *s, ridl_sub sub);
```

Every subscription-creating call returns a status and yields its handle through
an out-parameter, so a failed registration is distinguishable from a valid
handle whose value happens to be zero. `ridl_query_fn` returns a status because
a provider can fail to produce a reply for a Stratum 3 reason; a **functional**
failure is not that, and travels in the reply payload as §10.1 requires.

**Reads copy out.** A shared store cannot lend a pointer into a block a writer
may overwrite, so the caller supplies storage and the retry is internal. A
process-local store is happy to copy, so one shape serves both.

**`read_all` is the coherent read**, and it is the same shape as a seqlock retry
loop — read the generation, copy the block, re-read, retry — which is why the
shape is right rather than a coincidence. It is also the only operation that
answers whether a set of values coexisted, per §6 above.

**Two `serve` functions, not one.** The handler signatures differ — a command
handler returns nothing, a query handler must produce a reply — and with a single
registration taking an untyped function pointer, registering a command handler
against a query ordinal compiles cleanly and corrupts the stack when the runtime
calls it. Distinct typedefs make the compiler catch it; the ordinal's kind in the
table catches the rest at registration rather than at first call.

**The `int32_t` status is never a functional error.** §10.1 makes functional
errors data: a fallible query returns `T | E` and the error arm travels in the
reply payload. The status is Stratum 3 — unknown identifier, size mismatch,
wrong kind, throttled, no provider. A `DiagError` arriving as a status code would
collapse the three strata of §10.1 to §10.3 into one.

## 10. The write side

**Publishing is fire-and-forget and infallible.** Both alternatives contradict
the reference.

A `Result` for constraint violations misstates §4.5: a violating value is not
rejected, the channel transitions to invalid and that invalidity propagates. The
write succeeded — what it published was invalidity — and §4.5 gives no error
channel back to the publisher.

A delivery future contradicts §4's latest-value rule, under which a sample that
fails to propagate is superseded by the next one, so retrying a stale sample is
wrong by construction. It would also make the shared-store and socket cases
behave differently, breaking the transport-invisible property of §3. And the
language already has the construct for needing to know: §6.1's `command`, whose
acknowledgment is exactly this.

**`ridl_send` returning does not mean the command executed.** ADR-0015 decision 3
makes a command's `max` a response bound on acceptance, and §6.1's delivery
acknowledgment is runtime-internal. §6.1 gives the answer for a caller who needs
the outcome: use a `query`.

**What the FFI face cannot carry.** Under the fixed-layout option of §5.2, a
payload with a string, an optional, or a collection is Rust-only, as are a
variable-length query reply (`FaultPage { faults: Vec<FaultEvent> }` has no
stated size) and streams (§12) — a `<FwBlock>` parameter or a streaming query is
not a single copy-out, so it is excluded rather than approximated. Under the
FlatBuffers option the first restriction lifts; streams remain excluded. The
Rust facade carries all of it either way.

## 11. Open

1. **The cell payload encoding** — fixed layout or FlatBuffers (§5.2). The
   FlatBuffers option needs a `ridl-backend-flatbuffers` and, before it, an
   answer to typl §17.11 per ADR-0013 decision 6.
2. **The publication step bracket.** ADR-0015 decision 9 grounds coherence in
   rmdl's topological step, so a scheduled provider's step is the platform's and
   needs no API. A hand-written or FFI provider has no scheduler, and either
   brackets explicitly or gets a generation per write. Whether the bracket is
   exposed, and to whom, is undecided.
3. **The handle names.** `Signal<T>` / `SignalMut<T>`, `SignalConsumer<T>` /
   `SignalProvider<T>`, and `SignalHandle<T>` / `ProvidedSignal<T>` are the live
   candidates. Note the pair is a supertrait relation rather than two peers,
   because a provider reads its own channel: §4.4 seeds the cell before the
   first publication, §4.3's change-driven publication requires comparing
   against the current value and knowing when it last published, and §4.5 keeps
   the last good value accessible. Any name asserting write-only — `Writer`,
   `Publisher` — misdescribes it.

## 12. Gaps found in the reference

Four, independent of whether this design is adopted. Each is worth an issue.

1. **The RPC sequence has no caller scope.** §3.1 defines the sequence as
   "per-channel monotonic counter, assigned by the sender per provider
   instance", which is written for pub/sub, where §4.2 gives every flow exactly
   one owning provider. On an RPC the initiator is the consumer — ADR-0015
   decision 3 says so — and there may be many. Two callers both sending sequence
   5 collide, and a provider deduplicating on the sequence alone drops the
   second caller's genuine command as a duplicate. The key must be (caller
   identity, sequence), which means either a sender identity in the envelope on
   the RPC kinds or a dedup table scoped per connection. Neither is stated, and
   the failure direction is dangerous in exactly the safety context §6.1 invokes
   when it calls a rejected command "a _detected_ event rather than a silent
   one".
2. **An invalid event payload is unspecified.** §4.5's invalid state is
   state-shaped: a channel transitions and holds a last good value. An
   occurrence has no channel state, and §5 does not say what happens when an
   event's payload violates its typl constraints. Dropping it and letting the
   consumer see a sequence gap is the only behaviour consistent with §5's
   no-cache rule — and it reuses §3.1's "sequence gaps make loss detectable" — at
   the cost that a consumer cannot distinguish a provider emitting a bad payload
   from a dropped occurrence without telemetry. The reference should say so
   either way.
3. **Freshness is evaluated and unreportable.** §9 makes `max` a contract term
   and ADR-0015 decision 11 has the store evaluate each cell against it, but no
   API surface carries the verdict and no provenance value expresses it (§7.2).
4. **§3.1's promise about generated APIs is unmet**, and the current signature
   makes ignoring the envelope easier than reading it (§2).

## References

- `docs/specification/ridl-language-reference.md` — §3.1 (the envelope, the time
  base, RIDL-406), §4.2 to §4.5 (direction, publication, init and last-value,
  invalid state), §5.1 and §5.2 (occurrences, TTL), §6.1 and §6.2 (the
  acknowledgment, provider-side rejection), §7.1 and §7.2 (the reply,
  concurrency), §8 (`fixed`), §9 and §9.3 (timing, the RPC bounds), §10.1 to
  §10.4 (the three strata, failure management), §11 (ordinals and evolution),
  §12 (streams), §14.5 (the service)
- `docs/specification/typl-language-reference.md` — the `bounded` invariant,
  §17.11 (the deferred width floor)
- ADR-0012 — decision 3 (the four correspondence obligations, and the latency of
  correspondence this design cannot yet express), decision 4 (the admission
  tests applied here to generated names), decision 6 (`family` and `shape` as IR
  fields), decision 9 (fail-closed classification)
- ADR-0013 — decision 1 (a backend is classified by what its target can
  represent), decision 3 (the identity table), decision 6 (typl §17.11 as a
  FlatBuffers precondition), decision 7 (in-band realisation, never surfaced),
  open items 1 and 2
- ADR-0015 — decision 1 (absorption), decision 3 (the RPC bounds and the
  initiator), decision 9 (production coherence and the step), decision 10
  (delivery coherence as a demand on every binding), decision 11 (the store and
  the dispatcher), decision 15 (interface ids), decision 21 (ring depth)
- `docs/wip/2026-08-03-rpc-response-bound-design.md` §4.4 and
  `docs/wip/2026-08-03-schema-projection-design.md` §4 — the store and the
  generation counter this note faces
- `crates/ridl-backend-rust/src/interact.rs`, `src/c_header.rs` — the shipped
  consumer and provider traits, and the header's present refusal of the
  interaction layer
