use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "serial-repack")]
#[command(about = "Multi-channel serial packet recorder, replay tool, and binary exporter.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Record {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 1)]
        sync_every: u64,
    },
    Replay {
        #[arg(long = "in")]
        input: PathBuf,
        #[arg(long = "map", required = true)]
        mappings: Vec<String>,
        #[arg(long, default_value_t = 1.0)]
        speed: f64,
    },
    Export {
        #[arg(long = "in")]
        input: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
    },
    Inspect {
        #[arg(long = "in")]
        input: PathBuf,
    },
}
