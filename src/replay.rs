use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};

use crate::log_format::{CaptureLog, PacketRecord};
use crate::serial_io;

pub trait PacketWriter {
    fn write_packet(&mut self, packet: &[u8]) -> std::io::Result<()>;
}

impl<T: Write> PacketWriter for T {
    fn write_packet(&mut self, packet: &[u8]) -> std::io::Result<()> {
        self.write_all(packet)
    }
}

impl PacketWriter for Box<dyn PacketWriter> {
    fn write_packet(&mut self, packet: &[u8]) -> std::io::Result<()> {
        self.as_mut().write_packet(packet)
    }
}

pub trait Sleeper {
    fn sleep(&mut self, duration: Duration);
}

#[derive(Debug, Default)]
pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

pub fn parse_channel_mappings(values: &[String]) -> Result<HashMap<String, String>> {
    let mut mappings = HashMap::new();
    for value in values {
        let (channel, port) = value
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid --map value {value:?}, expected channel=port"))?;
        if channel.is_empty() || port.is_empty() {
            bail!("invalid --map value {value:?}, expected channel=port");
        }
        if mappings
            .insert(channel.to_string(), port.to_string())
            .is_some()
        {
            bail!("duplicate replay mapping for channel {channel}");
        }
    }
    Ok(mappings)
}

pub fn replay_to_serial(
    log: &CaptureLog,
    mappings: &HashMap<String, String>,
    speed: f64,
    from: Option<f64>,
    to: Option<f64>,
) -> Result<()> {
    if speed <= 0.0 {
        bail!("speed must be greater than 0");
    }

    let mut writers: HashMap<u16, Box<dyn PacketWriter>> = HashMap::new();
    for (channel_name, port) in mappings {
        let channel = log
            .channel_by_name(channel_name)
            .ok_or_else(|| anyhow!("unknown channel in mapping: {channel_name}"))?;
        let serial = serial_io::open_serial_writer(&channel.serial, port)
            .with_context(|| format!("failed to open replay port {port} for {channel_name}"))?;
        writers.insert(channel.id, Box::new(serial));
    }

    let mut sleeper = ThreadSleeper;
    replay_records(log, &mut writers, &mut sleeper, speed, from, to)
}

pub fn replay_records<W: PacketWriter, S: Sleeper>(
    log: &CaptureLog,
    writers: &mut HashMap<u16, W>,
    sleeper: &mut S,
    speed: f64,
    from: Option<f64>,
    to: Option<f64>,
) -> Result<()> {
    if speed <= 0.0 {
        bail!("speed must be greater than 0");
    }

    if let (Some(start), Some(end)) = (from, to) {
        if end < start {
            bail!("--to must be greater than or equal to --from");
        }
    }

    let selected: HashSet<u16> = writers.keys().copied().collect();
    let mut selected_events: Vec<&PacketRecord> = log
        .records
        .iter()
        .filter(|record| selected.contains(&record.channel_id))
        .collect();
    selected_events.sort_by_key(|record| record.timestamp_unix_ns);

    if selected_events.is_empty() {
        return Ok(());
    }

    let default_start_ns = log.first_timestamp().unwrap_or(0);
    let start_ns = from
        .map(|seconds| unix_seconds_to_ns(seconds, "--from"))
        .transpose()?
        .and_then(|target_ns| nearest_timestamp(&selected_events, target_ns))
        .unwrap_or(default_start_ns);

    let end_ns = to
        .map(|seconds| unix_seconds_to_ns(seconds, "--to"))
        .transpose()?
        .map(|target_ns| nearest_timestamp(&selected_events, target_ns).unwrap_or(target_ns));

    let events: Vec<&PacketRecord> = selected_events
        .into_iter()
        .filter(|record| record.timestamp_unix_ns >= start_ns)
        .filter(|record| end_ns.map_or(true, |end| record.timestamp_unix_ns <= end))
        .collect();

    let Some(first_selected) = events.first().map(|record| record.timestamp_unix_ns) else {
        return Ok(());
    };
    if first_selected > start_ns {
        sleep_scaled(first_selected - start_ns, speed, sleeper);
    }

    let mut previous = first_selected;
    for event in events {
        if event.timestamp_unix_ns > previous {
            sleep_scaled(event.timestamp_unix_ns - previous, speed, sleeper);
        }
        let writer = writers
            .get_mut(&event.channel_id)
            .ok_or_else(|| anyhow!("missing writer for channel_id {}", event.channel_id))?;
        writer.write_packet(&event.packet)?;
        previous = event.timestamp_unix_ns;
    }

    Ok(())
}

fn sleep_scaled<S: Sleeper>(delta_ns: u64, speed: f64, sleeper: &mut S) {
    let scaled_ns = (delta_ns as f64 / speed).round();
    if scaled_ns > 0.0 {
        sleeper.sleep(Duration::from_nanos(scaled_ns as u64));
    }
}

fn unix_seconds_to_ns(seconds: f64, arg_name: &str) -> Result<u64> {
    if seconds < 0.0 {
        bail!("{arg_name} must be greater than or equal to 0");
    }
    let nanos = seconds * 1_000_000_000.0;
    if !nanos.is_finite() || nanos > u64::MAX as f64 {
        bail!("{arg_name} is too large");
    }
    Ok(nanos.round() as u64)
}

fn nearest_timestamp(records: &[&PacketRecord], target_ns: u64) -> Option<u64> {
    records
        .iter()
        .min_by_key(|record| record.timestamp_unix_ns.abs_diff(target_ns))
        .map(|record| record.timestamp_unix_ns)
}
