use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use serial_repack::cli::{Cli, Command};
use serial_repack::{config, log_format, matlab_export, recorder, replay};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Record {
            config,
            out,
            sync_every,
        } => {
            let config = config::Config::from_path(config)?;
            let stop_requested = Arc::new(AtomicBool::new(false));
            {
                let stop_requested = Arc::clone(&stop_requested);
                ctrlc::set_handler(move || {
                    if !stop_requested.swap(true, Ordering::SeqCst) {
                        eprintln!("interrupt received, stopping capture and writing output...");
                    }
                })?;
            }
            let live_writer = log_format::LiveLogWriter::create(
                &out,
                &log_format::CaptureLog::from_config(&config)?,
                sync_every,
            )?;
            let log = recorder::record_from_serial(
                &config,
                Arc::clone(&stop_requested),
                Some(live_writer),
            )?;
            eprintln!();
            println!(
                "recorded {} packets to {}",
                log.records.len(),
                out.display()
            );
        }
        Command::Replay {
            input,
            mappings,
            speed,
        } => {
            let log = log_format::read_log_file(&input)?;
            let mappings = replay::parse_channel_mappings(&mappings)?;
            replay::replay_to_serial(&log, &mappings, speed)?;
        }
        Command::Export { input, out_dir } => {
            let log = log_format::read_log_file(&input)?;
            matlab_export::export_matlab(&log, &out_dir)?;
            println!("exported channel data to {}", out_dir.display());
        }
        Command::Inspect { input } => {
            let log = log_format::read_log_file(&input)?;
            print!("{}", log_format::inspect_summary(&log));
        }
    }

    Ok(())
}
