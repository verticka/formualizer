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

The fix lives in `docs/umya-shared-formula-quoted-sheet.patch`, a
`git format-patch` (apply with `git am`, base **PSU3D0/umya-spreadsheet**
rev `4b64d65` — the 2.3.2 base formualizer pins). It touches
`src/helper/formula.rs::parse_to_tokens`:

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

## Regression tests

The patch already appends two tests to `src/helper/formula.rs`
(`quoted_sheet_ref_round_trips`, `shared_adjustment_preserves_quoted_sheet`) —
both fail on stock umya and pass with the fix.

On the formualizer side, `crates/formualizer-workbook/tests/umya/shared_formula_quoted_sheet.rs`
loads `tests/fixtures/shared_formula_quoted_sheet.xlsx` end-to-end and asserts
`Main!B2 = Main!B3 = "42-x"`, `Main!B4 = ""`.

## Wiring (done)

formualizer pins `umya-spreadsheet = "=2.3.2"`, so the patched crate **must**
keep version 2.3.2 — i.e. the fix has to sit on the PSU3D0 2.3.2 base
(`4b64d65`), **not** on upstream umya 3.0.0 (which renames `Spreadsheet` ->
`Workbook`, drops the typed `set_formula_result_*` setters formualizer relies
on, and still carries this very bug). The fix is therefore the `4b64d65`
PSU3D0 base + this patch, published in the `verticka/umya-spreadsheet` fork and
pinned in the workspace `Cargo.toml`:

```toml
[patch.crates-io]
umya-spreadsheet = { git = "https://github.com/verticka/umya-spreadsheet", rev = "1b64aca9dd31172cacbd92cc2c5e8826e7005ff6" }
```

To reproduce that fork branch from scratch:

```sh
git clone https://github.com/verticka/umya-spreadsheet.git
cd umya-spreadsheet
git remote add psu3d0 https://github.com/PSU3D0/umya-spreadsheet.git
git fetch psu3d0 4b64d65daf19bba79800951cd38920d7eb95320c
git checkout -b fix/shared-formula-quoted-sheet-2.3.2 4b64d65daf19bba79800951cd38920d7eb95320c
git am < /path/to/formualizer/docs/umya-shared-formula-quoted-sheet.patch
cargo test --lib shared_formula_quoted_sheet   # 2 tests pass
git push origin fix/shared-formula-quoted-sheet-2.3.2
```
