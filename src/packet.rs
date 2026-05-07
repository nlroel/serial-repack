#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParserStats {
    pub packets: u64,
    pub bad_frames: u64,
    pub discarded_bytes: u64,
    pub incomplete_tail_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct PacketParser {
    packet_len: usize,
    header: Vec<u8>,
    tail: Vec<u8>,
    buffer: Vec<u8>,
    stats: ParserStats,
}

impl PacketParser {
    pub fn new(packet_len: usize, header: Vec<u8>, tail: Vec<u8>) -> Self {
        Self {
            packet_len,
            header,
            tail,
            buffer: Vec::with_capacity(packet_len * 2),
            stats: ParserStats::default(),
        }
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(bytes);
        let mut packets = Vec::new();

        loop {
            let Some(header_pos) = find_subslice(&self.buffer, &self.header) else {
                self.discard_without_header();
                break;
            };

            if header_pos > 0 {
                self.buffer.drain(..header_pos);
                self.stats.discarded_bytes += header_pos as u64;
            }

            if self.buffer.len() < self.packet_len {
                break;
            }

            let tail_start = self.packet_len - self.tail.len();
            if self.buffer[tail_start..self.packet_len] == self.tail {
                let packet: Vec<u8> = self.buffer.drain(..self.packet_len).collect();
                self.stats.packets += 1;
                packets.push(packet);
            } else {
                self.buffer.drain(..1);
                self.stats.bad_frames += 1;
                self.stats.discarded_bytes += 1;
            }
        }

        packets
    }

    pub fn finish(mut self) -> ParserStats {
        if !self.buffer.is_empty() {
            self.stats.incomplete_tail_bytes = self.buffer.len() as u64;
        }
        self.stats
    }

    pub fn stats(&self) -> &ParserStats {
        &self.stats
    }

    fn discard_without_header(&mut self) {
        if self.buffer.len() < self.header.len() {
            return;
        }

        let keep = self.header.len().saturating_sub(1);
        let discard = self.buffer.len().saturating_sub(keep);
        if discard > 0 {
            self.buffer.drain(..discard);
            self.stats.discarded_bytes += discard as u64;
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
