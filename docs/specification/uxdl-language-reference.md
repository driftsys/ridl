# uxdl Language Reference

**User-Experience Description Language** — the user-interaction layer of the
RIDL family: typed, transport-neutral contracts between viewmodels and views
(`display` · `input` · `action` · `fetch` · `fixed`), over the typl vocabulary
and the shared `interact` core.

Version: 0.1.0 — Draft

> **Provenance.** uxdl is the sibling of ridl over the family's `interact` core
> (concept note §3): the same interaction machinery, profiled for the user
> boundary instead of the system boundary. Its precursor is **uxil**, the
> MarkSpec UX Interaction Language (surfaces, states, eleven interaction verbs)
> — mined as prior work in Appendix D. The concept note's placeholder vocabulary
> (`display`/`activate`/`toggle`) is settled here. Everything ridl defines over
> the core — timing with defaults, init values, invalid-state propagation, the
> implicit envelope, sender timestamping, system time, the three error strata,
> ordinal evolution — is **inherited, not restated**; this document specifies
> only where uxdl profiles differently.

---

## Table of Contents

1. [Scope and Position in the Family](#1-scope-and-position-in-the-family)
2. [Lexical Additions](#2-lexical-additions)
3. [The View Contract Model](#3-the-view-contract-model)
4. [Display](#4-display)
5. [Input](#5-input)
6. [Action and Its Refinements](#6-action-and-its-refinements)
7. [Fetch](#7-fetch)
8. [Fixed](#8-fixed)
9. [View States](#9-view-states)
10. [Timing in the User Boundary](#10-timing-in-the-user-boundary)
11. [Errors at the User Boundary](#11-errors-at-the-user-boundary)
12. [Identity and Evolution](#12-identity-and-evolution)
13. [What uxdl Is Not](#13-what-uxdl-is-not)
14. [Conventions](#14-conventions)
15. [Diagnostics](#15-diagnostics)
16. [Open Questions](#16-open-questions)

- [Appendix A — Full Example](#appendix-a--full-example)
- [Appendix B — Codegen Targets](#appendix-b--codegen-targets)
- [Appendix C — Formal Grammar (EBNF)](#appendix-c--formal-grammar-ebnf)
- [Appendix D — Prior Art Survey](#appendix-d--prior-art-survey)
- [Appendix E — Coverage Analysis: UI Architecture Patterns](#appendix-e--coverage-analysis-ui-architecture-patterns)
- [Appendix F — Glossary](#appendix-f--glossary)

---

## 1. Scope and Position in the Family

### 1.1 What uxdl is

uxdl answers one question: _how does the user interact with the system?_ A uxdl
`view` is the **single source of truth for the seam between a viewmodel and its
rendering** — what state the view displays, what the user can do, what data can
be fetched, with types, ranges, units, timing, and failure semantics all carried
by the same machinery that serves system contracts.

A `.uxdl` file accepts view declarations plus everything typl accepts. It
rejects system-interaction declarations (ridl), behaviour (rmdl), and
architecture (rsdl).

### 1.2 One core, two boundaries

ridl and uxdl are **two profiles of one `interact` core** — the same six
primitives, aimed at different audiences:

| `interact` primitive   | ridl (system boundary) | uxdl (user boundary)              |
| ---------------------- | ---------------------- | --------------------------------- |
| continuous state value | `signal`               | `display`                         |
| discrete occurrence    | `event`                | `input`                           |
| fire-and-forget action | `command`              | `action` (+ refinements)          |
| stateful mutation      | command-over-state     | `toggle` (an `action` refinement) |
| request / response     | `query`                | `fetch`                           |
| provisioned constant   | `final`                | `fixed`                           |

Everything the core defines is shared and **inherited verbatim from the ridl
reference**: optional timing with the configurable `[100ms..1000ms]` default
(ridl §9.1), init values and the never-empty channel (ridl §4.4, typl §5.8),
invalid-state propagation with provenance (ridl §4.5), the implicit envelope
with sender timestamping and system time (ridl §3.1), the three error strata
with errors-as-data (ridl §10), delivery acknowledgment beneath actions (ridl
§6.1), ordinals + `reserved` evolution (ridl §11), and failure-management
scoping (ridl §10.4). Where this document is silent, the ridl rule applies to
the corresponding kind.

### 1.3 Standalone contracts — wiring is rsdl's job

A `view` is a **standalone contract**. It does not name, import, or bind the
services that will feed it — connecting `CruisePanel.speedReadout` to
`CruiseControl.currentSpeed` is **wiring**, and wiring is rsdl:

```
# rsdl (manifest sketch)
instance panel : CruisePanel
  bind speedReadout <- cruise.currentSpeed
  bind resumeButton -> cruise.setLever(LeverCmd.RESUME)
```

Consequences: a view is designable before any service exists, testable against
injectors (the test plane drives displays and observes actions with no backend),
reusable across systems whose services differ, and the dependency lattice stays
acyclic — uxdl depends on typl only. The concept-note sketch's in-source `binds`
clause is **rejected** for v0.1 (decision on record): it coupled UI contracts to
specific services and duplicated rsdl's role.

---

## 2. Lexical Additions

uxdl inherits the family lexical conventions (typl §2) and ridl's activated
token classes (duration literals, the `@` timing sigil).

Keywords **used** by the uxdl profile, beyond typl's set:

```
view  display  input  action  fetch  fixed
activate  toggle  select  adjust  dismiss
states  during
```

Reserved for future uxdl use (registry entries, no v0.1 semantics): `navigate`,
`scroll`, `drag`, `observe`, `surface`, `agent`. All other family keywords
remain reserved in every profile (typl §1.4). Note `when` is _not_ used for
state gating — it is anticipated rmdl clock vocabulary; uxdl uses `during`.

---

## 3. The View Contract Model

### 3.1 Provider and consumer

A `view` contract has the same two roles as any interact contract, cast for the
user boundary:

- the **viewmodel** _provides_ the view contract: it publishes displays,
  executes actions, answers fetches, holds fixed values. It may be hand-written,
  or an rmdl model realising the view (one behaviour language, both interaction
  profiles — concept note §3)
- the **renderer** (the view proper — a Compose screen, a SwiftUI view, a web
  component, a cluster HMI) _consumes_ it: subscribes to displays, raises inputs
  and actions, calls fetches

Direction note: in ridl, occurrences (`event`) flow provider → consumer; in
uxdl, occurrences (`input`) flow **consumer → provider** — the user is on the
consuming side, and what occurs is _their_ gesture. This is the one directional
asymmetry between the two profiles; everything else maps one-to-one.

### 3.2 Inherited machinery, user-boundary meaning

The inherited semantics acquire UI-native readings — this is the payoff of one
core:

| Core rule                           | System reading (ridl)                         | User reading (uxdl)                                                                                 |
| ----------------------------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| init value, never-empty channel     | subscriber has state before first publication | **skeleton/placeholder state is contractual** — every display has a defined pre-live value          |
| provenance `init/live/invalid`      | cache trust                                   | render placeholder / render live / render **error state**                                           |
| freshness SLO (`@[..max]`)          | staleness alert                               | stale-data indicator on the display                                                                 |
| debounce (`@[min..]`)               | publication rate cap                          | **input/adjust debouncing declared in the contract**, not hand-coded per widget                     |
| delivery ack (Stratum 2 nack)       | command rejected, detected                    | **refused action is a UI affordance** — the runtime knows the tap was rejected and why              |
| envelope timestamp (sender-stamped) | publication time                              | **gesture time** — an input is stamped when the user acted, not when the viewmodel got around to it |
| result unions on fetch              | typed RPC failure                             | typed error rendering for on-demand data                                                            |
| ordinals + `reserved`               | wire evolution                                | view contracts evolve without breaking shipped renderers                                            |

### 3.3 Payloads

Payloads are named typl types, always — the anti-coupling rule holds. A view
never defines shapes; `Speed` on a cluster display is the _same type_ the
powertrain service publishes, with its unit, range, and step intact — which is
what makes end-to-end (sensor → service → viewmodel → pixel) type identity
checkable.

---

## 4. Display

A **continuous view-state value** — what the renderer shows. The uxdl profile of
the core's state primitive; all of ridl §4 applies (timing §9.1 defaults, init
§4.4, invalid §4.5, last-value, envelope).

```ridl
display speedReadout : Speed
display engagedLamp  : boolean = false
display nowPlaying   : TrackInfo @[50ms..2s]
```

- Single named-type payload; optional timing (default `[100ms..1000ms]`);
  optional bare `= value` init override (ridl §4.4 — no keyword)
- The renderer always has something to draw: init value → placeholder/skeleton,
  live value → content, invalid state → error presentation, staleness → stale
  indicator. **All four renderer states are derived from the contract**, not
  invented per screen
- A display is the viewmodel's truth, not the widget's echo — do not declare a
  display for state the renderer owns purely locally (scroll position, focus)
  unless the viewmodel genuinely consumes it

---

## 5. Input

A **discrete user occurrence carrying data** — consumer → provider. The uxdl
profile of the core's occurrence primitive; event rules apply (range timing
only, TTL, throttle) with direction reversed.

```ridl
input searchText  : SearchQuery
input pinEntry    : PinCode @[100ms..2s]
```

- Use `input` when the occurrence's payload is the point (text typed, a code
  entered, a barcode scanned). When the point is _invoking an operation_, use
  `action` (§6)
- The `min` bound is a **declared debounce** on the gesture source; `max` is the
  occurrence TTL — a stale gesture (queued behind a frozen UI) is discarded by
  the binding rather than applied late. Both evaluated on the envelope's sender
  (gesture-time) timestamp
- Continuous gesture streams (scroll, drag) are **not** v0.1 inputs — reserved,
  §16.3

---

## 6. Action and Its Refinements

A **fire-and-forget user operation** — the uxdl profile of `command`. All of
ridl §6 applies: no return, no functional-error channel, `require` permitted,
runtime delivery acknowledgment beneath it, outcome observed through displays.

One kind, six surface keywords. `action` is the generic form; five
**refinements** are actions with declared gesture semantics and payload rules:

| Keyword    | Semantic                              | Payload rule                                                     |
| ---------- | ------------------------------------- | ---------------------------------------------------------------- |
| `action`   | generic operation                     | any parameters                                                   |
| `activate` | invoke/trigger (tap, press, confirm)  | no parameters (UXDL-204)                                         |
| `toggle`   | flip a binary state                   | no parameters (UXDL-204); pair with a boolean display            |
| `select`   | choose one item from a collection     | exactly one parameter — the key (UXDL-205)                       |
| `adjust`   | set a continuous value (slider, dial) | exactly one parameter — the value, a ranged/unit type (UXDL-205) |
| `dismiss`  | close/cancel/leave a transient        | no parameters (UXDL-204)                                         |

```ridl
activate playButton
toggle   muteButton                       // pairs with: display muted : boolean
select   trackItem(id: TrackId)
adjust   volumeSlider(level: Ratio)
dismiss  errorBanner during ERROR
```

- Refinements are **semantics, not new kinds**: all six compile to the core's
  action primitive (one codegen path, one evolution rule, one ack). What a
  refinement adds is machine-readable gesture meaning — for renderer scaffolding
  (a `toggle` scaffolds a switch, an `adjust` a slider bound to the parameter's
  typl range/step), for the test plane (an `adjust` fuzzes its range
  mechanically), and for accessibility tooling
- uxil's key templates (`/item:{track_id}`) become **typed parameters** —
  `select trackItem(id: TrackId)` — the key is vocabulary, not string template
- A refused action (precondition or state gate fails) is a **negative ack** to
  the renderer's runtime (ridl §6.1): the UI can disable, shake, or explain — a
  rejected tap is detected, never silently ignored
- CQRS holds at the user boundary: actions mutate, displays report.
  `toggle muteButton` does not "return" the new state — `display muted` does

---

## 7. Fetch

**Request/response** on demand — the uxdl profile of `query`. All of ridl §7 and
§10.1 apply: non-void return, result unions for functional failure, streams,
`require`/`ensure`.

```ridl
fetch trackDetails(id: TrackId): TrackDetailsResult      // result union — fallible
fetch searchSuggest(prefix: SearchQuery): <Suggestion>   // stream — e.g. autocomplete
```

Use `fetch` for data the view needs _on demand_ (detail panes, pagination,
autocomplete) — not for state the view always shows; that is a `display`.
Linters flag a fetch polled on a timer as a probable display (UXDL-206).

---

## 8. Fixed

A **static capability** — the uxdl profile of `final`. All of ridl §8 applies.

```ridl
fixed supportedLocales : [LanguageCode; 1..32]
fixed maxVolume        : Ratio
```

Capabilities the renderer may cache unconditionally for the software-instance
lifetime: locales, feature availability, limits that scaffold the UI once.

---

## 9. View States

A view may declare a **state set** — its coarse operating modes — by referencing
a typl enum (vocabulary lives in typl, always):

```ridl
enum MediaHomeState { LOADING = 0, READY = 1, ERROR = 2 }

view MediaHome states MediaHomeState {
  ...
}
```

Semantics:

- `states E` induces an **implicit display** `state : E` — first in ordinal
  order, init = `E`'s init value (typl §5.8: the value `0`, i.e. the first
  declared state). The current state is ordinary view-state: published by the
  viewmodel, rendered, subscribed, replayed, injected like any display. There is
  no second state machinery
- Any interaction may be **gated** with `during`:

```ridl
activate playButton during READY
dismiss  errorBanner during ERROR
```

`during S1, S2` means the interaction is _available_ only in those states, and
gates **consumer-initiated interactions only** (`input`, actions, `fetch`) — a
display cannot be gated; state-dependent content is the viewmodel's business.
The gate's failure semantics follow each kind's channel: a gated **action** or
**fetch** raised in the wrong state is a Stratum 2 `PRECONDITION_FAILED` —
negatively acked / error-returned, renderable as disabled; a gated **input** has
no ack channel (occurrences never do), so an out-of-state occurrence is
**discarded** by the providing binding, recorded by observability — the
event-TTL discipline applied to availability

- `during` on a view without `states` is an error (UXDL-207); naming a value not
  in the state enum is an error (UXDL-208); `during` on a `display` is an error
  (UXDL-209) — availability gating is for consumer-initiated interactions;
  state-dependent _content_ is the viewmodel's business
- **Transitions are not declared.** Which state follows which is behaviour
  (rmdl) or implementation — the contract declares the states and what is
  available in them, not the graph. (uxil's per-surface `@states` mapped here;
  its navigation targets did not — §16.1)

---

## 10. Timing in the User Boundary

ridl §9 applies unchanged — including the configurable default `[100ms..1000ms]`
and the `ridl.toml [defaults]` mechanism. What changes is what the bounds _mean
to a UI engineer_:

- `display … @[16ms..500ms]` — don't repaint-spam faster than a frame; guarantee
  a refresh at least twice a second; staleness beyond 500ms is contractually
  _stale_ and the renderer may indicate it
- `input … @[100ms..2s]` — declared debounce at the source; gestures older than
  2s (a wedged queue) are discarded, not applied late
- Strict periodic `@Xms` is as rare in UI as an isochronous render contract — it
  exists for cluster/HMI cases where a display genuinely drives a fixed-rate
  loop

Freshness, debounce, TTL are all evaluated on envelope sender timestamps (ridl
§3.1) under system time — a gesture's age is measured from the _gesture_, which
matters exactly when the UI thread was the bottleneck.

---

## 11. Errors at the User Boundary

Inherited three-strata model (ridl §10), with user-boundary surfaces:

| Stratum                        | uxdl surface                                                                                                                                                                                      |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 — functional (result unions) | `fetch` returns a result union; the error arm is a _renderable, typed_ failure (retry affordance, error pane) — never a toast built from a string                                                 |
| 2 — contract (derived)         | invalid display payload → channel invalid state → error presentation (ridl §4.5); refused action/input (`INVALID_VALUE`, `PRECONDITION_FAILED`) → negative ack → disabled/refused affordance (§6) |
| 3 — transport                  | connection/backend loss is runtime material; its _user-facing_ consequence is typically staleness (§10) and failure-management policy (ridl §10.4) — never contract syntax                        |

Failure _management_ — what the UI does in degraded modes, offline fallbacks,
error screens as system policy — is the same §10.4 body of work; uxdl
contributes the vocabulary (states like `ERROR` are just enum values) and the
detection, not the policy.

---

## 12. Identity and Evolution

ridl §11 applies verbatim: implicit 1-based ordinals across a view's declared
interactions (one sequence, all kinds), append-only, `reserved` tombstones, no
in-language version, `ridl-diff` as the gate. The implicit `state` display of §9
is **ordinal 0 — outside the declared sequence** — so adding a `states`
declaration to an existing view is an _additive_ change, not a renumbering of
every interaction (removing one remains breaking, as removing any display is). A
shipped renderer keeps working against an evolved viewmodel exactly as a shipped
service consumer does.

---

## 13. What uxdl Is Not

Recording the fence explicitly, because UI languages attract scope:

- **Not layout or styling.** No geometry, no widget taxonomy, no theming, no
  design tokens. A `toggle` may _scaffold_ a switch; nothing in the contract
  says switch, checkbox, or voice command
- **Not navigation.** Screen-flow is a graph over views — topology, not
  contract. Deferred with `navigate` reserved (§16.1)
- **Not the renderer's private state.** Focus, scroll position, animation
  progress stay out unless the viewmodel consumes them
- **Not localization.** Displayed _text_ is typed data (`Message`, `Label`);
  which language string renders is a presentation concern
  (`fixed supportedLocales` + locale state is as far as the contract goes)
- **Not accessibility metadata — yet.** A11y roles/labels may become a
  labels-profile or annotation concern (§16.4); refinements already carry the
  machine-readable gesture semantics a11y tooling needs

---

## 14. Conventions

- Displays are nouns (`speedReadout`, `muted`); inputs are noun-ish payloads
  (`searchText`); actions are widget-ish imperatives named for the _control_
  (`playButton`, `volumeSlider`) — the contract names the interaction point, not
  the implementation
- Pair every `toggle` with the boolean display it flips; pair `adjust` with a
  display of the same type when the value is shown
- Prefer refinements over generic `action` — they carry scaffolding and test
  semantics for free
- Keep views cohesive: one view per viewmodel seam, not one view per widget;
  child-panel decomposition belongs to renderer composition until real reuse
  demands contract-level splitting
- State sets small and coarse (loading/ready/error/editing) — fine-grained modes
  are viewmodel behaviour, not contract

---

## 15. Diagnostics

Coded `UXDL-`, same lifecycle rules as typl §16. Timing, init, invalid, and
evolution rules inherited from ridl apply to the corresponding uxdl kinds and
are reported under their ridl/typl codes; the table below is uxdl-specific.

### 15.1 Structure (UXDL-1xx)

| Code     | Rule                                                                                                        | Severity |
| -------- | ----------------------------------------------------------------------------------------------------------- | -------- |
| UXDL-101 | type declaration inside a `view` body                                                                       | error    |
| UXDL-102 | duplicate interaction name within a view                                                                    | error    |
| UXDL-103 | `states` reference is not a typl `enum`                                                                     | error    |
| UXDL-104 | interaction named `state` colliding with the implicit state display                                         | error    |
| UXDL-105 | system-interaction keyword (`signal`, `event`, `command`, `query`, `final`, `interface`) in `.uxdl` context | error    |

### 15.2 Kinds and Refinements (UXDL-2xx)

| Code     | Rule                                                                                          | Severity |
| -------- | --------------------------------------------------------------------------------------------- | -------- |
| UXDL-201 | stream `<T>` on `display` or `input`                                                          | error    |
| UXDL-202 | explicit return type on an action (any refinement)                                            | error    |
| UXDL-203 | `fetch` returning `()` or a bare `error` type                                                 | error    |
| UXDL-204 | parameters on `activate`, `toggle`, or `dismiss`                                              | warning  |
| UXDL-205 | `select`/`adjust` without exactly one parameter                                               | error    |
| UXDL-206 | `fetch` that mirrors a display (heuristic: polled, parameterless)                             | info     |
| UXDL-207 | `during` on a view with no `states` declaration                                               | error    |
| UXDL-208 | `during` names a value not in the state enum                                                  | error    |
| UXDL-209 | `during` on a `display` — availability gating applies to consumer-initiated interactions only | error    |
| UXDL-210 | `toggle` with no boolean display in the same view (heuristic)                                 | info     |

---

## 16. Open Questions

1. **Navigation.** A nav graph over views (uxil: `navigate ->` targets, `screen`
   kind). It is topology — candidate homes: an rsdl-adjacent flow manifest, or a
   later uxdl extension once the contract layer is proven. `navigate` stays
   reserved.
2. **Two-way binding sugar.** `display` + `adjust` pairs (slider bound to shown
   value) are the two-way binding idiom; whether a declared pairing
   (`adjust volume for volumeShown`) earns syntax or stays convention (§14) —
   evidence first.
3. **Continuous gesture streams.** `scroll`/`drag` (uxil verbs) are neither
   occurrences nor state values — they are bounded-lifetime flows. Candidate:
   input refinements over the stream container, or rmdl-adjacent. Reserved.
4. **Accessibility metadata.** Roles, a11y labels, reading order — likely an
   `@labels` profile vocabulary first (same promotion path as safety properties,
   ridl §10.4), keywords only if earned.
5. **Non-visual surfaces.** uxil's `agent` kind (voice/conversational surfaces,
   no visual) — the display/input/action decomposition appears to hold (prompts
   are displays, utterances are inputs); needs a worked example before claiming
   it.
6. **Per-state timing.** Whether a display's bounds may differ per view state
   (`@[16ms..100ms] during PLAYING`) or timing stays state-independent. Deferred
   — compose views if rates genuinely differ.
7. **interact-core promotion.** Which of ridl's §3–§11 rules move verbatim into
   a core spec both profiles cite (the §1.2 inheritance list is the candidate
   set) — settle when the core is extracted for rmdl's benefit.

---

## Appendix A — Full Example

```ridl
package veh.hmi.media

import veh.media.TrackInfo
import veh.media.TrackId
import veh.media.TrackDetails
import veh.common.Ratio

// --- vocabulary (typl, package level) ---

type SearchQuery : string [0..128]

enum MediaHomeState { LOADING = 0, READY = 1, ERROR = 2 }

error enum MediaError {
  NOT_AVAILABLE = 0
  RESTRICTED    = 1
}

union TrackDetailsResult {
  ok  : TrackDetails
  err : MediaError
}

// --- the view contract ---

/**
 * Media home screen — viewmodel/renderer seam.
 * @labels QM, PUBLIC
 */
view MediaHome states MediaHomeState {

  // implicit: display state : MediaHomeState = LOADING

  /// Currently playing track — skeleton until live (init provenance)
  display nowPlaying : TrackInfo @[50ms..2s]

  /// Mute state — pairs with `toggle muteButton`
  display muted : boolean = false

  /// Search box occurrences, debounced 100ms at the source, stale after 2s
  input searchText : SearchQuery @[100ms..2s]

  activate playButton  during READY
  activate nextButton  during READY
  toggle   muteButton
  select   trackItem(id: TrackId) during READY
  adjust   volumeSlider(level: Ratio)
  dismiss  errorBanner during ERROR

  /// On-demand detail pane — typed failure, renderable error arm
  fetch trackDetails(id: TrackId): TrackDetailsResult

  /// Autocomplete suggestions as a stream
  fetch searchSuggest(prefix: SearchQuery): <Label>

  fixed supportedLocales : [LanguageCode; 1..32]
  fixed maxVolume        : Ratio
}
```

Wiring (rsdl manifest sketch — _not_ uxdl):

```
instance mediaHome : MediaHome
  bind nowPlaying   <- media.playback.currentTrack
  bind playButton   -> media.playback.play()
  bind trackDetails -> media.catalog.getDetails
  deploy on ecu.cockpit
```

---

## Appendix B — Codegen Targets

uxdl bindings target **UI frameworks**, not transports — the viewmodel side
still rides ridl-style transports when it lives across a broker (rsdl decides);
the renderer side is generated against the framework's reactive idiom:

| uxdl                           | Kotlin / Compose                                                                       | Swift / SwiftUI                             | TypeScript / React & web                                           | rmdl-realised viewmodel |
| ------------------------------ | -------------------------------------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------ | ----------------------- |
| `view`                         | `interface XxxViewModel` + `@Composable` scaffold                                      | `ObservableObject` protocol + View scaffold | hook `useXxxViewModel()` / custom element                          | WASM component export   |
| `display`                      | `StateFlow<T>` (seeded with init)                                                      | `@Published var` (seeded)                   | signal/store with initial value                                    | output flow             |
| provenance `init/live/invalid` | sealed `DisplayState<T>`                                                               | enum wrapper                                | tagged union                                                       | flow status             |
| `input`                        | callback → `SharedFlow`                                                                | closure → subject                           | event handler prop                                                 | input flow              |
| `action` + refinements         | `fun play()`, `toggle` → `Switch` scaffold, `adjust` → `Slider(range, step)` from typl | funcs; `adjust` → `Slider` bound to range   | handlers; `adjust` → `<input type="range" min max step>` from typl | action flow             |
| `during` gating                | `enabled = state == READY` derived                                                     | `.disabled(...)` derived                    | `disabled` prop derived                                            | observer                |
| `fetch`                        | `suspend fun`, result union → sealed                                                   | `async throws`-free `Result`                | promise of tagged union                                            | request flow            |
| `fixed`                        | `val`                                                                                  | `let`                                       | constant                                                           | provisioned             |

The typl vocabulary drives widget scaffolding: an `adjust` parameter's
range/step/unit becomes the slider's min/max/step and its unit label — the same
declaration that sized the CAN signal sizes the slider.

---

## Appendix C — Formal Grammar (EBNF)

The uxdl profile adds to the typl grammar (typl Appendix E; shared productions —
`timing`, `init_value`, `attr_block`, `reserved`, `param_list`, `stream_type`,
`return_type` — as in the ridl grammar, Appendix C there):

```ebnf
definition    = [ "internal" ] ( typl_definition | view_def ) ;

view_def      = doc_comment? "view" CamelCase_id [ "states" type_ref ]
                "{" { ux_interaction sep? } "}" ;

ux_interaction = display_def | input_def | action_def | fetch_def | fixed_def | reserved ;

display_def   = doc_comment? "display" camelCase_id ":" type_ref init_value? timing? ;
input_def     = doc_comment? "input"   camelCase_id ":" type_ref timing? during_clause? ;

action_def    = doc_comment? action_kw camelCase_id [ "(" param_list ")" ]
                during_clause? attr_block? ;
action_kw     = "action" | "activate" | "toggle" | "select" | "adjust" | "dismiss" ;

fetch_def     = doc_comment? "fetch" camelCase_id "(" param_list ")" ":" return_type
                during_clause? attr_block? ;

fixed_def     = doc_comment? "fixed" camelCase_id ":" final_type ;

during_clause = "during" SCREAMING_SNAKE_ID { "," SCREAMING_SNAKE_ID } ;
```

---

## Appendix D — Prior Art Survey

| Source                              | Taken / rejected                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **uxil (MarkSpec)** — the precursor | Taken: the closed interaction-verb idea (11 verbs → 5 refinements + reserved set), surface states (→ `states` + `during`), the discipline of a corpus registry with stable diagnostics. Rejected/superseded: prose-embedded declaration surfaces (uxdl lives in `.uxdl` files); untyped string key templates `{track_id}` → typed `select` parameters; surface kinds `screen/panel/agent` → one `view` construct (non-visual surfaces §16.5); `navigate ->` targets → deferred (§16.1); the event dictionary prose → doc comments |
| **MVVM (XAML/WPF lineage)**         | the seam itself: view ↔ viewmodel with observable state and commands. uxdl is that seam made an explicit, transport-neutral, typed contract; two-way binding deliberately decomposed into display + adjust/input (§16.2)                                                                                                                                                                                                                                                                                                          |
| **Elm architecture / Redux**        | the discipline that UI = f(state) and user intent is discrete messages: displays are the model projection, inputs/actions are the messages. Rejected: the single global store — a view is a scoped contract                                                                                                                                                                                                                                                                                                                       |
| **Jetpack Compose / StateFlow**     | `StateFlow`'s seeded-value requirement is exactly the never-empty channel (init value) — the binding maps 1:1; unidirectional data flow validates CQRS at the user boundary                                                                                                                                                                                                                                                                                                                                                       |
| **SwiftUI**                         | `@Published`/`ObservableObject` as the display binding; SwiftUI's implicit equatable-skip matches debounce-on-change semantics                                                                                                                                                                                                                                                                                                                                                                                                    |
| **Qt QML**                          | properties/signals/slots ≈ display/input/action — the oldest complete precedent; validates that one core serves both embedded HMI and desktop                                                                                                                                                                                                                                                                                                                                                                                     |
| **React (hooks/signals)**           | props-down-events-up = displays down, inputs/actions up; the hook is the natural binding artifact                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **AUTOSAR HMI / cluster practice**  | the reason timing, init, invalid, and staleness are _contractual_ at the user boundary: a cluster telltale has legally-relevant init and invalid renderings                                                                                                                                                                                                                                                                                                                                                                       |
| **ARIA / accessibility tooling**    | consumes exactly what refinements encode (role semantics: toggle→switch, adjust→slider); drives §16.4                                                                                                                                                                                                                                                                                                                                                                                                                             |

---

## Appendix E — Coverage Analysis: UI Architecture Patterns

Can uxdl express what UI-contract authors reach for? ✓ covered, ≈ covered
differently, ✗ not expressible (deliberate or open).

| Pattern construct                                            | uxdl equivalent                                          | Status                                       |
| ------------------------------------------------------------ | -------------------------------------------------------- | -------------------------------------------- |
| observable view-state (MVVM property, StateFlow, @Published) | `display` (+ init, provenance, freshness)                | ✓ richer                                     |
| command (MVVM ICommand, Compose lambda)                      | `action` + refinements, with ack                         | ✓                                            |
| command `canExecute`                                         | `during` gating / `require` → derived `enabled`          | ✓ derived, not hand-wired                    |
| two-way binding                                              | display + adjust/input pair                              | ≈ decomposed, deliberate (§16.2)             |
| async data loading (suspend/async, React Query)              | `fetch` with result union                                | ✓ typed failure                              |
| loading/error/empty screen states                            | `states` enum + implicit state display + init provenance | ✓                                            |
| skeleton/placeholder UI                                      | init value + `init` provenance                           | ✓ contractual                                |
| stale-while-revalidate indicators                            | freshness SLO on display                                 | ✓ contractual                                |
| list virtualization / item identity                          | `select` with typed key parameter                        | ≈ (identity yes, virtualization is renderer) |
| input debounce/throttle (RxJS, per-widget code)              | timing bounds on `input`/`adjust`                        | ✓ declared once                              |
| navigation / routing                                         | —                                                        | ✗ deferred §16.1                             |
| drag/scroll gesture streams                                  | —                                                        | ✗ reserved §16.3                             |
| form validation                                              | typl constraints + Stratum 2 (`INVALID_VALUE` on input)  | ✓ derived from vocabulary                    |
| optimistic updates                                           | — (viewmodel behaviour, observable via displays)         | ≈ relocated to rmdl/impl                     |
| theming/design tokens                                        | —                                                        | ✗ deliberate (§13)                           |
| a11y roles/labels                                            | refinement semantics now; metadata open                  | ≈ §16.4                                      |
| voice/conversational surfaces                                | display/input/action decomposition, unproven             | ≈ open §16.5                                 |

**Verdict.** The MVVM working set is covered with less hand-wiring than the
frameworks themselves require — `canExecute`, debounce, skeletons, staleness,
and typed load-failure all _derive_ from contract declarations instead of being
re-implemented per screen. The honest gaps are navigation and continuous
gestures (both deferred with reserved keywords), and the unproven claim is
non-visual surfaces. What no surveyed pattern has: the same vocabulary type
flowing from sensor to slider with unit/range/step intact, and UI evolution
guarded by the same `ridl-diff` gate as the services beneath it.

---

## Appendix F — Glossary

Family terms (profile, core, envelope, init value, provenance, result union,
ordinal, `reserved`, system time, SSOT, …) are defined in the typl and ridl
glossaries and mean the same here. uxdl-specific:

| Term               | Definition                                                                                                                                      |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| **view**           | the top-level uxdl construct: the typed contract at the viewmodel ↔ renderer seam                                                               |
| **viewmodel**      | the provider of a view contract — publishes displays, executes actions, answers fetches; possibly an rmdl model                                 |
| **renderer**       | the consumer of a view contract — the framework-side view proper (Compose/SwiftUI/web/HMI); subscribes, raises inputs and actions               |
| **display**        | continuous view-state (the core's state primitive at the user boundary) — never-empty, provenance-carrying, freshness-bounded                   |
| **input**          | a discrete user occurrence carrying data, flowing consumer → provider — the one directional asymmetry vs ridl                                   |
| **action**         | a fire-and-forget user operation (core command); acknowledged by the runtime, refused visibly, outcome observed via displays                    |
| **refinement**     | one of `activate · toggle · select · adjust · dismiss` — an action with declared gesture semantics and payload rules; semantics, not a new kind |
| **fetch**          | on-demand request/response (core query); fallible via result unions                                                                             |
| **fixed**          | a static capability (core final) — cacheable for the software-instance lifetime                                                                 |
| **view states**    | the coarse operating modes of a view — a typl enum inducing an implicit `state` display                                                         |
| **`during`**       | availability gating of an interaction to named states; compiles to a `require` on the state display                                             |
| **skeleton state** | the renderer's pre-live presentation — contractual in uxdl, because every display has an init value and `init` provenance                       |
| **gesture time**   | the envelope timestamp of an input/action — sender-stamped at the user's act, the basis of debounce and TTL                                     |
| **wiring**         | connecting a view's interactions to services — rsdl's job, never declared in a view (§1.3)                                                      |

---

_End of uxdl Language Reference v0.1.0 — Draft._
