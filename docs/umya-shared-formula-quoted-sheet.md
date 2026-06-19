# Bug: `umya-spreadsheet` corrupts quoted sheet references in shared formulas

## Symptom

Loading a real `.xlsx` whose **shared formulas** reference a sheet whose name
needs quoting (e.g. `'(1)'!$E$38`, common with ELCIA/numbered sheets) fails at
ingest with:

```
Formula parse error at <sheet>!<cell>: ParserError ... Reached end of formula while parsing string
```

This is **not** a `formualizer-parse` bug — the master formula parses fine in
isolation. It is a bug in the `umya-spreadsheet` reader's shared-formula
*expansion*: a shared-formula member (e.g. `<f t="shared" si="14"/>`) is
reconstructed from its master by `parse_to_tokens` → coordinate adjustment →
`render`, and that round-trip mangles any single-quoted sheet reference.

## Impact

Any workbook with a sheet name requiring quotes (`(1)`, `My Sheet`, names
starting with a digit, …) that appears inside a **shared** formula cannot be
loaded. Non-shared formulas are unaffected (umya returns their text verbatim,
without the token round-trip).

## Minimal, vendor-free reproduction

`cargo run -p formualizer-workbook --features umya --example gen_shared_fixture -- repro.xlsx`
(see `crates/formualizer-workbook/examples/gen_shared_fixture.rs`) produces
`tests/fixtures/shared_formula_quoted_sheet.xlsx`: two sheets — `(1)` with
`E38 = 42`, and `Main` with a shared formula over `B2:B4`
(`=IF(A2=1,'(1)'!$E$38&"-x","")`). The master is `B2`; `B3`/`B4` are shared
members, so umya must expand them.

- **Stock umya** → load fails (`Main!B3: ... Reached end of formula while parsing string`).
- **Patched umya** → loads; `B2 = B3 = "42-x"`, `B4 = ""`.

## Root cause (3 issues) and fix

See `docs/umya-shared-formula-quoted-sheet.patch` (apply in the
`umya-spreadsheet` repo). In `src/helper/formula.rs::parse_to_tokens`:

1. On a `'` (start of a quoted sheet path), the tokenizer sets `in_string = true`
   instead of `in_path = true`, so it scans for a closing `"` and swallows the
   sheet reference and everything after it as one "string".
2. The single-quoted path drops its surrounding quotes, so re-serialization
   loses them.

In `src/helper/address.rs::join_address`:

3. `split_address` strips the surrounding quotes from the sheet name for
   comparison, but `join_address` never re-adds them — so `'(1)'!$E$38`
   round-trips to the invalid `(1)!$E$38`. `join_address` must re-quote sheet
   names that are not bare identifiers (and double internal `'`).

## Regression tests (for the `umya-spreadsheet` PR)

Add to `src/helper/formula.rs` (both fail on stock, pass with the patch):

```rust
#[cfg(test)]
mod shared_formula_quoted_sheet_tests {
    use super::*;

    #[test]
    fn quoted_sheet_ref_round_trips() {
        let f = r#"IF(A2=1,'(1)'!$E$38&"-x","")"#;
        assert_eq!(render(&parse_to_tokens(format!("={}", f))), f);
    }

    #[test]
    fn shared_adjustment_preserves_quoted_sheet() {
        let mut tokens = parse_to_tokens(r#"=IF(A2=1,'(1)'!$E$38&"-x","")"#);
        let out =
            adjustment_insert_formula_coordinate(&mut tokens, &0, &0, &2, &1, "", "Main", true);
        assert!(out.contains("'(1)'!$E$38"), "quoted sheet ref corrupted: {out}");
        assert!(!out.contains("(1)!$E$38"), "sheet name lost its quotes: {out}");
    }
}
```
