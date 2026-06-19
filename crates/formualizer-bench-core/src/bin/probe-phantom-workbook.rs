//! Ad-hoc probe: load a real XLSX, run `evaluate_all` a few times under
//! `CyclePolicy::Iterate`, and print per-pass wall time plus the SCC cycle
//! telemetry. Used to measure the phantom-SCC per-task overhead optimization
//! (RFC #112 Stage 2b) on real workbooks.
//!
//! ```bash
//! cargo run --release -p formualizer-bench-core --features formualizer_runner \
//!   --bin probe-phantom-workbook -- --path /some/workbook.xlsx --passes 3
//! ```

#[cfg(not(feature = "formualizer_runner"))]
fn main() {
    eprintln!(
        "This binary requires feature `formualizer_runner`: cargo run -p formualizer-bench-core --features formualizer_runner --bin probe-phantom-workbook -- ..."
    );
    std::process::exit(2);
}

#[cfg(feature = "formualizer_runner")]
fn main() -> anyhow::Result<()> {
    use std::{path::PathBuf, time::Instant};

    use clap::Parser;
    use formualizer_eval::engine::CycleConfig;
    use formualizer_workbook::{
        LoadStrategy, SpreadsheetReader, UmyaAdapter, Workbook, WorkbookConfig,
    };

    #[derive(Parser, Debug)]
    struct Cli {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, default_value_t = 3)]
        passes: usize,
        /// max_iterations for iterative calc.
        #[arg(long, default_value_t = 100)]
        max_iterations: u32,
        #[arg(long, default_value_t = 0.001)]
        max_change: f64,
        /// Disable computed-overlay mirroring (isolates overlay write cost).
        #[arg(long)]
        no_overlay: bool,
    }

    let cli = Cli::parse();

    let mut config = WorkbookConfig::ephemeral();
    config.eval = config
        .eval
        .with_cycle(CycleConfig::iterate(cli.max_iterations, cli.max_change));
    if cli.no_overlay {
        config.eval = config.eval.with_formula_overlay(false);
    }

    let open_start = Instant::now();
    let backend = UmyaAdapter::open_path(&cli.path)
        .map_err(|e| anyhow::anyhow!("open {}: {e}", cli.path.display()))?;
    let mut wb = Workbook::from_reader(backend, LoadStrategy::EagerAll, config)
        .map_err(|e| anyhow::anyhow!("load workbook: {e}"))?;
    println!("load_ms={:.1}", open_start.elapsed().as_secs_f64() * 1000.0);

    for pass in 0..cli.passes {
        let t = Instant::now();
        let res = wb
            .evaluate_all()
            .map_err(|e| anyhow::anyhow!("evaluate_all pass {pass}: {e}"))?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let tel = wb.engine().last_cycle_telemetry().clone();
        println!(
            "pass={pass} eval_ms={ms:.1} scc_task_ms={} computed_vertices={} cycle_errors={} | static_sccs={} phantom_sccs={} iterated_sccs={} converged_sccs={} live_cycles={} settle_passes_total={} max_passes_single_scc={} circ_stamped={}",
            tel.elapsed_ms,
            res.computed_vertices,
            res.cycle_errors,
            tel.static_sccs,
            tel.phantom_sccs,
            tel.iterated_sccs,
            tel.converged_sccs,
            tel.live_cycles_witnessed,
            tel.settle_passes_total,
            tel.max_passes_single_scc,
            tel.circ_cells_stamped,
        );
    }

    Ok(())
}
