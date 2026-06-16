//! Regression tests for INDEX over named ranges (issue #150).
//!
//! `INDEX(name, n)` must resolve the named range and return the n-th value,
//! including when the named range targets a worksheet whose name contains
//! non-ASCII (accented) characters such as `Données`.

use crate::engine::named_range::{NameScope, NamedDefinition};
use crate::engine::{EvalConfig, eval::Engine};
use crate::reference::{CellRef, Coord, RangeRef};
use crate::test_workbook::TestWorkbook;
use formualizer_parse::LiteralValue;
use formualizer_parse::parser::parse;

/// Define a workbook-scoped name `Vals` covering `<sheet>!A1:A5` (values
/// 10,20,30,40,50) and assert `=INDEX(Vals,3)` on `Sheet1` returns 30.
fn assert_index_named_range_on_sheet(sheet: &str) {
    let mut engine = Engine::new(TestWorkbook::new(), EvalConfig::default());

    engine.add_sheet(sheet).unwrap();
    for r in 1..=5u32 {
        engine
            .set_cell_value(sheet, r, 1, LiteralValue::Number(r as f64 * 10.0))
            .unwrap();
    }

    let sheet_id = engine.graph.sheet_id(sheet).unwrap();
    let start = CellRef::new(sheet_id, Coord::new(0, 0, true, true)); // A1
    let end = CellRef::new(sheet_id, Coord::new(4, 0, true, true)); // A5
    engine
        .define_name(
            "Vals",
            NamedDefinition::Range(RangeRef::new(start, end)),
            NameScope::Workbook,
        )
        .unwrap();

    engine
        .set_cell_formula("Sheet1", 1, 2, parse("=INDEX(Vals,3)").unwrap())
        .unwrap();
    engine.evaluate_all().unwrap();

    assert_eq!(
        engine.get_cell_value("Sheet1", 1, 2),
        Some(LiteralValue::Number(30.0)),
        "INDEX(Vals,3) on sheet {sheet:?} should resolve to 30"
    );
}

#[test]
fn index_over_named_range_targeting_ascii_sheet() {
    // Baseline: INDEX over a named range works for an ordinary sheet name.
    assert_index_named_range_on_sheet("Data");
}

#[test]
fn index_over_named_range_targeting_accented_sheet() {
    // Regression for #150: accented sheet name must resolve identically.
    assert_index_named_range_on_sheet("Données");
}
