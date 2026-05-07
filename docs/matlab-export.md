# Export

Export command:

```bash
serial-repack export --in capture.srp --out-dir capture_out
```

Output layout:

```text
capture_out/
  radar_a/
    data.bin
    timestamps_ns.bin
  radar_b/
    data.bin
    timestamps_ns.bin
```

## Files

- `data.bin`: complete packets stored consecutively, including header and tail.
- `timestamps_ns.bin`: little-endian `uint64` Unix timestamps in ns.

`serial-repack` does not generate MATLAB `.m` files. Use the standalone helper script in `matlab/load_serial_repack_export.m` or keep your own parser separately.

## Binary Shapes

For each enabled channel:

- `data.bin` size is `packet_len * packet_count` bytes.
- `timestamps_ns.bin` size is `8 * packet_count` bytes.

The standalone helper can load all enabled channels using the original TOML config:

```matlab
addpath('matlab')
capture = load_serial_repack_export('examples/multi_channel.toml', 'capture_out');
```

It returns:

```matlab
capture.radar_a.data
capture.radar_a.timestamps_ns
capture.radar_a.timestamps_s
```

A custom MATLAB-side parser should get `packet_len` and enabled channel names from the original TOML config, then read:

```matlab
fid = fopen(fullfile(export_dir, channel_name, 'data.bin'), 'rb', 'ieee-le');
data = fread(fid, [packet_len, packet_count], '*uint8');
fclose(fid);
```

and:

```matlab
fid = fopen(fullfile(export_dir, channel_name, 'timestamps_ns.bin'), 'rb', 'ieee-le');
timestamps_ns = fread(fid, inf, '*uint64');
fclose(fid);
```
