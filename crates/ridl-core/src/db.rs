//! The salsa incremental database for the RIDL family (docs/ROADMAP.md epic
//! E0.4, ADR-0004 §3).
//!
//! This crate is where "the compiler is a library first" becomes concrete: an
//! [`InputFile`] input, a memoized [`parse_file`] query over
//! [`ridl_syntax::parse`], and the concrete [`RidlDatabase`] that `ridlc` and
//! the language server run on. Salsa memoizes each parse and backdates on the
//! green-tree equality of [`ridl_syntax::Parse`], so editing one file's text
//! re-runs the parse for that file alone.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ridl_syntax::{Parse, parse};

/// A source file the compiler tracks: its path and its text.
///
/// Editing the text through the generated `set_text` setter starts a new salsa
/// revision, which is what drives incremental reparsing.
#[salsa::input]
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
/// A salsa event callback counts tracked-query executions so the spike can
/// prove that an edit reparses only the edited file. The counter is read and
/// reset with [`RidlDatabase::take_execution_count`].
#[salsa::db]
#[derive(Clone)]
pub struct RidlDatabase {
    storage: salsa::Storage<Self>,
    executions: Arc<AtomicUsize>,
}

impl RidlDatabase {
    /// Returns the number of tracked-query executions since the previous call
    /// and resets the counter to zero.
    pub fn take_execution_count(&self) -> usize {
        self.executions.swap(0, Ordering::SeqCst)
    }
}

impl Default for RidlDatabase {
    fn default() -> Self {
        // A manual `Default` (rather than a derive) is required to install the
        // event callback at `Storage` construction; the callback increments the
        // execution counter on every query execution.
        let executions = Arc::new(AtomicUsize::new(0));
        let storage = salsa::Storage::new(Some(Box::new({
            let executions = executions.clone();
            move |event| {
                if let salsa::EventKind::WillExecute { .. } = event.kind {
                    executions.fetch_add(1, Ordering::SeqCst);
                }
            }
        })));
        Self {
            storage,
            executions,
        }
    }
}

#[salsa::db]
impl salsa::Database for RidlDatabase {}

#[cfg(test)]
mod tests {
    use super::{InputFile, RidlDatabase, parse_file};
    use salsa::Setter;

    /// The salsa spike's proof: editing one file's text re-parses only that
    /// file, and the other file's parse stays memoized (docs/ROADMAP.md epic
    /// E0.4). Execution counting rides a salsa event callback on the database.
    #[test]
    fn edit_reparses_only_the_edited_file() {
        let mut db = RidlDatabase::default();

        let a = InputFile::new(&db, "a.typl".to_string(), "type A: m".to_string());
        let b = InputFile::new(&db, "b.typl".to_string(), "type B: s".to_string());

        // Initial parse of both inputs runs the query twice.
        let parse_a = parse_file(&db, a);
        let parse_b = parse_file(&db, b);
        assert_eq!(
            db.take_execution_count(),
            2,
            "the first parse of A and B must run the query exactly twice",
        );
        assert_eq!(parse_a.syntax().text().to_string(), "type A: m");
        assert_eq!(parse_b.syntax().text().to_string(), "type B: s");

        // Re-querying unchanged inputs is a pure memo hit: no executions.
        let _ = parse_file(&db, a);
        let _ = parse_file(&db, b);
        assert_eq!(
            db.take_execution_count(),
            0,
            "re-querying unchanged inputs must run no executions",
        );

        // Edit A's text only.
        a.set_text(&mut db).to("type A: kg".to_string());

        let parse_a2 = parse_file(&db, a);
        let parse_b2 = parse_file(&db, b);
        assert_eq!(
            db.take_execution_count(),
            1,
            "editing A must re-parse exactly one file",
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
