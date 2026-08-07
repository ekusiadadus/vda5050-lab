use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, ValueEnum};
use vda5050_demo::{LiveRunPlan, LiveScenario, write_live_plan};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScenarioArg {
    Fault,
    Control,
}

#[derive(Debug, Parser)]
#[command(name = "vda5050-live-plan")]
#[command(about = "Write the fixed pre-CONNECT Tier 1 live-demo plan")]
struct Cli {
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long)]
    run_id: String,
    #[arg(long, value_enum, default_value_t = ScenarioArg::Fault)]
    scenario: ScenarioArg,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let scenario = match cli.scenario {
        ScenarioArg::Fault => LiveScenario::ReconnectMissingOnline,
        ScenarioArg::Control => LiveScenario::ReconnectControl,
    };
    let result = LiveRunPlan::new(&cli.run_id, scenario)
        .map_err(|error| error.to_string())
        .and_then(|plan| {
            write_live_plan(&cli.output_dir, &plan)
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vda5050-live-plan: {error}");
            ExitCode::FAILURE
        }
    }
}
