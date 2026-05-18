# Configuration

Configuration uses TOML. Each serial port is a named channel.

```toml
[[channels]]
name = "radar_a"
enabled = true

[channels.serial]
port = "/dev/ttyUSB0"
passthrough_port = "/dev/ttyUSB10"
baud_rate = 921600
data_bits = 8
stop_bits = 1
parity = "none"
flow_control = "none"
read_timeout_ms = 100

[channels.packet]
packet_len = 64
header = "AA55"
tail = "55AA"
```

## Channel Fields

- `name`: unique channel name used in logs, replay mappings, and MATLAB exports.
- `enabled`: optional; defaults to `true`.
- `serial`: serial-port settings for recording.
- `packet`: parser settings for this channel.

## Serial Fields

- `port`: platform serial port path, such as `/dev/ttyUSB0` or `COM7`.
- `passthrough_port`: optional output serial port for raw-byte passthrough while recording. Example: Linux can point to a `socat` PTY; Windows can point to a `com0com` virtual COM port.
- `baud_rate`: baud rate.
- `data_bits`: one of `5`, `6`, `7`, `8`; default `8`.
- `stop_bits`: one of `1`, `2`; default `1`.
- `parity`: `none`, `odd`, or `even`; default `none`.
- `flow_control`: `none`, `software`, or `hardware`; default `none`.
- `read_timeout_ms`: read timeout in milliseconds; must be greater than `0`; default `100`.

## Packet Fields

- `packet_len`: whole packet length in bytes, including header and tail.
- `header`: fixed header as a hex string.
- `tail`: fixed tail as a hex string. Use an empty string (`""`) when your protocol has no tail marker (only header + fixed length).

`packet_len` must be at least `len(header) + len(tail)`.
