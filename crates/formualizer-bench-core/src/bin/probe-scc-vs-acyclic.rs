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
    }

    let cli = Cli::parse();
    let dir = std::env::temp_dir().join("scc-vs-acyclic");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(if cli.acyclic {
        format!("acyclic-{}.xlsx", cli.rows)
    } else {
        format!("cyclic-{}.xlsx", cli.rows)
    });

    let acyclic = cli.acyclic;
    write_workbook(&path, |book| {
        let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
        // A1: volatile guard so every recalc re-dirties the whole sheet.
        sheet
            .get_cell_mut((1, 1))
            .set_formula("IF(NOW()>DATE(2000,1,1),1,1)");
        for r in 0..cli.rows {
            let row = r as u32 + 2;
            if acyclic {
                // No mutual reference -> two independent acyclic formulas.
                sheet
                    .get_cell_mut((1, row))
                    .set_formula(format!("IF($A$1,1,99)"));
                sheet
                    .get_cell_mut((2, row))
                    .set_formula(format!("IF($A$1,98,1)"));
            } else {
                // Mutual reference -> one 2-member phantom SCC per row.
                sheet
                    .get_cell_mut((1, row))
                    .set_formula(format!("IF($A$1,1,B{row})"));
                sheet
                    .get_cell_mut((2, row))
                    .set_formula(format!("IF($A$1,A{row},1)"));
            }
            sheet
                .get_cell_mut((3, row))
                .set_formula(format!("A{row}+B{row}"));
        }
    });

    let mut config = WorkbookConfig::ephemeral();
    config.eval = config.eval.with_cycle(CycleConfig::iterate(100, 0.001));
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
    Ok(())
}
