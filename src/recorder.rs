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

enum RecorderEvent {
    Packet(PacketRecord),
    Done(ChannelStats),
    Error(anyhow::Error),
}

pub fn record_from_serial(config: &Config, stop_requested: Arc<AtomicBool>) -> Result<CaptureLog> {
    let mut log = CaptureLog::from_config(config)?;
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    let channel_names: HashMap<u16, String> = log
        .channels
        .iter()
        .map(|ch| (ch.id, ch.name.clone()))
        .collect();
    let mut total_packets: u64 = 0;

    for channel in log.channels.clone() {
        let tx = tx.clone();
        let stop_requested = Arc::clone(&stop_requested);
        handles.push(thread::spawn(move || {
            let result = (|| -> Result<ChannelStats> {
                let mut reader = serial_io::open_serial_reader(&channel.serial)?;
                let stats = record_channel_reader(
                    channel.id,
                    channel.packet_len,
                    channel.header.clone(),
                    channel.tail.clone(),
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
                total_packets += 1;
                if total_packets % 100 == 0 {
                    eprintln!("recording... total packets matched: {total_packets}");
                }
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
    Ok(log)
}

pub fn record_channel_reader<R, C, F>(
    channel_id: u16,
    packet_len: usize,
    header: Vec<u8>,
    tail: Vec<u8>,
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
    let mut parser = PacketParser::new(packet_len, header, tail);
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
                        channel_id,
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
