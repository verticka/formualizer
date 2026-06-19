//! Compare per-formula evaluation cost of guarded phantom SCC pairs (cyclic,
//! routed through `evaluate_scc_unit`) against an acyclic control with the same
//! formula complexity (routed through the normal layer path). Isolates the
//! SCC-task overhead from the base formula-evaluation cost. The guard is a
//! volatile cell so every recalc re-dirties the whole sheet (mirrors the real
//! phantom_scc_demo workbook).

#[cfg(not(feature = "formualizer_runner"))]
fn main() {
    eprintln!("requires --features formualizer_runner");
    std::process::exit(2);
}

#[cfg(feature = "formualizer_runner")]
fn main() -> anyhow::Result<()> {
    use std::time::Instant;

    use clap::Parser;
    use formualizer_eval::engine::CycleConfig;
    use formualizer_testkit::write_workbook;
    use formualizer_workbook::{
        LoadStrategy, SpreadsheetReader, UmyaAdapter, Workbook, WorkbookConfig,
    };

    #[derive(Parser, Debug)]
    struct Cli {
        #[arg(long, default_value_t = 100_000)]
        rows: usize,
        #[arg(long, default_value_t = 4)]
        passes: usize,
        /// Build the acyclic control (no mutual reference, no SCC).
        #[arg(long)]
        acyclic: bool,
        /// Disable the value-change recalc gate (measure its overhead).
        #[arg(long)]
        no_gate: bool,
        /// Worst case: each consumer also reads the changing volatile, so every
        /// edit-free recalc changes every consumer value (gate cannot skip).
        #[arg(long)]
        changing: bool,
    }

    let cli = Cli::parse();
    let dir = std::env::temp_dir().join("scc-vs-acyclic");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "{}{}-{}.xlsx",
        if cli.acyclic { "acyclic" } else { "cyclic" },
        if cli.changing { "-chg" } else { "" },
        cli.rows
    ));

    let acyclic = cli.acyclic;
    let changing = cli.changing;
    write_workbook(&path, |book| {
        let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
        // A1: volatile guard so every recalc re-dirties the whole sheet. In
        // `changing` mode it is NOW() itself (its value changes every recalc).
        sheet.get_cell_mut((1, 1)).set_formula(if changing {
            "RAND()".to_string()
        } else {
            "IF(NOW()>DATE(2000,1,1),1,1)".to_string()
        });
        for r in 0..cli.rows {
            let row = r as u32 + 2;
            let guard = if changing {
                format!("$A$1>DATE(2000,1,1)")
            } else {
                format!("$A$1")
            };
            if acyclic {
                // No mutual reference -> two independent acyclic formulas.
                sheet
                    .get_cell_mut((1, row))
                    .set_formula(format!("IF({guard},1,99)"));
                sheet
                    .get_cell_mut((2, row))
                    .set_formula(format!("IF({guard},98,1)"));
            } else {
                // Mutual reference -> one 2-member phantom SCC per row.
                sheet
                    .get_cell_mut((1, row))
                    .set_formula(format!("IF({guard},1,B{row})"));
                sheet
                    .get_cell_mut((2, row))
                    .set_formula(format!("IF({guard},A{row},1)"));
            }
            // In `changing` mode the consumer reads the changing volatile, so
            // its value differs every recalc and the gate must re-evaluate it.
            sheet.get_cell_mut((3, row)).set_formula(if changing {
                format!("A{row}+B{row}+$A$1")
            } else {
                format!("A{row}+B{row}")
            });
        }
        // A plain value input (col 5 = E1) and one formula reading it (E2):
        // editing E1 should recalc only E2, skipping every phantom SCC.
        sheet.get_cell_mut((5, 1)).set_value_number(1.0);
        sheet.get_cell_mut((5, 2)).set_formula("E1*2".to_string());
    });

    let mut config = WorkbookConfig::ephemeral();
    config.eval = config.eval.with_cycle(CycleConfig::iterate(100, 0.001));
    config.eval.value_change_gate_enabled = !cli.no_gate;
    let backend = UmyaAdapter::open_path(&path)
        .map_err(|e| anyhow::anyhow!("open {}: {e}", path.display()))?;
    let mut wb = Workbook::from_reader(backend, LoadStrategy::EagerAll, config)
        .map_err(|e| anyhow::anyhow!("load: {e}"))?;

    let mode = if acyclic { "acyclic" } else { "cyclic " };
    for pass in 0..cli.passes {
        let t = Instant::now();
        let res = wb.evaluate_all().map_err(|e| anyhow::anyhow!("eval: {e}"))?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let per = ms * 1000.0 / (res.computed_vertices.max(1) as f64);
        let tel = wb.engine().last_cycle_telemetry().clone();
        println!(
            "[{mode}] pass={pass} eval_ms={ms:.1} computed={} us_per_computed={per:.3} phantom_sccs={}",
            res.computed_vertices, tel.phantom_sccs
        );
    }

    // Interactive scenario: edit ONE plain value (E1) and time the recalc.
    for round in 0..3 {
        wb.set_value("Sheet1", 1, 5, formualizer_workbook::LiteralValue::Number(round as f64 + 2.0))
            .map_err(|e| anyhow::anyhow!("set_value E1: {e}"))?;
        let t = Instant::now();
        let res = wb.evaluate_all().map_err(|e| anyhow::anyhow!("recalc: {e}"))?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let tel = wb.engine().last_cycle_telemetry().clone();
        let e2 = wb.get_value("Sheet1", 2, 5);
        println!(
            "[{mode}] edit E1 round={round} recalc_ms={ms:.1} computed={} phantom_sccs={} E2={e2:?}",
            res.computed_vertices, tel.phantom_sccs
        );
    }
    Ok(())
}
