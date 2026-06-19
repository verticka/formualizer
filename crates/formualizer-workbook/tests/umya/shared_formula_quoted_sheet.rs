// Regression test for the umya-spreadsheet shared-formula quoted-sheet bug.
//
// A shared formula whose master references a sheet that needs quoting
// (e.g. `'(1)'!$E$38`) used to corrupt the reference during umya's
// shared-formula expansion, producing `(1)!$E$38` and failing to parse.
// With the patched umya the workbook loads and the shared members
// evaluate identically to the master. See
// `docs/umya-shared-formula-quoted-sheet.md`.

use formualizer_workbook::{
    LiteralValue, LoadStrategy, SpreadsheetReader, UmyaAdapter, Workbook, WorkbookConfig,
};

fn fixture_path() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR points at crates/formualizer-workbook; the fixture
    // lives at the workspace root under tests/fixtures.
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/shared_formula_quoted_sheet.xlsx")
}

// Requires the patched umya-spreadsheet (the fix is not yet upstream). With
// the default `[patch.crates-io]` (PSU3D0 2.3.2, unpatched) this loads the
// shared member as an error instead of "42-x". Pin the patched umya (see
// docs/umya-shared-formula-quoted-sheet.md) and run with `-- --ignored`.
#[test]
#[ignore = "needs patched umya-spreadsheet; see docs/umya-shared-formula-quoted-sheet.md"]
fn loads_shared_formula_with_quoted_sheet_ref() {
    let path = fixture_path();
    assert!(path.exists(), "missing fixture: {}", path.display());

    let backend = UmyaAdapter::open_path(&path)
        .expect("open workbook with quoted-sheet shared formula");
    let mut wb = Workbook::from_reader(
        backend,
        LoadStrategy::EagerAll,
        WorkbookConfig::interactive(),
    )
    .expect("load workbook with quoted-sheet shared formula");

    // Master (B2) and shared member (B3) both have A=1 -> "42-x".
    let b2 = wb.evaluate_cell("Main", 2, 2).expect("evaluate Main!B2");
    let b3 = wb.evaluate_cell("Main", 3, 2).expect("evaluate Main!B3");
    let b4 = wb.evaluate_cell("Main", 4, 2).expect("evaluate Main!B4");

    assert_eq!(b2, LiteralValue::Text("42-x".to_string()));
    assert_eq!(b3, LiteralValue::Text("42-x".to_string()));
    assert_eq!(b4, LiteralValue::Text(String::new()));
}
