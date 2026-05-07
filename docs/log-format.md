# Log Format

Capture files use the `.srp` extension and a small little-endian binary format.

## Header

- magic: ASCII `SRP1`
- channel count: `u16`
- record count: `u64`
- stats count: `u16`

## Channel Table

For each channel:

- `channel_id`: `u16`
- `name`: length-prefixed UTF-8 string
- serial port config
- `packet_len`: `u32`
- `header`: length-prefixed bytes
- `tail`: length-prefixed bytes

Strings and byte arrays are encoded as:

- length: `u32`
- bytes: `[u8; length]`

## Records

For each packet record:

- `channel_id`: `u16`
- `timestamp_unix_ns`: `u64`
- `packet_len`: `u32`
- `packet_bytes`: `[u8; packet_len]`

The packet bytes are the complete parsed packet, including header and tail.

## Statistics

For each channel:

- `channel_id`: `u16`
- valid packet count: `u64`
- bad frame count: `u64`
- discarded byte count: `u64`
- incomplete tail byte count: `u64`
