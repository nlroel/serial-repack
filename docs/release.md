# Release

Releases are created from Git tags.

## Targets

The release workflow builds:

- Linux: `x86_64-unknown-linux-gnu`
- Windows: `x86_64-pc-windows-msvc`

Each archive contains:

- `serial-repack` or `serial-repack.exe`
- `README.md`
- `LICENSE`
- `examples/`
- `docs/`
- `matlab/`

## Create A Release

Update `Cargo.toml` version first, then tag the commit:

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions will build both platform archives and publish a GitHub Release.

## Platform Notes

Linux serial ports usually look like `/dev/ttyUSB0` or `/dev/ttyACM0`.

Windows serial ports usually look like `COM3`, `COM7`, etc.

The code opens explicit port names from config or replay mappings. It does not require serial-port enumeration, which keeps Linux builds independent of `libudev`.
