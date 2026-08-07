use std::{path::PathBuf, process::ExitCode};

use clap::Parser;
use vda5050_demo::{BrokerTarget, RunOptions, run_live};

#[derive(Debug, Parser)]
#[command(name = "vda5050-demo")]
#[command(about = "Isolated Tier 1 VDA 5050 simulator and trace producer")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    broker_host: String,
    #[arg(long, default_value_t = 18_884)]
    broker_port: u16,
    #[arg(long)]
    isolated_network: bool,
    #[arg(long, default_value = "demo/artifacts")]
    output_dir: PathBuf,
    #[arg(long, default_value = "demo-0001")]
    run_id: String,
    #[arg(long, default_value_t = 1)]
    robot_count: usize,
    #[arg(long)]
    no_animation: bool,
    #[arg(long)]
    control_online_after_reconnect: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let broker = match BrokerTarget::new(&cli.broker_host, cli.isolated_network) {
        Ok(target) => target,
        Err(error) => {
            eprintln!("vda5050-demo: {error}");
            return ExitCode::FAILURE;
        }
    };
    let options = RunOptions {
        broker,
        port: cli.broker_port,
        output_dir: cli.output_dir,
        run_id: cli.run_id,
        robot_count: cli.robot_count,
        animate: !cli.no_animation,
        send_online_after_reconnect: cli.control_online_after_reconnect,
    };
    match run_live(&options) {
        Ok(artifacts) => {
            println!("\nTrace: {}", artifacts.trace.display());
            println!("Evidence: {}", artifacts.evidence_manifest.display());
            println!("Run manifest: {}", artifacts.run_manifest.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("vda5050-demo: {error}");
            ExitCode::FAILURE
        }
    }
}
