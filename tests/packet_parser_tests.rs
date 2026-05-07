use serial_repack::packet::PacketParser;

#[test]
fn parses_continuous_packets() {
    let mut parser = PacketParser::new(6, vec![0xAA], vec![0x55]);
    let packets = parser.push_bytes(&[0xAA, 1, 2, 3, 4, 0x55, 0xAA, 5, 6, 7, 8, 0x55]);

    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0], vec![0xAA, 1, 2, 3, 4, 0x55]);
    assert_eq!(parser.stats().packets, 2);
}

#[test]
fn discards_noise_before_packet() {
    let mut parser = PacketParser::new(4, vec![0xAA], vec![0x55]);
    let packets = parser.push_bytes(&[0, 1, 0xAA, 9, 8, 0x55]);

    assert_eq!(packets, vec![vec![0xAA, 9, 8, 0x55]]);
    assert_eq!(parser.stats().discarded_bytes, 2);
}

#[test]
fn resyncs_after_bad_tail() {
    let mut parser = PacketParser::new(4, vec![0xAA], vec![0x55]);
    let packets = parser.push_bytes(&[0xAA, 1, 2, 3, 0xAA, 4, 5, 0x55]);

    assert_eq!(packets, vec![vec![0xAA, 4, 5, 0x55]]);
    assert_eq!(parser.stats().bad_frames, 1);
}

#[test]
fn reports_incomplete_tail() {
    let mut parser = PacketParser::new(4, vec![0xAA], vec![0x55]);
    assert!(parser.push_bytes(&[0xAA, 1]).is_empty());

    let stats = parser.finish();
    assert_eq!(stats.incomplete_tail_bytes, 2);
}
