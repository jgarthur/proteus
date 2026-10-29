//! Runs an exact runner manifest with deterministic genome/lineage observations.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use proteus::genome_observe::GenomeAudit;
use proteus::observe::collect_metrics_with_census;
use proteus::runner::{current_build_info, load_run_manifest};
use proteus::{
    apply_bootstrap, EventTotals, Simulation, TickReport, BOOTSTRAP_RNG_SALT, BOOTSTRAP_RNG_VERSION,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

const USAGE: &str =
    "proteus-audit --manifest PATH --output DIRECTORY [--trace-start TICK --trace-end TICK]";
const SCHEMA: &str = "1.0.0";

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() {
    if let Err(error) = run() {
        eprintln!("proteus-audit: {error}");
        std::process::exit(1);
    }
}

fn options() -> Result<BTreeMap<String, String>> {
    let mut args = std::env::args().skip(1);
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if key == "--help" {
            println!("{USAGE}");
            std::process::exit(0);
        }
        if !["--manifest", "--output", "--trace-start", "--trace-end"].contains(&key.as_str()) {
            return Err(format!("unknown option {key}; {USAGE}").into());
        }
        let value = args.next().ok_or(USAGE)?;
        if options.insert(key, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    Ok(options)
}

fn file(directory: &Path, name: &str) -> Result<BufWriter<File>> {
    Ok(BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(name))?,
    ))
}

fn line(writer: &mut BufWriter<File>, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn finish(mut writer: BufWriter<File>) -> Result<()> {
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

struct Samples {
    lineage: BufWriter<File>,
    genomes: BufWriter<File>,
    metrics: BufWriter<File>,
    known_genomes: BTreeSet<String>,
}

impl Samples {
    fn record(
        &mut self,
        simulation: &Simulation,
        report: TickReport,
        events: EventTotals,
        audit: &GenomeAudit,
    ) -> Result<()> {
        let mut live = 0;
        let mut genome_counts: BTreeMap<String, u64> = BTreeMap::new();
        for (cell, state) in simulation.grid().cells().iter().enumerate() {
            let Some(program) = &state.program else {
                continue;
            };
            let mut digest = String::with_capacity(64);
            for byte in Sha256::digest(&program.code) {
                write!(digest, "{byte:02x}")?;
            }
            if self.known_genomes.insert(digest.clone()) {
                line(
                    &mut self.genomes,
                    &json!({"genome":digest, "code":program.code}),
                )?;
            }
            if program.live {
                live += 1;
                *genome_counts.entry(digest.clone()).or_default() += 1;
            }
            line(
                &mut self.lineage,
                &json!({
                    "tick":simulation.tick(), "cell":cell,
                    "uid":program.lineage.uid, "parent_uid":program.lineage.parent,
                    "generation":program.lineage.generation,
                    "birth_tick":program.lineage.birth_tick(), "live":program.live,
                    "genome":digest, "size":program.code.len(),
                }),
            )?;
        }
        let metrics =
            collect_metrics_with_census(simulation.grid(), 0, simulation.tick(), report, events);
        line(
            &mut self.metrics,
            &json!({
                "tick":simulation.tick(), "metrics":metrics, "genome_events":audit.totals,
                "live":live, "live_genome_counts":genome_counts,
                "pending_constructions":audit.pending_constructions(),
            }),
        )?;
        Ok(())
    }
}

fn run() -> Result<()> {
    if std::env::args().skip(1).eq(["--build-info"]) {
        serde_json::to_writer(std::io::stdout(), &current_build_info()?)?;
        println!();
        return Ok(());
    }
    let options = options()?;
    let manifest = PathBuf::from(options.get("--manifest").ok_or(USAGE)?);
    let output = PathBuf::from(options.get("--output").ok_or(USAGE)?);
    let trace_start = options
        .get("--trace-start")
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(0);
    let trace_end = options
        .get("--trace-end")
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(0);
    if trace_end < trace_start {
        return Err("trace-end must be >= trace-start".into());
    }
    let run = load_run_manifest(&manifest)?;
    let build = current_build_info()?;
    let mut simulation = Simulation::new(run.manifest.simulation.clone())?;
    apply_bootstrap(&mut simulation, &run.manifest.bootstrap)?;
    // Reserve a fresh directory only after validating all inputs. A summary is
    // written last; its absence identifies an interrupted/incomplete audit.
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&output)?;
    #[cfg(feature = "rayon")]
    let engine_threads = rayon::current_num_threads();
    #[cfg(not(feature = "rayon"))]
    let engine_threads = 1;
    let mut provenance = file(&output, "manifest.json")?;
    line(
        &mut provenance,
        &json!({
            "audit_schema_version": SCHEMA, "input_digest":run.input_digest,
            "input":run.manifest, "build":build,
            "engine_threads":engine_threads,
            "bootstrap_rng_version":BOOTSTRAP_RNG_VERSION,
            "bootstrap_rng_salt":format!("0x{BOOTSTRAP_RNG_SALT:016x}"),
            "trace_start":trace_start, "trace_end":trace_end,
        }),
    )?;
    finish(provenance)?;
    let mut samples = Samples {
        lineage: file(&output, "lineage.jsonl")?,
        genomes: file(&output, "genomes.jsonl")?,
        metrics: file(&output, "metrics.jsonl")?,
        known_genomes: BTreeSet::new(),
    };
    let mut trace = file(&output, "events.jsonl")?;
    let mut births = file(&output, "births.jsonl")?;
    let mut attempts = file(&output, "actions.jsonl")?;
    let mut audit = GenomeAudit::new(trace_start, trace_end);
    let mut totals = EventTotals::default();
    let mut report = TickReport::default();
    let initial_code_bytes: u64 = simulation
        .grid()
        .cells()
        .iter()
        .filter_map(|cell| cell.program.as_ref())
        .map(|p| p.code.len() as u64)
        .sum();
    samples.record(&simulation, report, totals, &audit)?;
    let mut last_sample = 0;
    eprintln!(
        "proteus-audit: {} ticks -> {}",
        run.manifest.limits.ticks,
        output.display()
    );
    while simulation.tick() < run.manifest.limits.ticks {
        report = simulation.run_tick_report_with_genomes(&mut audit);
        totals.record(report);
        // Cross-check independently accumulated causal counts against engine
        // counters on every tick, including short-lived unsampled programs.
        if audit.totals.boots != totals.boot_births
            || audit.totals.maintenance_deaths != totals.deaths
            || audit.totals.spawns != totals.spawn_births
            || audit.totals.base_mutations != totals.base_mutations
            || audit.totals.background_mutations != totals.background_mutations
        {
            return Err(format!(
                "observer counts disagree with engine at tick {}",
                simulation.tick()
            )
            .into());
        }
        if audit.totals.constructions_started
            != audit.totals.boots_with_known_construction
                + audit.totals.constructions_died_unbooted
                + audit.pending_constructions() as u64
        {
            return Err("construction outcomes do not balance".into());
        }
        let code_bytes: u64 = simulation
            .grid()
            .cells()
            .iter()
            .filter_map(|cell| cell.program.as_ref())
            .map(|p| p.code.len() as u64)
            .sum();
        let expected_bytes = i128::from(initial_code_bytes)
            + i128::from(audit.totals.appends)
            + i128::from(audit.totals.spawns)
            - i128::from(audit.totals.adjacent_deletions)
            - i128::from(audit.totals.local_deleted_bytes)
            - i128::from(audit.totals.maintenance_deleted_bytes);
        if i128::from(code_bytes) != expected_bytes {
            return Err(format!(
                "genome byte accounting disagrees at tick {}",
                simulation.tick()
            )
            .into());
        }
        for attempt in audit.attempts.drain(..) {
            line(&mut attempts, &attempt)?;
        }
        for birth in audit.births.drain(..) {
            line(&mut births, &birth)?;
        }
        for event in audit.events.drain(..) {
            line(&mut trace, &event)?;
        }
        if simulation.tick() % run.manifest.observation.every_n_ticks == 0 {
            samples.record(&simulation, report, totals, &audit)?;
            last_sample = simulation.tick();
        }
    }
    if last_sample != simulation.tick() {
        samples.record(&simulation, report, totals, &audit)?;
    }
    finish(trace)?;
    finish(births)?;
    finish(attempts)?;
    finish(samples.lineage)?;
    finish(samples.genomes)?;
    finish(samples.metrics)?;
    let mut summary = file(&output, "summary.json")?;
    line(
        &mut summary,
        &json!({
            "audit_schema_version":SCHEMA, "input_digest":run.input_digest,
            "tick":simulation.tick(), "genome_events":audit.totals,
            "pending_constructions":audit.pending_constructions(),
            "final_metrics": collect_metrics_with_census(simulation.grid(), 0, simulation.tick(), report, totals),
        }),
    )?;
    finish(summary)?;
    eprintln!("proteus-audit: completed tick {}", simulation.tick());
    Ok(())
}
