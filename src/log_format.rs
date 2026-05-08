use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};

use crate::config::{Config, SerialConfig};
use crate::packet::ParserStats;

const MAGIC: &[u8; 4] = b"SRP1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureLog {
    pub channels: Vec<ChannelMeta>,
    pub records: Vec<PacketRecord>,
    pub stats: Vec<ChannelStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelMeta {
    pub id: u16,
    pub name: String,
    pub serial: SerialConfig,
    pub packet_len: usize,
    pub header: Vec<u8>,
    pub tail: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PacketRecord {
    pub channel_id: u16,
    pub timestamp_unix_ns: u64,
    pub packet: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelStats {
    pub channel_id: u16,
    pub packets: u64,
    pub bad_frames: u64,
    pub discarded_bytes: u64,
    pub incomplete_tail_bytes: u64,
}

impl CaptureLog {
    pub fn from_config(config: &Config) -> Result<Self> {
        let channels = config
            .validated_channels()?
            .into_iter()
            .enumerate()
            .map(|(idx, ch)| {
                let id = u16::try_from(idx).context("too many channels for u16 channel_id")?;
                Ok(ChannelMeta {
                    id,
                    name: ch.name,
                    serial: ch.serial,
                    packet_len: ch.packet_len,
                    header: ch.header,
                    tail: ch.tail,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            channels,
            records: Vec::new(),
            stats: Vec::new(),
        })
    }

    pub fn channel_by_name(&self, name: &str) -> Option<&ChannelMeta> {
        self.channels.iter().find(|channel| channel.name == name)
    }

    pub fn channel_by_id(&self, id: u16) -> Option<&ChannelMeta> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    pub fn first_timestamp(&self) -> Option<u64> {
        self.records
            .iter()
            .map(|record| record.timestamp_unix_ns)
            .min()
    }
}

impl From<(u16, ParserStats)> for ChannelStats {
    fn from((channel_id, stats): (u16, ParserStats)) -> Self {
        Self {
            channel_id,
            packets: stats.packets,
            bad_frames: stats.bad_frames,
            discarded_bytes: stats.discarded_bytes,
            incomplete_tail_bytes: stats.incomplete_tail_bytes,
        }
    }
}

pub fn write_log_file(path: impl AsRef<Path>, log: &CaptureLog) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create parent directory {}", parent.display())
            })?;
        }
    }

    let file = File::create(path.as_ref())
        .with_context(|| format!("failed to create {}", path.as_ref().display()))?;
    write_log(BufWriter::new(file), log)
}

pub fn read_log_file(path: impl AsRef<Path>) -> Result<CaptureLog> {
    let file = File::open(path.as_ref())
        .with_context(|| format!("failed to open {}", path.as_ref().display()))?;
    read_log(BufReader::new(file))
}

pub fn write_log(mut writer: impl Write, log: &CaptureLog) -> Result<()> {
    writer.write_all(MAGIC)?;
    writer.write_u16::<LittleEndian>(u16::try_from(log.channels.len())?)?;
    writer.write_u64::<LittleEndian>(u64::try_from(log.records.len())?)?;
    writer.write_u16::<LittleEndian>(u16::try_from(log.stats.len())?)?;

    for channel in &log.channels {
        writer.write_u16::<LittleEndian>(channel.id)?;
        write_string(&mut writer, &channel.name)?;
        write_string(&mut writer, &channel.serial.port)?;
        writer.write_u32::<LittleEndian>(channel.serial.baud_rate)?;
        writer.write_u8(channel.serial.data_bits)?;
        writer.write_u8(channel.serial.stop_bits)?;
        write_string(&mut writer, &channel.serial.parity)?;
        write_string(&mut writer, &channel.serial.flow_control)?;
        writer.write_u64::<LittleEndian>(channel.serial.read_timeout_ms)?;
        writer.write_u32::<LittleEndian>(u32::try_from(channel.packet_len)?)?;
        write_bytes(&mut writer, &channel.header)?;
        write_bytes(&mut writer, &channel.tail)?;
    }

    for record in &log.records {
        writer.write_u16::<LittleEndian>(record.channel_id)?;
        writer.write_u64::<LittleEndian>(record.timestamp_unix_ns)?;
        writer.write_u32::<LittleEndian>(u32::try_from(record.packet.len())?)?;
        writer.write_all(&record.packet)?;
    }

    for stat in &log.stats {
        writer.write_u16::<LittleEndian>(stat.channel_id)?;
        writer.write_u64::<LittleEndian>(stat.packets)?;
        writer.write_u64::<LittleEndian>(stat.bad_frames)?;
        writer.write_u64::<LittleEndian>(stat.discarded_bytes)?;
        writer.write_u64::<LittleEndian>(stat.incomplete_tail_bytes)?;
    }

    Ok(())
}

pub fn read_log(mut reader: impl Read) -> Result<CaptureLog> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        bail!("invalid SRP magic");
    }

    let channel_count = reader.read_u16::<LittleEndian>()? as usize;
    let record_count = reader.read_u64::<LittleEndian>()? as usize;
    let stat_count = reader.read_u16::<LittleEndian>()? as usize;

    let mut channels = Vec::with_capacity(channel_count);
    for _ in 0..channel_count {
        let id = reader.read_u16::<LittleEndian>()?;
        let name = read_string(&mut reader)?;
        let port = read_string(&mut reader)?;
        let baud_rate = reader.read_u32::<LittleEndian>()?;
        let data_bits = reader.read_u8()?;
        let stop_bits = reader.read_u8()?;
        let parity = read_string(&mut reader)?;
        let flow_control = read_string(&mut reader)?;
        let read_timeout_ms = reader.read_u64::<LittleEndian>()?;
        let packet_len = reader.read_u32::<LittleEndian>()? as usize;
        let header = read_bytes(&mut reader)?;
        let tail = read_bytes(&mut reader)?;
        channels.push(ChannelMeta {
            id,
            name,
            serial: SerialConfig {
                port,
                baud_rate,
                data_bits,
                stop_bits,
                parity,
                flow_control,
                read_timeout_ms,
            },
            packet_len,
            header,
            tail,
        });
    }

    let packet_lens: HashMap<u16, usize> = channels
        .iter()
        .map(|channel| (channel.id, channel.packet_len))
        .collect();

    let mut records = Vec::with_capacity(record_count);
    for _ in 0..record_count {
        let channel_id = reader.read_u16::<LittleEndian>()?;
        let timestamp_unix_ns = reader.read_u64::<LittleEndian>()?;
        let packet_len = reader.read_u32::<LittleEndian>()? as usize;
        let expected_len = packet_lens
            .get(&channel_id)
            .copied()
            .ok_or_else(|| anyhow!("record references unknown channel_id {channel_id}"))?;
        if packet_len != expected_len {
            bail!("record packet_len {packet_len} does not match channel {channel_id} length {expected_len}");
        }
        let mut packet = vec![0u8; packet_len];
        reader.read_exact(&mut packet)?;
        records.push(PacketRecord {
            channel_id,
            timestamp_unix_ns,
            packet,
        });
    }

    let mut stats = Vec::with_capacity(stat_count);
    for _ in 0..stat_count {
        stats.push(ChannelStats {
            channel_id: reader.read_u16::<LittleEndian>()?,
            packets: reader.read_u64::<LittleEndian>()?,
            bad_frames: reader.read_u64::<LittleEndian>()?,
            discarded_bytes: reader.read_u64::<LittleEndian>()?,
            incomplete_tail_bytes: reader.read_u64::<LittleEndian>()?,
        });
    }

    Ok(CaptureLog {
        channels,
        records,
        stats,
    })
}

pub fn inspect_summary(log: &CaptureLog) -> String {
    let mut out = String::new();
    out.push_str("serial-repack capture\n");
    out.push_str(&format!("channels: {}\n", log.channels.len()));
    out.push_str(&format!("records: {}\n", log.records.len()));
    if let Some(first) = log.first_timestamp() {
        let last = log
            .records
            .iter()
            .map(|record| record.timestamp_unix_ns)
            .max()
            .unwrap_or(first);
        out.push_str(&format!("time_range_ns: {first}..{last}\n"));
    }
    for channel in &log.channels {
        let count = log
            .records
            .iter()
            .filter(|record| record.channel_id == channel.id)
            .count();
        out.push_str(&format!(
            "- {} id={} packet_len={} records={}\n",
            channel.name, channel.id, channel.packet_len, count
        ));
    }
    out
}

fn write_string(writer: &mut impl Write, value: &str) -> Result<()> {
    write_bytes(writer, value.as_bytes())
}

fn read_string(reader: &mut impl Read) -> Result<String> {
    let bytes = read_bytes(reader)?;
    String::from_utf8(bytes).context("invalid UTF-8 string in log")
}

fn write_bytes(writer: &mut impl Write, value: &[u8]) -> Result<()> {
    writer.write_u32::<LittleEndian>(u32::try_from(value.len())?)?;
    writer.write_all(value)?;
    Ok(())
}

fn read_bytes(reader: &mut impl Read) -> Result<Vec<u8>> {
    let len = reader.read_u32::<LittleEndian>()? as usize;
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}
