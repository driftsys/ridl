//! The valid (kind, family, shape) combinations of an interaction (ADR-0012
//! decision 4, as amended on 2026-10-11). The open cells of the decision's
//! table count as valid, so a later spelling for one needs no enum value.
//!
//! The IR readers call [`check_package`] on decode and the writers call it
//! before they encode, so an invalid combination such as
//! `(command, intent, none)` is refused where the IR is read, not by a lint a
//! caller can forget. The codegen readers and writers do the same with
//! [`check_model`].

use crate::codegen::v1;
use crate::v2;

/// The four kinds that carry a family. `fixed` carries none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Signal,
    Event,
    Command,
    Query,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Self::Signal => "signal",
            Self::Event => "event",
            Self::Command => "command",
            Self::Query => "query",
        }
    }
}

/// A (kind, family, shape) combination the table does not hold, or a family
/// or shape number outside its enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionClassError {
    pub kind: Kind,
    pub family: i32,
    pub shape: i32,
}

impl std::fmt::Display for InteractionClassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let family = v2::Family::try_from(self.family).map_or_else(
            |_| format!("family {}", self.family),
            |family| family.as_str_name().to_string(),
        );
        let shape = v2::Shape::try_from(self.shape).map_or_else(
            |_| format!("shape {}", self.shape),
            |shape| shape.as_str_name().to_string(),
        );
        write!(
            f,
            "invalid interaction class: a {} cannot be ({family}, {shape})",
            self.kind.word()
        )
    }
}

impl std::error::Error for InteractionClassError {}

/// Whether `(kind, family, shape)` is a cell of the table.
pub fn check_interaction_class(
    kind: Kind,
    family: i32,
    shape: i32,
) -> Result<(), InteractionClassError> {
    use v2::{Family, Shape};
    let error = || InteractionClassError {
        kind,
        family,
        shape,
    };
    let family_value = Family::try_from(family).map_err(|_| error())?;
    let shape_value = Shape::try_from(shape).map_err(|_| error())?;
    // Written as a `match` rather than as `matches!`, because each arm is one
    // row of the ADR-0012 decision 4 table and reads as that table.
    #[allow(clippy::match_like_matches_macro)]
    let valid = match (kind, family_value, shape_value) {
        (
            Kind::Signal,
            Family::Dispatch | Family::Presentation | Family::Acquisition | Family::Control,
            Shape::None,
        ) => true,
        (
            Kind::Event,
            Family::Dispatch | Family::Presentation | Family::Intent | Family::Acquisition,
            Shape::None,
        ) => true,
        (Kind::Command, Family::Dispatch | Family::Control, Shape::None) => true,
        (
            Kind::Command,
            Family::Intent,
            Shape::Activate | Shape::Toggle | Shape::Select | Shape::Adjust | Shape::Dismiss,
        ) => true,
        (Kind::Query, Family::Dispatch | Family::Acquisition, Shape::None) => true,
        _ => false,
    };
    if valid { Ok(()) } else { Err(error()) }
}

/// Every interaction of every interface shape of `package`.
pub fn check_package(package: &v2::Package) -> Result<(), InteractionClassError> {
    for shape in package.shapes() {
        for decl in &shape.interface.interactions {
            check_decl(decl)?;
        }
    }
    Ok(())
}

fn check_decl(decl: &v2::Decl) -> Result<(), InteractionClassError> {
    use v2::decl::Kind as DeclKind;
    match decl.kind.as_ref() {
        Some(DeclKind::SignalDef(signal)) => {
            check_interaction_class(Kind::Signal, signal.family, 0)
        }
        Some(DeclKind::EventDef(event)) => check_interaction_class(Kind::Event, event.family, 0),
        Some(DeclKind::CommandDef(command)) => {
            check_interaction_class(Kind::Command, command.family, command.shape)
        }
        Some(DeclKind::QueryDef(query)) => check_interaction_class(Kind::Query, query.family, 0),
        _ => Ok(()),
    }
}

/// Every interaction of every interface of `model`.
pub fn check_model(model: &v1::Model) -> Result<(), InteractionClassError> {
    for interface in &model.interfaces {
        for slot in &interface.slots {
            let Some(v1::interaction_slot::Occupant::Interaction(interaction)) =
                slot.occupant.as_ref()
            else {
                continue;
            };
            let shape = match &interaction.shape {
                Some(v1::interaction::Shape::Command(command)) => command.shape,
                _ => 0,
            };
            let kind = match v1::Kind::try_from(interaction.kind) {
                Ok(v1::Kind::Signal) => Kind::Signal,
                Ok(v1::Kind::Event) => Kind::Event,
                Ok(v1::Kind::Command) => Kind::Command,
                Ok(v1::Kind::Query) => Kind::Query,
                _ => continue,
            };
            check_interaction_class(kind, interaction.family, shape)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2::{Family, Shape};

    fn ok(kind: Kind, family: Family, shape: Shape) -> bool {
        check_interaction_class(kind, family as i32, shape as i32).is_ok()
    }

    #[test]
    fn the_table_of_valid_combinations() {
        // ADR-0012 decision 4, the open cells counted as valid.
        for family in [
            Family::Dispatch,
            Family::Presentation,
            Family::Acquisition,
            Family::Control,
        ] {
            assert!(ok(Kind::Signal, family, Shape::None), "signal {family:?}");
        }
        assert!(!ok(Kind::Signal, Family::Intent, Shape::None));
        for family in [
            Family::Dispatch,
            Family::Presentation,
            Family::Intent,
            Family::Acquisition,
        ] {
            assert!(ok(Kind::Event, family, Shape::None), "event {family:?}");
        }
        assert!(!ok(Kind::Event, Family::Control, Shape::None));
        assert!(ok(Kind::Command, Family::Dispatch, Shape::None));
        assert!(ok(Kind::Command, Family::Control, Shape::None));
        for shape in [
            Shape::Activate,
            Shape::Toggle,
            Shape::Select,
            Shape::Adjust,
            Shape::Dismiss,
        ] {
            assert!(
                ok(Kind::Command, Family::Intent, shape),
                "command intent {shape:?}"
            );
        }
        assert!(
            !ok(Kind::Command, Family::Intent, Shape::None),
            "decision 5: no generic intent"
        );
        assert!(
            !ok(Kind::Command, Family::Dispatch, Shape::Toggle),
            "a shape needs intent"
        );
        assert!(!ok(Kind::Command, Family::Presentation, Shape::None));
        assert!(!ok(Kind::Command, Family::Acquisition, Shape::None));
        for family in [Family::Dispatch, Family::Acquisition] {
            assert!(ok(Kind::Query, family, Shape::None), "query {family:?}");
        }
        assert!(!ok(Kind::Query, Family::Intent, Shape::None));
        assert!(!ok(Kind::Query, Family::Control, Shape::None));
    }

    #[test]
    fn a_number_outside_the_enums_is_invalid() {
        assert!(check_interaction_class(Kind::Signal, 99, 0).is_err());
        assert!(check_interaction_class(Kind::Command, 2, 99).is_err());
    }

    #[test]
    fn the_error_names_the_combination() {
        let err = InteractionClassError {
            kind: Kind::Command,
            family: 2,
            shape: 0,
        };
        assert_eq!(
            err.to_string(),
            "invalid interaction class: a command cannot be (FAMILY_INTENT, SHAPE_NONE)"
        );
    }
}
