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
    replay_records(log, &mut writers, &mut sleeper, speed)
}

pub fn replay_records<W: PacketWriter, S: Sleeper>(
    log: &CaptureLog,
    writers: &mut HashMap<u16, W>,
    sleeper: &mut S,
    speed: f64,
) -> Result<()> {
    if speed <= 0.0 {
        bail!("speed must be greater than 0");
    }

    let selected: HashSet<u16> = writers.keys().copied().collect();
    let mut events: Vec<&PacketRecord> = log
        .records
        .iter()
        .filter(|record| selected.contains(&record.channel_id))
        .collect();
    events.sort_by_key(|record| record.timestamp_unix_ns);

    let Some(first_selected) = events.first().map(|record| record.timestamp_unix_ns) else {
        return Ok(());
    };
    let first_global = log.first_timestamp().unwrap_or(first_selected);
    if first_selected > first_global {
        sleep_scaled(first_selected - first_global, speed, sleeper);
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
