use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
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
    live_writer: Option<crate::log_format::LiveLogWriter>,
) -> Result<CaptureLog> {
    let mut log = CaptureLog::from_config(config)?;
    let (tx, rx) = mpsc::sync_channel(4096);
    let mut live_writer = live_writer;
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
                let mut passthrough_writer = channel
                    .serial
                    .passthrough_port
                    .as_deref()
                    .map(|port| serial_io::open_serial_writer(&channel.serial, port))
                    .transpose()?;
                let spec = ChannelCaptureSpec {
                    channel_id: channel.id,
                    packet_len: channel.packet_len,
                    baud_rate: channel.serial.baud_rate,
                    data_bits: channel.serial.data_bits,
                    stop_bits: channel.serial.stop_bits,
                    parity: channel.serial.parity.clone(),
                    header: channel.header.clone(),
                    tail: channel.tail.clone(),
                };
                let stats = if let Some(writer) = passthrough_writer.as_mut() {
                    record_channel_reader(
                        spec,
                        reader.as_mut(),
                        &SystemClock,
                        &stop_requested,
                        Some(writer.as_mut()),
                        |record| {
                            if stop_requested.load(Ordering::Relaxed) {
                                return Ok(());
                            }

                            match tx.try_send(RecorderEvent::Packet(record)) {
                                Ok(()) => Ok(()),
                                Err(TrySendError::Full(RecorderEvent::Packet(record))) => {
                                    if stop_requested.load(Ordering::Relaxed) {
                                        Ok(())
                                    } else {
                                        tx.send(RecorderEvent::Packet(record))
                                            .context("record receiver dropped")
                                    }
                                }
                                Err(TrySendError::Disconnected(_)) => Ok(()),
                                Err(TrySendError::Full(_)) => {
                                    unreachable!("only packet events are sent here")
                                }
                            }
                        },
                    )?
                } else {
                    record_channel_reader(
                        spec,
                        reader.as_mut(),
                        &SystemClock,
                        &stop_requested,
                        None,
                        |record| {
                            if stop_requested.load(Ordering::Relaxed) {
                                return Ok(());
                            }

                            match tx.try_send(RecorderEvent::Packet(record)) {
                                Ok(()) => Ok(()),
                                Err(TrySendError::Full(RecorderEvent::Packet(record))) => {
                                    if stop_requested.load(Ordering::Relaxed) {
                                        Ok(())
                                    } else {
                                        tx.send(RecorderEvent::Packet(record))
                                            .context("record receiver dropped")
                                    }
                                }
                                Err(TrySendError::Disconnected(_)) => Ok(()),
                                Err(TrySendError::Full(_)) => {
                                    unreachable!("only packet events are sent here")
                                }
                            }
                        },
                    )?
                };
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

    let mut last_render = Instant::now();
    loop {
        if stop_requested.load(Ordering::Relaxed) {
            break;
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => handle_recorder_event(
                event,
                &mut log,
                &mut blink,
                &channel_names,
                &mut live_writer,
                &mut last_render,
            )?,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(rx);

    for handle in handles {
        handle.join().expect("recorder thread panicked");
    }

    log.records.sort_by_key(|record| record.timestamp_unix_ns);
    log.stats.sort_by_key(|stat| stat.channel_id);
    if let Some(mut writer) = live_writer {
        writer.finalize(&log.stats)?;
    }
    Ok(log)
}

fn handle_recorder_event(
    event: RecorderEvent,
    log: &mut CaptureLog,
    blink: &mut [bool],
    channel_names: &HashMap<u16, String>,
    live_writer: &mut Option<crate::log_format::LiveLogWriter>,
    last_render: &mut Instant,
) -> Result<()> {
    match event {
        RecorderEvent::Packet(record) => {
            if let Some(state) = blink.get_mut(record.channel_id as usize) {
                *state = !*state;
            }
            if let Some(writer) = live_writer.as_mut() {
                writer.write_packet(&record)?;
            }
            if last_render.elapsed() >= Duration::from_millis(100) {
                render_channel_lamps(log, blink);
                *last_render = Instant::now();
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

    Ok(())
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
            line.push(' ');
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

fn record_channel_reader<C, F>(
    spec: ChannelCaptureSpec,
    reader: &mut dyn serialport::SerialPort,
    clock: &C,
    stop_requested: &AtomicBool,
    mut passthrough_writer: Option<&mut dyn serialport::SerialPort>,
    mut on_packet: F,
) -> Result<ParserStats>
where
    C: Clock,
    F: FnMut(PacketRecord) -> Result<()>,
{
    let mut parser = PacketParser::new(spec.packet_len, spec.header, spec.tail);
    let mut buffer = [0u8; 4096];

    loop {
        if stop_requested.load(Ordering::Relaxed) {
            break;
        }

        match reader.bytes_to_read() {
            Ok(0) => {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            Ok(_) => {}
            Err(err) => return Err(err.into()),
        }

        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                if let Some(writer) = passthrough_writer.as_mut() {
                    writer
                        .write_all(&buffer[..n])
                        .context("failed to forward serial bytes to passthrough port")?;
                }
                let packets = parser.push_bytes(&buffer[..n]);
                let base_timestamp = clock.now_unix_ns();
                let interval_ns = estimated_packet_interval_ns(
                    spec.packet_len,
                    spec.baud_rate,
                    spec.data_bits,
                    spec.stop_bits,
                    &spec.parity,
                );
                let first_timestamp = base_timestamp.saturating_sub(
                    interval_ns.saturating_mul((packets.len().saturating_sub(1)) as u64),
                );

                for (idx, packet) in packets.into_iter().enumerate() {
                    on_packet(PacketRecord {
                        channel_id: spec.channel_id,
                        timestamp_unix_ns: first_timestamp
                            .saturating_add(interval_ns.saturating_mul(idx as u64)),
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

fn estimated_packet_interval_ns(
    packet_len: usize,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: &str,
) -> u64 {
    if packet_len == 0 || baud_rate == 0 || data_bits == 0 {
        return 0;
    }
    let parity_bits = if parity.eq_ignore_ascii_case("none") {
        0u128
    } else {
        1u128
    };
    let bits_per_byte = 1u128 + u128::from(data_bits) + u128::from(stop_bits) + parity_bits;
    let bits_per_packet = (packet_len as u128) * bits_per_byte;
    let ns = (bits_per_packet * 1_000_000_000u128) / u128::from(baud_rate);
    ns.min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::estimated_packet_interval_ns;

    #[test]
    fn estimates_packet_interval_from_len_and_baud() {
        assert_eq!(
            estimated_packet_interval_ns(10, 1_000_000, 8, 1, "none"),
            100_000
        );
        assert_eq!(
            estimated_packet_interval_ns(10, 1_000_000, 7, 2, "even"),
            110_000
        );
    }
}
