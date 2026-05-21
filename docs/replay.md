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

The default speed is `1.0`.

### Time Window

`--from` 和 `--to` 传入 **Unix 时间戳（秒）**。程序会在已选通道的数据里找到最接近该时间戳的数据点。

- `--from`: 回放起点（取最近数据点）
- `--to`: 回放终点（可选，也取最近数据点）
- if `--to` is omitted, replay continues to the end

## Packet Data

Replay writes the stored complete packet bytes directly to the mapped serial port. It does not regenerate headers, tails, checksums, or payloads.
