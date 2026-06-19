// Generates a minimal, vendor-free workbook that reproduces the umya
// shared-formula expansion bug: a sheet whose name needs quoting -- "(1)" --
// referenced from a SHARED formula on another sheet. umya writes individual
// formulas, so the shared grouping is injected by a post-processing step.
fn main() {
    let out = std::env::args().nth(1).unwrap();
    let mut book = umya_spreadsheet::new_file();
    // Rename default sheet to the quoted-name sheet "(1)".
    book.get_sheet_by_name_mut("Sheet1").unwrap().set_name("(1)");
    book.get_sheet_mut(&0).unwrap().get_cell_mut((5u32, 38u32)).set_value_number(42.0); // E38
    book.new_sheet("Main").unwrap();
    let main = book.get_sheet_by_name_mut("Main").unwrap();
    for row in 2u32..=4 {
        main.get_cell_mut((1u32, row)).set_value_number(if row < 4 { 1.0 } else { 0.0 }); // A
        main.get_cell_mut((2u32, row))
            .set_formula(format!("IF(A{row}=1,'(1)'!$E$38&\"-x\",\"\")")); // B
    }
    umya_spreadsheet::writer::xlsx::write(&book, std::path::Path::new(&out)).unwrap();
    println!("wrote {out}");
}
