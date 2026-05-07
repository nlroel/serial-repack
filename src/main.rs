use anyhow::Result;
use clap::Parser;
use serial_repack::cli::{Cli, Command};
use serial_repack::{config, log_format, matlab_export, recorder, replay};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Record { config, out } => {
            let config = config::Config::from_path(config)?;
            let log = recorder::record_from_serial(&config)?;
            log_format::write_log_file(&out, &log)?;
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
