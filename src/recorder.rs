use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::config::Config;
use crate::log_format::{CaptureLog, ChannelStats, PacketRecord};
use crate::packet::{PacketParser, ParserStats};
use crate::serial_io;

pub trait Clock: Send + Sync + 'static {
    fn now_unix_ns(&self) -> u64;
}

#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ns(&self) -> u64 {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch");
        duration.as_secs() * 1_000_000_000 + u64::from(duration.subsec_nanos())
    }
}

struct ChannelCaptureSpec {
    channel_id: u16,
    packet_len: usize,
    header: Vec<u8>,
    tail: Vec<u8>,
}

enum RecorderEvent {
    Packet(PacketRecord),
    Done(ChannelStats),
    Error(anyhow::Error),
}

pub fn record_from_serial(
    config: &Config,
    stop_requested: Arc<AtomicBool>,
    mut live_writer: Option<crate::log_format::LiveLogWriter>,
) -> Result<CaptureLog> {
    let mut log = CaptureLog::from_config(config)?;
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    let channel_names: HashMap<u16, String> = log
        .channels
        .iter()
        .map(|ch| (ch.id, ch.name.clone()))
        .collect();
    let mut blink = vec![false; log.channels.len()];

    for channel in log.channels.clone() {
        let tx = tx.clone();
        let stop_requested = Arc::clone(&stop_requested);
        handles.push(thread::spawn(move || {
            let result = (|| -> Result<ChannelStats> {
                let mut reader = serial_io::open_serial_reader(&channel.serial)?;
                let spec = ChannelCaptureSpec {
                    channel_id: channel.id,
                    packet_len: channel.packet_len,
                    header: channel.header.clone(),
                    tail: channel.tail.clone(),
                };
                let stats = record_channel_reader(
                    spec,
                    &mut reader,
                    &SystemClock,
                    &stop_requested,
                    |record| {
                        tx.send(RecorderEvent::Packet(record))
                            .context("record receiver dropped")
                    },
                )?;
                Ok(ChannelStats::from((channel.id, stats)))
            })();

            match result {
                Ok(stats) => {
                    let _ = tx.send(RecorderEvent::Done(stats));
                }
                Err(err) => {
                    let _ = tx.send(RecorderEvent::Error(err));
                }
            }
        }));
    }
    drop(tx);

    for event in rx {
        match event {
            RecorderEvent::Packet(record) => {
                if let Some(state) = blink.get_mut(record.channel_id as usize) {
                    *state = !*state;
                }
                if let Some(writer) = live_writer.as_mut() {
                    writer.write_packet(&record)?;
                }
                render_channel_lamps(&log, &blink);
                log.records.push(record)
            }
            RecorderEvent::Done(stats) => {
                let name = channel_names
                    .get(&stats.channel_id)
                    .map(String::as_str)
                    .unwrap_or("<unknown>");
                eprintln!(
                    "channel done: {name} (id={}) packets={} bad_frames={} discarded_bytes={}",
                    stats.channel_id, stats.packets, stats.bad_frames, stats.discarded_bytes
                );
                log.stats.push(stats)
            }
            RecorderEvent::Error(err) => return Err(err),
        }
    }

    for handle in handles {
        handle.join().expect("recorder thread panicked");
    }

    log.records.sort_by_key(|record| record.timestamp_unix_ns);
    log.stats.sort_by_key(|stat| stat.channel_id);
    if let Some(writer) = live_writer.as_mut() {
        writer.finalize(&log.stats)?;
    }
    Ok(log)
}

fn render_channel_lamps(log: &CaptureLog, blink: &[bool]) {
    let name_width = log
        .channels
        .iter()
        .map(|channel| channel.name.chars().count())
        .max()
        .unwrap_or(1);
    let mut channels: Vec<_> = log.channels.iter().collect();
    channels.sort_by_key(|channel| channel.id);
    let mut line = String::from("\rchannels: ");
    for (idx, channel) in channels.iter().enumerate() {
        let lamp = if blink.get(channel.id as usize).copied().unwrap_or(false) {
            "🟢"
        } else {
            "⚪"
        };
        if idx > 0 {
            line.push_str(" ");
        }
        line.push_str(&format!(
            "#{} {:width$}:{}",
            channel.id,
            channel.name,
            lamp,
            width = name_width
        ));
    }
    eprint!("{line}");
}

fn record_channel_reader<R, C, F>(
    spec: ChannelCaptureSpec,
    reader: &mut R,
    clock: &C,
    stop_requested: &AtomicBool,
    mut on_packet: F,
) -> Result<ParserStats>
where
    R: Read,
    C: Clock,
    F: FnMut(PacketRecord) -> Result<()>,
{
    let mut parser = PacketParser::new(spec.packet_len, spec.header, spec.tail);
    let mut buffer = [0u8; 4096];

    loop {
        if stop_requested.load(Ordering::Relaxed) {
            break;
        }
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                for packet in parser.push_bytes(&buffer[..n]) {
                    on_packet(PacketRecord {
                        channel_id: spec.channel_id,
                        timestamp_unix_ns: clock.now_unix_ns(),
                        packet,
                    })?;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.into()),
        }
    }

    Ok(parser.finish())
}
