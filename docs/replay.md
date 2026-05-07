# Replay

Replay maps channel names in a capture file to target serial ports.

```bash
serial-repack replay \
  --in capture.srp \
  --map radar_a=/dev/ttyUSB3 \
  --map radar_b=COM7
```

Only channels listed with `--map` are replayed. Unmapped channels are skipped.

## Timing

Replay keeps the original global time axis:

- packet order is sorted by timestamp
- selected channels keep their relative timing from the original capture
- if the first selected packet happened later than the first global packet, replay waits for that initial offset

The default speed is `1.0`. A future version may expose other speed modes more fully; the current CLI already accepts `--speed`.

## Packet Data

Replay writes the stored complete packet bytes directly to the mapped serial port. It does not regenerate headers, tails, checksums, or payloads.
