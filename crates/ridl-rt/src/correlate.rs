//! The caller-side call table and the waiter registry a runtime keeps behind
//! [`Wakeable`](crate::port::Wakeable) (ADR-0021 decision 15).
//!
//! Every runtime with asynchronous replies keeps a table of the calls it has
//! sent and not yet released, and every runtime that implements `Wakeable`
//! keeps the wakers its handles registered. These two types are that storage,
//! written once so that each runtime does not write it alone. Both are pure
//! data structures: they allocate nothing, hold no lock, and wake nothing. A
//! runtime puts them behind its own lock, and every operation that would wake
//! a task returns the waker instead, so the runtime wakes it after releasing
//! that lock and no waker runs under the runtime's mutex.
//!
//! [`Table`] holds `N` calls. A [`Correlation`] is `(generation << 16) | slot`:
//! the slot index in the low 16 bits and the slot's generation above it, so
//! `N` is at most 65536 and 48 bits of generation remain. A slot is reclaimed
//! by [`Table::forget`] alone — at once for a settled call, and at the
//! settlement for a call in flight — and its generation advances when it is,
//! so the correlation the slot had before answers as unknown. The table stores
//! each call's outcome status, one `Interest::Outcome` waker, and one
//! `Interest::Stream` waker for each of up to [`STREAM_WAKERS`] streams of the
//! call, but no reply bytes and no chunks: a runtime keeps those in storage of
//! its own indexed by [`Table::slot`].
//!
//! A settlement, a forget and a reclaim each stop or change every stream of
//! the call, so after each [`Table::settle`] and [`Table::forget`] that did
//! not answer `Unknown` the runtime takes the slot's stream wakers with
//! [`Table::take_streams`], under the same lock, and wakes them after
//! releasing it. The stream wakers are a
//! separate hand-back, not a field of [`Settled`] or [`Forgotten`], so those
//! two enums keep the shape a runtime already matches on.
//!
//! [`Waiters`] holds one waker per kind of key a handle stores — `Slot`,
//! `Event` and `Claim` — whatever interface an `Event` or `Claim` key names,
//! so a change to any key of that kind takes it (ADR-0021 decision 13). It
//! does not hold a `Stream` or a `ClaimStream` waker: each of those is kept
//! with its call or its claim, one for each stream.
//!
//! `N` is checked at compile time: a table of more than 65536 slots does not
//! build.
//!
//! ```compile_fail,E0080
//! const TOO_MANY: ridl_rt::correlate::Table<65537> = ridl_rt::correlate::Table::new(None);
//! ```
//!
//! ```
//! const ENOUGH: ridl_rt::correlate::Table<65536> = ridl_rt::correlate::Table::new(None);
//! ```

use core::task::Waker;

use crate::contract::StreamNo;
use crate::error::CallError;
use crate::port::{Correlation, Interest};

/// The `Interest::Stream` wakers one slot of a [`Table`] holds, one for each
/// stream of the call: the reply stream and up to three stream parameters.
///
/// A cell stays occupied by its stream until [`Table::take_stream`] or
/// [`Table::take_streams`] takes the waker. A registration on a fifth stream
/// of the same call, while all four cells are occupied, is not stored and is
/// handed back, so its waiter is woken at once on every registration: the
/// task reads its stream again and registers again, which is a busy loop
/// until a cell is freed. It does not wait forever, but it does not sleep.
pub const STREAM_WAKERS: usize = 4;

/// The bits of a correlation that hold the slot index.
const SLOT_BITS: u32 = 16;
const SLOT_MASK: u64 = (1 << SLOT_BITS) - 1;
/// The 48 bits a generation keeps: it wraps to 0 after `2^48 - 1` reclaims of
/// one slot.
const GENERATION_MASK: u64 = (1 << (64 - SLOT_BITS)) - 1;

/// What [`Table::settle`] did, and the waker it hands back to be woken.
///
/// The runtime wakes the waker after releasing its lock. Unless the answer
/// is [`Unknown`](Settled::Unknown), it also takes the slot's stream wakers
/// with [`Table::take_streams`] and wakes them; on `Unknown` it does not,
/// because the slot of a stale correlation may hold another call that is
/// still live. On
/// [`Reclaimed`](Settled::Reclaimed) it also wakes every `Interest::Slot`
/// waiter it holds and frees whatever it stored for the slot.
#[must_use = "a waker or a reclaim handed back and ignored leaves a task waiting"]
#[derive(Debug)]
pub enum Settled {
    /// The outcome is recorded and [`Table::outcome`] reads it from now on.
    /// The `Interest::Outcome` waker stored for the call, if any, is handed
    /// back and is no longer stored.
    Recorded(Option<Waker>),
    /// The call had been forgotten while in flight: the outcome is not
    /// recorded, the slot is free, and its reservation is credited to the
    /// budget.
    Reclaimed,
    /// No call in flight has this correlation — never issued, already
    /// settled, or of a generation the slot no longer has. Nothing changed.
    Unknown,
}

/// What [`Table::forget`] did, and the waker it hands back to be woken.
///
/// Unless the answer is [`Unknown`](Forgotten::Unknown), the runtime then
/// takes the slot's stream wakers with [`Table::take_streams`] and wakes them
/// after releasing its lock: every stream of a forgotten call stops. On
/// `Unknown` it does not, because the slot of a stale correlation may hold
/// another call that is still live. On
/// [`Reclaimed`](Forgotten::Reclaimed) the runtime wakes every
/// `Interest::Slot` waiter it holds and frees whatever it stored for the slot.
#[must_use = "a waker or a reclaim handed back and ignored leaves a task waiting"]
#[derive(Debug)]
pub enum Forgotten {
    /// The call was settled: the slot is free now, and its reservation is
    /// credited to the budget.
    Reclaimed,
    /// The call is in flight: it is marked, its outcome will not be recorded,
    /// and the slot is reclaimed at its settlement, which [`Table::settle`]
    /// reports as [`Settled::Reclaimed`]. The call's `Interest::Outcome`
    /// waker, if any, is handed back, because no outcome will ever be readable
    /// for it.
    Marked(Option<Waker>),
    /// No call this table holds has this correlation, or it was already
    /// forgotten. Nothing changed.
    Unknown,
}

/// Where one slot's call is.
#[derive(Clone, Copy, Debug)]
enum State {
    Free,
    InFlight { forgotten: bool },
    Settled(Result<(), CallError>),
}

#[derive(Debug)]
struct Entry {
    generation: u64,
    state: State,
    /// The bytes debited from the budget at insert, credited at reclaim.
    reservation: u64,
    /// The call's one `Interest::Outcome` waker.
    waker: Option<Waker>,
    /// The call's `Interest::Stream` wakers, one for each stream number.
    streams: [Option<(StreamNo, Waker)>; STREAM_WAKERS],
}

/// An empty stream waker cell, as a constant so that an array of them can be
/// built in a `const` context.
const NO_STREAM: Option<(StreamNo, Waker)> = None;

impl Entry {
    const FREE: Entry = Entry {
        generation: 0,
        state: State::Free,
        reservation: 0,
        waker: None,
        streams: [NO_STREAM; STREAM_WAKERS],
    };
}

/// The caller-side call table: `N` slots, each with a generation, the call's
/// outcome status, one outcome waker and up to [`STREAM_WAKERS`] stream
/// wakers, and an optional byte budget.
///
/// Every method takes `&self` or `&mut self` and returns; a runtime keeps the
/// table behind its own lock. See the [module documentation](self).
#[derive(Debug)]
pub struct Table<const N: usize> {
    slots: [Entry; N],
    /// The bytes still free, or `None` for no budget.
    budget: Option<u64>,
}

impl<const N: usize> Table<N> {
    /// Fails the build of a `Table<N>` whose slot index does not fit in the 16
    /// bits a correlation keeps for it.
    const SLOT_INDEX_FITS: () = assert!(
        N <= 1 << SLOT_BITS,
        "a correlate::Table holds at most 65536 slots"
    );

    /// An empty table. `budget` is the byte budget reservations are debited
    /// from — a runtime sizes it from an interface's size table with
    /// [`table_budget`](crate::contract::table_budget) — or `None` for no
    /// budget, in which case only the slot count bounds the calls in flight.
    pub const fn new(budget: Option<u64>) -> Self {
        let () = Self::SLOT_INDEX_FITS;
        Table {
            slots: [Entry::FREE; N],
            budget,
        }
    }

    /// Takes the lowest free slot for a new call and debits `reservation`
    /// from the budget. `None` when every slot is in flight or settled and
    /// unforgotten, or when the budget has fewer than `reservation` bytes
    /// free; a runtime answers either with `SendError::Busy`. Without a
    /// budget, `reservation` is not read.
    pub fn insert(&mut self, reservation: u64) -> Option<Correlation> {
        let index = self
            .slots
            .iter()
            .position(|entry| matches!(entry.state, State::Free))?;
        if let Some(free) = self.budget {
            self.budget = Some(free.checked_sub(reservation)?);
        }
        let entry = &mut self.slots[index];
        entry.state = State::InFlight { forgotten: false };
        entry.reservation = reservation;
        // A runtime that did not take the stream wakers of the slot's
        // previous call leaves them here; the new call does not inherit them.
        entry.streams = [NO_STREAM; STREAM_WAKERS];
        Some(Correlation((entry.generation << SLOT_BITS) | index as u64))
    }

    /// Records the outcome of the call in flight under `c`, or, when the call
    /// was forgotten, reclaims its slot. The first settlement of a call is
    /// its outcome; a later one is [`Settled::Unknown`] and changes nothing.
    ///
    /// The slot's stream wakers are handed back through
    /// [`take_streams`](Table::take_streams), which the runtime calls next:
    /// a settlement ends a reply stream, and a parameter stream's producer
    /// reads the stream again to learn whether it goes on.
    pub fn settle(&mut self, c: Correlation, outcome: Result<(), CallError>) -> Settled {
        let Some(index) = self.held(c) else {
            return Settled::Unknown;
        };
        match self.slots[index].state {
            State::InFlight { forgotten: false } => {
                let entry = &mut self.slots[index];
                entry.state = State::Settled(outcome);
                Settled::Recorded(entry.waker.take())
            }
            State::InFlight { forgotten: true } => {
                self.reclaim(index);
                Settled::Reclaimed
            }
            State::Settled(_) | State::Free => Settled::Unknown,
        }
    }

    /// The outcome recorded for `c`, or `None` while it is in flight, after it
    /// is forgotten, or when the table holds no call under `c`. Reading it
    /// does not reclaim the slot: only [`forget`](Table::forget) does.
    #[must_use]
    pub fn outcome(&self, c: Correlation) -> Option<Result<(), CallError>> {
        match self.slots[self.held(c)?].state {
            State::Settled(outcome) => Some(outcome),
            State::InFlight { .. } | State::Free => None,
        }
    }

    /// Releases `c`, the one operation that reclaims a slot: at once for a
    /// settled call, and at its settlement for a call in flight, which this
    /// marks. Either way `c` has no readable outcome afterwards, and every
    /// stream of the call stops: the slot's stream wakers are handed back
    /// through [`take_streams`](Table::take_streams), which the runtime calls
    /// next, and a later [`wake_on_stream`](Table::wake_on_stream) under `c`
    /// stores nothing.
    pub fn forget(&mut self, c: Correlation) -> Forgotten {
        let Some(index) = self.held(c) else {
            return Forgotten::Unknown;
        };
        match self.slots[index].state {
            State::Settled(_) => {
                self.reclaim(index);
                Forgotten::Reclaimed
            }
            State::InFlight { forgotten: false } => {
                let entry = &mut self.slots[index];
                entry.state = State::InFlight { forgotten: true };
                Forgotten::Marked(entry.waker.take())
            }
            State::InFlight { forgotten: true } | State::Free => Forgotten::Unknown,
        }
    }

    /// Stores `waker` as the `Interest::Outcome(c)` waker of the call in
    /// flight under `c`, and returns the waker to wake:
    ///
    /// - the displaced waker, when the call held a waker of another task;
    /// - nothing, when the stored waker [`will_wake`](Waker::will_wake) the
    ///   same task as `waker`, which is a refresh: the stored waker is
    ///   replaced and not woken (ADR-0021 decision 13);
    /// - `waker` itself, not stored, when no outcome is still to be recorded
    ///   under `c`: the outcome is already known, the call was forgotten, or
    ///   the table holds no call under `c`. A stored waker would never be
    ///   woken, and the task's read that follows finds what there is.
    pub fn wake_on(&mut self, c: Correlation, waker: &Waker) -> Option<Waker> {
        let Some(index) = self.held(c) else {
            return Some(waker.clone());
        };
        let entry = &mut self.slots[index];
        match entry.state {
            State::InFlight { forgotten: false } => store(&mut entry.waker, waker),
            State::InFlight { forgotten: true } | State::Settled(_) | State::Free => {
                Some(waker.clone())
            }
        }
    }

    /// Stores `waker` as the `Interest::Stream(c, s)` waker of the call under
    /// `c`, one for each stream number, and returns the waker to wake, under
    /// the rule of [`wake_on`](Table::wake_on):
    ///
    /// - the displaced waker, when stream `s` held a waker of another task;
    /// - nothing, when the stored waker of stream `s`
    ///   [`will_wake`](Waker::will_wake) the same task as `waker`, which is a
    ///   refresh;
    /// - `waker` itself, not stored, when the call was forgotten or the table
    ///   holds no call under `c`, since no stream of `c` will change again,
    ///   and when the slot already holds [`STREAM_WAKERS`] wakers of other
    ///   streams.
    ///
    /// A settled call still stores the waker: a command is settled at its
    /// acknowledgment, and its parameter streams go on after it.
    pub fn wake_on_stream(&mut self, c: Correlation, s: StreamNo, waker: &Waker) -> Option<Waker> {
        let Some(index) = self.held(c) else {
            return Some(waker.clone());
        };
        let entry = &mut self.slots[index];
        match entry.state {
            State::InFlight { forgotten: false } | State::Settled(_) => {}
            State::InFlight { forgotten: true } | State::Free => return Some(waker.clone()),
        }
        if let Some((_, stored)) = entry.streams.iter_mut().flatten().find(|(no, _)| *no == s) {
            return Some(core::mem::replace(stored, waker.clone()))
                .filter(|displaced| !displaced.will_wake(waker));
        }
        match entry.streams.iter_mut().find(|cell| cell.is_none()) {
            Some(cell) => {
                *cell = Some((s, waker.clone()));
                None
            }
            None => Some(waker.clone()),
        }
    }

    /// Takes the `Interest::Stream(c, s)` waker of the call under `c`, which
    /// a runtime wakes when a chunk or the end arrives on stream `s` or its
    /// window frees. `None` when none is stored or the table holds no call
    /// under `c`.
    pub fn take_stream(&mut self, c: Correlation, s: StreamNo) -> Option<Waker> {
        let index = self.held(c)?;
        let cell = self.slots[index]
            .streams
            .iter_mut()
            .find(|cell| matches!(cell, Some((no, _)) if *no == s))?;
        cell.take().map(|(_, waker)| waker)
    }

    /// Takes every stream waker slot `slot` holds, whatever stream it was
    /// registered on. A runtime calls it after every [`settle`](Table::settle)
    /// and [`forget`](Table::forget) that did not answer `Unknown`, with the
    /// slot of the correlation it passed, [`Table::slot`]`(c)`: it works after
    /// a reclaim too, when `c` already answers as unknown. A runtime that does
    /// not call it after a settlement or a forget leaves the stream waiters of
    /// that call asleep; the stream tests of `ridl-rt-conformance` check that
    /// they are woken. The wakers are taken when this is called,
    /// whether or not the iterator is consumed, and the iterator does not
    /// borrow the table. A slot index the table does not have yields nothing.
    pub fn take_streams(&mut self, slot: usize) -> impl Iterator<Item = Waker> + use<N> {
        let taken = match self.slots.get_mut(slot) {
            Some(entry) => core::mem::replace(&mut entry.streams, [NO_STREAM; STREAM_WAKERS]),
            None => [NO_STREAM; STREAM_WAKERS],
        };
        taken.into_iter().flatten().map(|(_, waker)| waker)
    }

    /// The slot index of `c`, which a runtime uses to index its own storage
    /// for the call — reply bytes, arguments — beside the table.
    #[must_use]
    pub fn slot(c: Correlation) -> usize {
        (c.0 & SLOT_MASK) as usize
    }

    /// The slot of `c` when that slot holds a call under `c`'s generation.
    fn held(&self, c: Correlation) -> Option<usize> {
        let index = Self::slot(c);
        let entry = self.slots.get(index)?;
        let generation = c.0 >> SLOT_BITS;
        (entry.generation == generation && !matches!(entry.state, State::Free)).then_some(index)
    }

    /// Frees a slot: credits its reservation, drops its outcome waker, and
    /// advances its generation so the correlation it had answers as unknown.
    /// The stream wakers stay for [`take_streams`](Table::take_streams) to
    /// hand back; the slot's next [`insert`](Table::insert) drops any left.
    fn reclaim(&mut self, index: usize) {
        let entry = &mut self.slots[index];
        if let Some(free) = self.budget {
            self.budget = Some(free.saturating_add(entry.reservation));
        }
        entry.state = State::Free;
        entry.reservation = 0;
        entry.waker = None;
        entry.generation = entry.generation.wrapping_add(1) & GENERATION_MASK;
    }
}

/// The waiter registry a handle keeps behind
/// [`Wakeable`](crate::port::Wakeable): one waker for each of the kinds
/// `Slot`, `Event` and `Claim`.
///
/// `Interest::Stream` and `Interest::ClaimStream` are not stored here either:
/// a `Stream` waker is kept with its call, in [`Table`], and a `ClaimStream`
/// waker with its claim, in the providing runtime's own claim storage.
///
/// An `Event` or `Claim` key's interface is not kept: the handle holds one
/// waker per kind, and a change to any key of that kind takes it (ADR-0021
/// decision 13). `Interest::Outcome` is not stored here; a call's outcome
/// waker is kept with the call, in [`Table`]. Whether a key's change has
/// already happened, and so whether a registration is woken at once, is the
/// runtime's to know: it registers, and then takes the waker back when the
/// change is already visible.
#[derive(Debug, Default)]
pub struct Waiters {
    slot: Option<Waker>,
    event: Option<Waker>,
    claim: Option<Waker>,
}

impl Waiters {
    /// A registry with no waker stored.
    #[must_use]
    pub const fn new() -> Self {
        Waiters {
            slot: None,
            event: None,
            claim: None,
        }
    }

    /// Stores `waker` as the one waker of `what`'s kind, and returns the
    /// waker to wake:
    ///
    /// - the displaced waker, when the kind held a waker of another task;
    /// - nothing, when the stored waker [`will_wake`](Waker::will_wake) the
    ///   same task as `waker`, which is a refresh: the stored waker is
    ///   replaced and not woken, whatever interface either key named;
    /// - `waker` itself, not stored, for `Interest::Outcome`,
    ///   `Interest::Stream` and `Interest::ClaimStream`, which are kept with
    ///   their call or their claim: a registration here could never be taken,
    ///   and handing it back leaves no task waiting on it.
    pub fn register(&mut self, what: Interest, waker: &Waker) -> Option<Waker> {
        match self.kind(what) {
            Some(stored) => store(stored, waker),
            None => Some(waker.clone()),
        }
    }

    /// Takes the waker of `what`'s kind, whatever interface it was registered
    /// under, which clears that kind. `None` when none is stored, and always
    /// for `Interest::Outcome`, `Interest::Stream` and `Interest::ClaimStream`.
    pub fn take(&mut self, what: Interest) -> Option<Waker> {
        self.kind(what)?.take()
    }

    /// Takes every stored waker, which clears every kind, for a runtime with
    /// one unkeyed "something changed" source. The wakers are taken when this
    /// is called, whether or not the iterator is consumed, and the iterator
    /// does not borrow the registry.
    pub fn take_all(&mut self) -> impl Iterator<Item = Waker> + use<> {
        [self.slot.take(), self.event.take(), self.claim.take()]
            .into_iter()
            .flatten()
    }

    fn kind(&mut self, what: Interest) -> Option<&mut Option<Waker>> {
        match what {
            Interest::Slot => Some(&mut self.slot),
            Interest::Event(_) => Some(&mut self.event),
            Interest::Claim(_) => Some(&mut self.claim),
            Interest::Outcome(_) | Interest::Stream(..) | Interest::ClaimStream(..) => None,
        }
    }
}

/// Stores `waker` in a one-waker slot and returns the displaced waker of
/// another task, or nothing on a refresh by the same task.
fn store(stored: &mut Option<Waker>, waker: &Waker) -> Option<Waker> {
    stored
        .replace(waker.clone())
        .filter(|displaced| !displaced.will_wake(waker))
}
