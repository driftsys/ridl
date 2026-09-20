//! The projection facts a target needs, derived from the IR and owned by no
//! backend.
//!
//! [`crate::name`] is the precedent and the reason: the pinned name transform
//! lives in `ridl-ir` because it is a projection — a pure function from IR
//! identity to a target's shape — and because `ridl-ir` is the crate every
//! consumer of a projection already depends on. The same holds for the facts
//! below. `ridl-backend-flatbuffers` emits the `.fbs` schema from them,
//! `ridl-backend-rust` emits the payload codec from them (roadmap story
//! E11.7), and `ridl-descriptor` will read the same size bound for the catalog
//! descriptor (Epic 16). None of those three may depend on either of the other
//! two, so the facts cannot live in a backend.
//!
//! One submodule per target. [`flatbuffers`] reads ADR-0019's projection rules
//! once, for both of the emitters that have to agree on them.

pub mod flatbuffers;
