use anyhow::{Result, anyhow};
use clap::Parser;
use conv_document::experiment::MergeWorkload;
use std::time::Instant;
#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 102400)]
    document_bytes: usize,
    #[arg(long, default_value_t = 2000)]
    samples: usize,
    #[arg(long, default_value_t = 200)]
    warmup: usize,
    #[arg(long, default_value_t = 0)]
    seed: u64,
    #[arg(long)]
    deletion_heavy: bool,
    #[arg(long, default_value = "benchmarks/raw-results/merge-native.json")]
    output: std::path::PathBuf,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let mut experiment = MergeWorkload::new(
        args.document_bytes,
        args.samples + args.warmup,
        args.deletion_heavy,
        args.seed,
    )
    .map_err(|e| anyhow!("setup: {e:?}"))?;
    for _ in 0..args.warmup {
        experiment
            .merge_one()
            .map_err(|e| anyhow!("warmup: {e:?}"))?;
    }
    let mut timings = Vec::new();
    for _ in 0..args.samples {
        let start = Instant::now();
        anyhow::ensure!(
            experiment
                .merge_one()
                .map_err(|e| anyhow!("merge: {e:?}"))?,
            "workload exhausted"
        );
        timings.push(start.elapsed().as_nanos() as u64);
    }
    anyhow::ensure!(experiment.verify(), "canonical final state mismatch");
    timings.sort_unstable();
    let percentile = |p: usize| {
        timings
            .get((timings.len() * p).div_ceil(100).saturating_sub(1))
            .copied()
            .unwrap_or(0)
    };
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    let report = serde_json::json!({"commit":String::from_utf8_lossy(&commit.stdout).trim(),"transport":"none, isolated native core", "document_bytes":args.document_bytes,
        "samples":args.samples,"warmup":args.warmup,"seed":args.seed,"deletion_heavy":args.deletion_heavy,"verified":experiment.verify(),
        "tombstones":experiment.document().deleted_ids().count(),"serialized_state_bytes":postcard::to_allocvec(experiment.document())?.len(),
        "merge_nanoseconds":{"p50":percentile(50),"p95":percentile(95),"p99":percentile(99)},"raw_nanoseconds":timings,
        "hardware":std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default().lines().find(|line| line.starts_with("model name")).unwrap_or("unknown"),
        "toolchain":String::from_utf8_lossy(&std::process::Command::new("rustc").arg("--version").output()?.stdout).trim(),
        "method":"one scalar insert per delta; excludes workload generation, decoding, protocol history, rendering, network and persistence"});
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.output, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Verified native merge experiment: {}",
        args.output.display()
    );
    Ok(())
}
