//! The attribute registry (ADR-0012 decision 8, as amended on 2026-10-11): one
//! row per attribute key any profile accepts. The ridl checker and the rsdl
//! attribute reader look a key up here and nowhere else; `ridl diff` reports a
//! change to every `Compared` row and records the reason for every row it does
//! not report.
//!
//! A row names its machine consumer, because an attribute nobody consumes is
//! not an attribute (general form §4.1). The `keywords` freeze dump holds one
//! line per (key, site, form) of this table, so removing a key or a site fails
//! `cargo xtask freeze check`.

/// The key of a row: a plain word, or the `namespace.key` pattern of the
/// backend keys (rsdl reference §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrName {
    Key(&'static str),
    Namespaced,
}

/// Who defines the key: the family, or a backend that claims the namespace
/// through `[backend.<name>]` in the manifest. A `user:` owner waits for a
/// consumer (ADR-0012 decision 8, as amended on 2026-10-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Core,
    Backend,
}

/// The syntactic form of the key (general form §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Flag,
    Assignment,
    Predicate,
    /// A backend key is a flag or an assignment, as the backend decides.
    FlagOrAssignment,
}

impl Form {
    /// The word the `keywords` dump prints.
    pub fn word(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Assignment => "assignment",
            Self::Predicate => "predicate",
            Self::FlagOrAssignment => "flag-or-assignment",
        }
    }
}

/// A declaration kind or body line that takes an attribute block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Site {
    Command,
    Query,
    System,
    Component,
    Distribution,
    Deployment,
    Machine,
    /// A member line of a `machine` (rsdl reference §5).
    Placement,
    /// Any other body line: a system or distribution member line, an
    /// `offers` or a `requires` line.
    Line,
}

impl Site {
    /// The word the `keywords` dump prints.
    pub fn word(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Query => "query",
            Self::System => "system",
            Self::Component => "component",
            Self::Distribution => "distribution",
            Self::Deployment => "deployment",
            Self::Machine => "machine",
            Self::Placement => "placement",
            Self::Line => "line",
        }
    }
}

/// Why `ridl diff` does not report a change to a key. The set is closed: a
/// new reason is a design decision, not an edit here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotReported {
    /// A sizing value changes no link and no contract (rsdl reference §14).
    Sizing,
}

/// What `ridl diff` does with a change to the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diff {
    /// Reported; the text names the category or the heading it is listed
    /// under.
    Compared(&'static str),
    NotReported(NotReported),
}

/// One row of the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrKey {
    pub name: AttrName,
    pub owner: Owner,
    pub form: Form,
    /// The sites that accept the key. A key on any other site is FORM-107.
    pub sites: &'static [Site],
    /// The machine consumer (general form §4.1).
    pub consumer: &'static str,
    pub diff: Diff,
}

impl AttrKey {
    /// The key as written, `ns.key` for the backend pattern.
    pub fn key(&self) -> &'static str {
        match self.name {
            AttrName::Key(key) => key,
            AttrName::Namespaced => "ns.key",
        }
    }

    /// Whether `site` accepts the key.
    pub fn allows(&self, site: Site) -> bool {
        self.sites.contains(&site)
    }
}

const RSDL_DECLARATIONS: &[Site] = &[
    Site::System,
    Site::Component,
    Site::Distribution,
    Site::Deployment,
    Site::Machine,
];

const SIZING_SITES: &[Site] = &[Site::Deployment, Site::Placement];

/// The registry. Every key any profile accepts is a row here. The sites
/// restate what the ridl checker and the rsdl attribute reader accepted
/// before this table existed; the design that added it changed none of them.
pub const ATTRIBUTES: &[AttrKey] = &[
    AttrKey {
        name: AttrName::Key("require"),
        owner: Owner::Core,
        form: Form::Predicate,
        sites: &[Site::Command, Site::Query],
        consumer: "the precondition check (ridl reference §13): generated clause code, \
                   the test plane and the online observer",
        diff: Diff::Compared("the interface category `contract_changed`"),
    },
    AttrKey {
        name: AttrName::Key("ensure"),
        owner: Owner::Core,
        form: Form::Predicate,
        sites: &[Site::Query],
        consumer: "the postcondition check (ridl reference §13): generated clause code, \
                   the test plane and the online observer",
        diff: Diff::Compared("the interface category `contract_changed`"),
    },
    AttrKey {
        name: AttrName::Key("instances"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: &[Site::Component],
        consumer: "the instance set of the component (rsdl reference §7)",
        diff: Diff::Compared("`composition changed`"),
    },
    AttrKey {
        name: AttrName::Key("external"),
        owner: Owner::Core,
        form: Form::Flag,
        sites: &[Site::Component, Site::Machine],
        consumer: "placement and the surface set (rsdl reference §9, §13)",
        diff: Diff::Compared(
            "`composition changed` on a component, `placement changed` on a machine",
        ),
    },
    AttrKey {
        name: AttrName::Key("tier"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: &[Site::Distribution],
        consumer: "RSDL-901, the tier inversion check (rsdl reference §3.3)",
        diff: Diff::Compared("`composition changed`"),
    },
    AttrKey {
        name: AttrName::Key("labels"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: RSDL_DECLARATIONS,
        consumer: "assurance profiles (typl reference §14.3, general form §4.7)",
        diff: Diff::Compared(
            "`composition changed` on a system, a component or a distribution, \
             `placement changed` on a deployment or a machine",
        ),
    },
    AttrKey {
        name: AttrName::Key("deprecated"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: RSDL_DECLARATIONS,
        consumer: "the family lint (general form §4.7)",
        diff: Diff::Compared(
            "`composition changed` on a system, a component or a distribution, \
             `placement changed` on a deployment or a machine",
        ),
    },
    AttrKey {
        name: AttrName::Key("depth"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: SIZING_SITES,
        consumer: "the ring depth of an event channel (rsdl reference §13)",
        diff: Diff::NotReported(NotReported::Sizing),
    },
    AttrKey {
        name: AttrName::Key("slots"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: SIZING_SITES,
        consumer: "the slot count of a command or query channel (rsdl reference §13)",
        diff: Diff::NotReported(NotReported::Sizing),
    },
    AttrKey {
        name: AttrName::Key("budget"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: SIZING_SITES,
        consumer: "the byte budget of a command or query channel (rsdl reference §13)",
        diff: Diff::NotReported(NotReported::Sizing),
    },
    AttrKey {
        name: AttrName::Namespaced,
        owner: Owner::Backend,
        form: Form::FlagOrAssignment,
        sites: &[
            Site::System,
            Site::Component,
            Site::Distribution,
            Site::Deployment,
            Site::Machine,
            Site::Placement,
            Site::Line,
        ],
        consumer: "the backend that claims the namespace, uninterpreted (rsdl reference §5)",
        diff: Diff::Compared("`composition changed` or `placement changed`, by the site"),
    },
];

/// The row of a plain key, or `None` for a key no row names. The backend
/// pattern is never returned: a dotted key is read through
/// [`backend_pattern`].
pub fn lookup(key: &str) -> Option<&'static AttrKey> {
    ATTRIBUTES
        .iter()
        .find(|row| matches!(row.name, AttrName::Key(name) if name == key))
}

/// The `namespace.key` row.
pub fn backend_pattern() -> &'static AttrKey {
    ATTRIBUTES
        .iter()
        .find(|row| row.name == AttrName::Namespaced)
        .expect("the registry holds the backend pattern row")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_rows_share_a_name() {
        for (i, row) in ATTRIBUTES.iter().enumerate() {
            assert!(
                !ATTRIBUTES[..i]
                    .iter()
                    .any(|earlier| earlier.name == row.name),
                "duplicate row `{}`",
                row.key()
            );
        }
    }

    #[test]
    fn every_row_has_a_consumer_and_a_site() {
        for row in ATTRIBUTES {
            assert!(
                !row.consumer.is_empty(),
                "`{}` names no consumer",
                row.key()
            );
            assert!(!row.sites.is_empty(), "`{}` has no site", row.key());
        }
    }

    #[test]
    fn the_registry_holds_every_key_the_profiles_accept() {
        // The ridl §13 predicates, the rsdl §5 keys, and the backend pattern.
        for key in [
            "require",
            "ensure",
            "instances",
            "external",
            "tier",
            "labels",
            "deprecated",
            "depth",
            "slots",
            "budget",
        ] {
            assert!(lookup(key).is_some(), "`{key}` has no row");
        }
        assert_eq!(backend_pattern().owner, Owner::Backend);
        assert_eq!(
            ATTRIBUTES.len(),
            11,
            "one row per key, plus the pattern row"
        );
    }

    #[test]
    fn the_sites_restate_the_checkers_allow_lists() {
        let allows = |key: &str, site: Site| lookup(key).unwrap().allows(site);
        assert!(allows("require", Site::Command) && allows("require", Site::Query));
        assert!(allows("ensure", Site::Query) && !allows("ensure", Site::Command));
        assert!(allows("instances", Site::Component) && !allows("instances", Site::System));
        assert!(allows("external", Site::Component) && allows("external", Site::Machine));
        assert!(allows("tier", Site::Distribution) && !allows("tier", Site::Component));
        for key in ["labels", "deprecated"] {
            for site in RSDL_DECLARATIONS {
                assert!(allows(key, *site), "`{key}` on {site:?}");
            }
            assert!(!allows(key, Site::Placement) && !allows(key, Site::Line));
            assert!(
                !allows(key, Site::Command),
                "`{key}` is not a ridl member key"
            );
        }
        for key in ["depth", "slots", "budget"] {
            assert!(allows(key, Site::Deployment) && allows(key, Site::Placement));
            assert!(!allows(key, Site::Component));
            assert_eq!(
                lookup(key).unwrap().diff,
                Diff::NotReported(NotReported::Sizing)
            );
        }
        for site in RSDL_DECLARATIONS
            .iter()
            .chain(SIZING_SITES)
            .chain(&[Site::Line])
        {
            assert!(backend_pattern().allows(*site), "backend key on {site:?}");
        }
        assert!(!backend_pattern().allows(Site::Command));
    }

    #[test]
    fn a_dotted_key_is_not_looked_up_as_a_plain_key() {
        assert!(lookup("ns.key").is_none());
        assert!(lookup("linux.cpuset").is_none());
    }
}
