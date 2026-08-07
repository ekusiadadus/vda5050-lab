use std::{fs, path::PathBuf, process::ExitCode};

use clap::Parser;
use vda5050_demo::{LsmartRunPlan, write_new_artifact};

#[derive(Debug, Parser)]
#[command(name = "vda5050-lsmart-plan")]
#[command(about = "Seal the official LSMART causal-demo plan before CONNECT")]
struct Cli {
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long)]
    run_id: String,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    inject_reconnect_fault: bool,
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vda5050-lsmart-plan: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(&cli.output_dir)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("output directory must be a real directory".into());
    }
    let plan = LsmartRunPlan::new(&cli.run_id, cli.inject_reconnect_fault)?;
    let mut bytes = serde_json::to_vec_pretty(&plan)?;
    bytes.push(b'\n');
    write_new_artifact(&cli.output_dir.join("lsmart-run-manifest.json"), &bytes)?;
    Ok(())
}
