use serial_repack::config::SerialConfig;
use serial_repack::log_format::{
    read_log, write_log, CaptureLog, ChannelMeta, ChannelStats, PacketRecord,
};

fn sample_log() -> CaptureLog {
    CaptureLog {
        channels: vec![
            ChannelMeta {
                id: 0,
                name: "radar_a".to_string(),
                serial: serial("/dev/ttyUSB0"),
                packet_len: 4,
                header: vec![0xAA],
                tail: vec![0x55],
            },
            ChannelMeta {
                id: 1,
                name: "radar_b".to_string(),
                serial: serial("COM7"),
                packet_len: 5,
                header: vec![0xBB],
                tail: vec![0x66],
            },
        ],
        records: vec![
            PacketRecord {
                channel_id: 1,
                timestamp_unix_ns: 100,
                packet: vec![0xBB, 1, 2, 3, 0x66],
            },
            PacketRecord {
                channel_id: 0,
                timestamp_unix_ns: 90,
                packet: vec![0xAA, 1, 2, 0x55],
            },
        ],
        stats: vec![ChannelStats {
            channel_id: 0,
            packets: 1,
            bad_frames: 0,
            discarded_bytes: 2,
            incomplete_tail_bytes: 0,
        }],
    }
}

fn serial(port: &str) -> SerialConfig {
    SerialConfig {
        port: port.to_string(),
        baud_rate: 115200,
        data_bits: 8,
        stop_bits: 1,
        parity: "none".to_string(),
        flow_control: "none".to_string(),
        read_timeout_ms: 100,
    }
}

#[test]
fn roundtrips_multi_channel_log() {
    let log = sample_log();
    let mut bytes = Vec::new();

    write_log(&mut bytes, &log).unwrap();
    let decoded = read_log(bytes.as_slice()).unwrap();

    assert_eq!(decoded, log);
}

#[test]
fn rejects_invalid_magic() {
    let err = read_log(&b"BAD!"[..]).unwrap_err().to_string();
    assert!(err.contains("invalid SRP magic"));
}

#[test]
fn rejects_wrong_record_packet_len() {
    let mut log = sample_log();
    log.records[0].packet.push(0);
    let mut bytes = Vec::new();
    write_log(&mut bytes, &log).unwrap();

    let err = read_log(bytes.as_slice()).unwrap_err().to_string();
    assert!(err.contains("does not match"));
}
