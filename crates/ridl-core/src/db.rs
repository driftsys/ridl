//! The salsa incremental database for the RIDL family (docs/ROADMAP.md epic
//! E0.4, ADR-0004 §3).
//!
//! This crate is where "the compiler is a library first" becomes concrete: an
//! [`InputFile`] input, a memoized [`parse_file`] query over
//! [`ridl_syntax::parse`], and the concrete [`RidlDatabase`] that `ridlc` and
//! the language server run on. Salsa memoizes each parse and backdates on the
//! green-tree equality of [`ridl_syntax::Parse`], so editing one file's text
//! re-runs the parse for that file alone.

use std::sync::{Arc, Mutex, OnceLock};

use ridl_syntax::{Parse, parse};

use crate::package::Package;

/// A source file the compiler tracks: its path and its text.
///
/// Editing the text through the generated `set_text` setter starts a new salsa
/// revision, which is what drives incremental reparsing.
#[salsa::input(debug)]
pub struct InputFile {
    pub path: String,
    #[returns(ref)]
    pub text: String,
}

/// Parses one [`InputFile`] into a lossless [`Parse`].
///
/// Salsa memoizes the result and backdates on [`Parse`] equality (green-tree
/// identity), so the body re-runs only when the file's `text` changes.
#[salsa::tracked(returns(clone))]
pub fn parse_file(db: &dyn salsa::Database, file: InputFile) -> Parse {
    parse(file.text(db))
}

/// The concrete salsa database `ridlc` and the language server run on.
///
/// A salsa event callback records the `database_key` of every tracked-query
/// execution, so a test can assert not only how many queries re-ran after an
/// edit but exactly which ones (issue #102). The log is read and reset with
/// [`RidlDatabase::take_executed_queries`]. The database deliberately does not
/// implement `Clone`: a clone sharing the execution log would make the
/// observability counters ambiguous (issue #102).
#[salsa::db]
pub struct RidlDatabase {
    storage: salsa::Storage<Self>,
    executed: Arc<Mutex<Vec<salsa::DatabaseKeyIndex>>>,
    /// The memoized built-in `ridl.std` package
    /// ([`std_package`](crate::std_lib::std_package)), created at most once
    /// per database.
    pub(crate) std_package_cache: OnceLock<Package>,
}

impl RidlDatabase {
    /// Returns the `database_key` of every tracked-query execution since the
    /// previous call, in execution order, and clears the log.
    pub fn take_executed_queries(&self) -> Vec<salsa::DatabaseKeyIndex> {
        std::mem::take(
            &mut *self
                .executed
                .lock()
                .expect("the execution log mutex is never poisoned"),
        )
    }
}

impl Default for RidlDatabase {
    fn default() -> Self {
        // A manual `Default` (rather than a derive) is required to install the
        // event callback at `Storage` construction; the callback records the
        // executed query's key on every query execution.
        let executed: Arc<Mutex<Vec<salsa::DatabaseKeyIndex>>> = Arc::default();
        let storage = salsa::Storage::new(Some(Box::new({
            let executed = executed.clone();
            move |event| {
                if let salsa::EventKind::WillExecute { database_key } = event.kind {
                    executed
                        .lock()
                        .expect("the execution log mutex is never poisoned")
                        .push(database_key);
                }
            }
        })));
        Self {
            storage,
            executed,
            std_package_cache: OnceLock::new(),
        }
    }
}

#[salsa::db]
impl salsa::Database for RidlDatabase {}

#[cfg(test)]
mod tests {
    use super::{InputFile, RidlDatabase, parse_file};
    use salsa::Setter;
    use salsa::plumbing::AsId;

    /// Renders a `database_key` the way salsa's ingredient does with the
    /// database attached, e.g. `parse_file(Id(0))`.
    fn rendered(db: &RidlDatabase, key: salsa::DatabaseKeyIndex) -> String {
        salsa::attach(db, || format!("{key:?}"))
    }

    /// The salsa spike's proof: editing one file's text re-parses only that
    /// file, and the other file's parse stays memoized (docs/ROADMAP.md epic
    /// E0.4). The event callback records each executed query's `database_key`,
    /// so the test asserts *which* query re-ran, not just how many (issue
    /// #102).
    #[test]
    fn edit_reparses_only_the_edited_file() {
        let mut db = RidlDatabase::default();

        let a = InputFile::new(&db, "a.typl".to_string(), "type A: m".to_string());
        let b = InputFile::new(&db, "b.typl".to_string(), "type B: s".to_string());

        // Initial parse of both inputs runs the query twice.
        let parse_a = parse_file(&db, a);
        let parse_b = parse_file(&db, b);
        let executed = db.take_executed_queries();
        assert_eq!(
            executed.len(),
            2,
            "the first parse of A and B must run the query exactly twice",
        );
        assert_eq!(parse_a.syntax().text().to_string(), "type A: m");
        assert_eq!(parse_b.syntax().text().to_string(), "type B: s");

        // Re-querying unchanged inputs is a pure memo hit: no executions.
        let _ = parse_file(&db, a);
        let _ = parse_file(&db, b);
        assert_eq!(
            db.take_executed_queries(),
            Vec::new(),
            "re-querying unchanged inputs must run no executions",
        );

        // Edit A's text only.
        a.set_text(&mut db).to("type A: kg".to_string());

        let parse_a2 = parse_file(&db, a);
        let parse_b2 = parse_file(&db, b);
        let executed = db.take_executed_queries();
        assert_eq!(
            executed.len(),
            1,
            "editing A must re-parse exactly one file",
        );
        assert_eq!(
            rendered(&db, executed[0]),
            format!("parse_file({:?})", a.as_id()),
            "the re-executed query must be the parse of A, keyed by A's input",
        );
        assert_eq!(
            parse_a2.syntax().text().to_string(),
            "type A: kg",
            "A's memoized parse must reflect the edited text",
        );
        assert_ne!(
            parse_a2, parse_a,
            "A's parse value must change after the edit"
        );
        assert_eq!(
            parse_b2, parse_b,
            "B's parse value must stay memoized and unchanged",
        );
    }
}
