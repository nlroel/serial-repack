use std::collections::HashMap;
use std::time::Duration;

use serial_repack::config::SerialConfig;
use serial_repack::log_format::{CaptureLog, ChannelMeta, PacketRecord};
use serial_repack::replay::{parse_channel_mappings, replay_records, PacketWriter, Sleeper};

#[derive(Debug, Default)]
struct MockWriter {
    packets: Vec<Vec<u8>>,
}

impl PacketWriter for MockWriter {
    fn write_packet(&mut self, packet: &[u8]) -> std::io::Result<()> {
        self.packets.push(packet.to_vec());
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MockSleeper {
    sleeps: Vec<Duration>,
}

impl Sleeper for MockSleeper {
    fn sleep(&mut self, duration: Duration) {
        self.sleeps.push(duration);
    }
}

#[test]
fn parses_channel_mappings() {
    let mappings = parse_channel_mappings(&[
        "radar_a=/dev/ttyUSB1".to_string(),
        "radar_b=COM7".to_string(),
    ])
    .unwrap();

    assert_eq!(mappings["radar_a"], "/dev/ttyUSB1");
    assert_eq!(mappings["radar_b"], "COM7");
}

#[test]
fn replays_only_selected_channels_and_keeps_global_offset() {
    let log = sample_log();
    let mut writers = HashMap::from([(1, MockWriter::default())]);
    let mut sleeper = MockSleeper::default();

    replay_records(&log, &mut writers, &mut sleeper, 1.0, None, None).unwrap();

    assert_eq!(
        writers.get(&1).unwrap().packets,
        vec![vec![0xB1], vec![0xB2]]
    );
    assert_eq!(
        sleeper.sleeps,
        vec![Duration::from_nanos(50), Duration::from_nanos(20)]
    );
}

#[test]
fn replays_events_in_timestamp_order() {
    let log = sample_log();
    let mut writers = HashMap::from([(0, MockWriter::default()), (1, MockWriter::default())]);
    let mut sleeper = MockSleeper::default();

    replay_records(&log, &mut writers, &mut sleeper, 1.0, None, None).unwrap();

    assert_eq!(writers.get(&0).unwrap().packets, vec![vec![0xA0]]);
    assert_eq!(
        writers.get(&1).unwrap().packets,
        vec![vec![0xB1], vec![0xB2]]
    );
    assert_eq!(
        sleeper.sleeps,
        vec![Duration::from_nanos(50), Duration::from_nanos(20)]
    );
}

#[test]
fn replays_from_nearest_unix_timestamp() {
    let log = sample_log();
    let mut writers = HashMap::from([(1, MockWriter::default())]);
    let mut sleeper = MockSleeper::default();

    replay_records(
        &log,
        &mut writers,
        &mut sleeper,
        1.0,
        Some(0.000000160),
        Some(0.000000160),
    )
    .unwrap();

    assert_eq!(writers.get(&1).unwrap().packets, vec![vec![0xB1]]);
    assert_eq!(sleeper.sleeps, vec![]);
}

#[test]
fn rejects_invalid_time_range() {
    let log = sample_log();
    let mut writers = HashMap::from([(1, MockWriter::default())]);
    let mut sleeper = MockSleeper::default();

    let err =
        replay_records(&log, &mut writers, &mut sleeper, 1.0, Some(1.0), Some(0.1)).unwrap_err();
    assert!(err
        .to_string()
        .contains("--to must be greater than or equal to --from"));
}

fn sample_log() -> CaptureLog {
    CaptureLog {
        channels: vec![channel(0, "radar_a"), channel(1, "radar_b")],
        records: vec![
            PacketRecord {
                channel_id: 0,
                timestamp_unix_ns: 100,
                packet: vec![0xA0],
            },
            PacketRecord {
                channel_id: 1,
                timestamp_unix_ns: 150,
                packet: vec![0xB1],
            },
            PacketRecord {
                channel_id: 1,
                timestamp_unix_ns: 170,
                packet: vec![0xB2],
            },
        ],
        stats: vec![],
    }
}

fn channel(id: u16, name: &str) -> ChannelMeta {
    ChannelMeta {
        id,
        name: name.to_string(),
        serial: SerialConfig {
            port: "unused".to_string(),
            passthrough_port: None,
            baud_rate: 115200,
            data_bits: 8,
            stop_bits: 1,
            parity: "none".to_string(),
            flow_control: "none".to_string(),
            read_timeout_ms: 100,
        },
        packet_len: 1,
        header: vec![],
        tail: vec![],
    }
}
