use serial_repack::config::Config;

fn valid_config() -> &'static str {
    r#"
[[channels]]
name = "radar_a"
enabled = true

[channels.serial]
port = "/dev/ttyUSB0"
baud_rate = 921600
data_bits = 8
stop_bits = 1
parity = "none"
flow_control = "none"
read_timeout_ms = 100

[channels.packet]
packet_len = 8
header = "AA55"
tail = "55AA"

[[channels]]
name = "radar_b"
enabled = true

[channels.serial]
port = "COM7"
baud_rate = 115200

[channels.packet]
packet_len = 10
header = "AABB"
tail = "BBAA"
"#
}

#[test]
fn parses_valid_multi_channel_config() {
    let config = Config::from_toml_str(valid_config()).unwrap();
    let channels = config.validated_channels().unwrap();

    assert_eq!(channels.len(), 2);
    assert_eq!(channels[0].name, "radar_a");
    assert_eq!(channels[0].header, vec![0xAA, 0x55]);
    assert_eq!(channels[1].serial.data_bits, 8);
}

#[test]
fn rejects_duplicate_channel_names() {
    let text = valid_config().replace("radar_b", "radar_a");
    let err = Config::from_toml_str(&text).unwrap_err().to_string();
    assert!(err.contains("duplicate channel name"));
}

#[test]
fn rejects_invalid_hex() {
    let text = valid_config().replace("AA55", "AA5");
    let err = Config::from_toml_str(&text).unwrap_err().to_string();
    assert!(err.contains("invalid hex"));
}

#[test]
fn rejects_packet_len_smaller_than_header_tail() {
    let text = valid_config().replace("packet_len = 8", "packet_len = 3");
    let err = Config::from_toml_str(&text).unwrap_err().to_string();
    assert!(err.contains("smaller than header+tail"));
}

#[test]
fn rejects_invalid_serial_setting() {
    let text = valid_config().replace("data_bits = 8", "data_bits = 9");
    let err = Config::from_toml_str(&text).unwrap_err().to_string();
    assert!(err.contains("unsupported serial setting"));
}


#[test]
fn rejects_zero_read_timeout() {
    let text = valid_config().replace("read_timeout_ms = 100", "read_timeout_ms = 0");
    let err = Config::from_toml_str(&text).unwrap_err().to_string();
    assert!(err.contains("unsupported serial setting"));
    assert!(err.contains("read_timeout_ms"));
}
