//! The lint table of the book page `docs/book/lints.md` against the
//! diagnostic catalogue (lint foundation spec §9). A lint added to the
//! catalogue without a row in the book, or a row whose code, default level or
//! summary differs from its catalogue row, fails this test.

use std::collections::BTreeSet;
use std::path::Path;

use ridl_core::diag::ALL_CATALOGS;
use ridl_core::lint::default_level;

/// The header row of the one table the test reads.
const HEADER: &str = "| Lint | Code | Default | Summary |";

/// One row: lint name, code, default level, summary.
type Row = (String, String, String, String);

/// Splits a Markdown table row into its trimmed cells.
fn cells(line: &str) -> Vec<String> {
    let inner = line
        .trim()
        .strip_prefix('|')
        .and_then(|rest| rest.strip_suffix('|'))
        .unwrap_or_else(|| panic!("a table row starts and ends with `|`: {line}"));
    inner
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// The rows of the table under [`HEADER`] in `page`. Panics when the page has
/// no such table or more than one.
fn book_rows(page: &str) -> BTreeSet<Row> {
    let lines: Vec<&str> = page.lines().collect();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| cells_match_header(line))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "lints.md holds exactly one table with the header `{HEADER}`"
    );
    let mut rows = BTreeSet::new();
    // The line after the header is the delimiter row.
    for line in lines[starts[0] + 2..]
        .iter()
        .take_while(|line| line.trim_start().starts_with('|'))
    {
        let row = cells(line);
        assert_eq!(row.len(), 4, "a lint row has four cells: {line}");
        let name = row[0].trim_matches('`').to_string();
        assert!(
            rows.insert((name, row[1].clone(), row[2].clone(), row[3].clone())),
            "duplicate lint row: {line}"
        );
    }
    rows
}

/// Whether `line` is the header row, compared cell by cell so that the
/// column padding prim adds does not matter.
fn cells_match_header(line: &str) -> bool {
    line.trim_start().starts_with('|') && cells(line) == cells(HEADER)
}

/// The rows the catalogue implies: every row that has a lint name.
fn catalogue_rows() -> BTreeSet<Row> {
    ALL_CATALOGS
        .iter()
        .flat_map(|(_, catalog)| catalog.iter())
        .filter_map(|entry| {
            entry.lint.map(|name| {
                (
                    name.to_string(),
                    entry.code.as_str().to_string(),
                    default_level(entry).as_str().to_string(),
                    entry.summary.to_string(),
                )
            })
        })
        .collect()
}

#[test]
fn lints_page_matches_catalogue() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/book/lints.md");
    let page = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let book = book_rows(&page);
    let catalogue = catalogue_rows();
    let missing: Vec<&Row> = catalogue.difference(&book).collect();
    let extra: Vec<&Row> = book.difference(&catalogue).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "lints.md differs from the catalogue\n  rows the catalogue has and the book lacks: \
         {missing:#?}\n  rows the book has and the catalogue lacks: {extra:#?}"
    );
}
