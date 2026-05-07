# Development

## Commands

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs these checks on Linux and Windows.

## Structure

- `src/cli.rs`: CLI definitions.
- `src/config.rs`: TOML parsing and validation.
- `src/packet.rs`: fixed-length frame parser.
- `src/log_format.rs`: `.srp` binary read/write.
- `src/recorder.rs`: multi-channel recording orchestration and testable reader core.
- `src/replay.rs`: replay mapping and scheduler.
- `src/matlab_export.rs`: MATLAB file export.
- `src/serial_io.rs`: real serial port adapter.

## Git Practice

Use small commits grouped by behavior:

- initialize project and CLI
- add config parsing
- add packet parser
- add log read/write
- add multi-channel recording
- add replay
- add MATLAB export
- add docs and tests

Suggested feature branch:

```bash
git checkout -b feature/multi-serial-recording
```

Default automated tests do not require real serial hardware. Manual hardware tests should verify recording and replay on the target OS.
