# serial-repack

`serial-repack` is a Rust CLI for multi-channel serial packet capture, timestamped storage, timed replay, packet parsing, and binary export.

It is designed for fixed-length serial protocols where each packet has:

- a fixed header
- a fixed whole-packet length
- a fixed tail

Each configured serial port is treated as a named channel. Capture uses one process-wide system clock and stores every valid packet as `channel_id + timestamp_unix_ns + packet_bytes`.

## Build

```bash
cargo build
```

Release builds:

```bash
cargo build --release
```

## Test

```bash
cargo test
```

## Record

```bash
serial-repack record --config examples/multi_channel.toml --out capture.srp --sync-every 10
```

Use Linux port names such as `/dev/ttyUSB0` and Windows port names such as `COM7` in the TOML config.

## Replay

Replay only selected channels by mapping log channel names to target serial ports:

```bash
serial-repack replay \
  --in capture.srp \
  --map radar_a=/dev/ttyUSB3 \
  --map radar_b=COM7
```

Unmapped channels are skipped. Selected channels keep their original global timing.

## Export

```bash
serial-repack export --in capture.srp --out-dir capture_out
```

The export contains one folder per channel:

```text
capture_out/radar_a/data.bin
capture_out/radar_a/timestamps_ns.bin
```

MATLAB parsing scripts are kept separately and should read this data using the original TOML config.

This repo includes one standalone helper:

```matlab
addpath('matlab')
capture = load_serial_repack_export('examples/multi_channel.toml', 'capture_out');
```

## Inspect

```bash
serial-repack inspect --in capture.srp
```

## Documentation

- [Configuration](docs/config.md)
- [Log Format](docs/log-format.md)
- [Replay](docs/replay.md)
- [Export](docs/matlab-export.md)
- [Development](docs/development.md)
- [Release](docs/release.md)


`--sync-every` controls how many packets are buffered before forcing a metadata+data sync to disk (default `1`).
