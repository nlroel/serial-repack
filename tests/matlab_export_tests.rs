use std::fs;

use byteorder::{LittleEndian, ReadBytesExt};
use serial_repack::config::SerialConfig;
use serial_repack::log_format::{CaptureLog, ChannelMeta, PacketRecord};
use serial_repack::matlab_export::export_matlab;

#[test]
fn exports_per_channel_matlab_files() {
    let temp = tempfile::tempdir().unwrap();
    let log = CaptureLog {
        channels: vec![channel(0, "radar_a", 4), channel(1, "radar_b", 5)],
        records: vec![
            PacketRecord {
                channel_id: 0,
                timestamp_unix_ns: 100,
                packet: vec![0xAA, 1, 2, 0x55],
            },
            PacketRecord {
                channel_id: 1,
                timestamp_unix_ns: 120,
                packet: vec![0xBB, 1, 2, 3, 0x66],
            },
        ],
        stats: vec![],
    };

    export_matlab(&log, temp.path()).unwrap();

    assert_eq!(
        fs::read(temp.path().join("radar_a/data.bin")).unwrap(),
        vec![0xAA, 1, 2, 0x55]
    );

    let ts_bytes = fs::read(temp.path().join("radar_b/timestamps_ns.bin")).unwrap();
    let mut cursor = std::io::Cursor::new(ts_bytes);
    assert_eq!(cursor.read_u64::<LittleEndian>().unwrap(), 120);

    assert!(!temp.path().join("load_capture.m").exists());
    assert!(!temp.path().join("load_timestamps.m").exists());
}

fn channel(id: u16, name: &str, packet_len: usize) -> ChannelMeta {
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
        packet_len,
        header: vec![],
        tail: vec![],
    }
}
