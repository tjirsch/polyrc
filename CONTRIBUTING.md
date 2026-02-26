# Contributing to polyrc

Thank you for your interest in contributing!

## Getting started

```bash
git clone https://github.com/tjirsch/polyrc
cd polyrc
cargo build
cargo test
```

## Adding a new format

1. Add the format variant to `FormatArg` in `src/cli.rs`
2. Create `src/formats/<name>.rs` implementing `Parser` and `Writer`
3. Register it in `src/formats/mod.rs`
4. Add format info to `src/supported_formats.rs`
5. Add discovery entries in `src/discover.rs`
6. Update `docs/formats.md` and `README.md`

## Pull requests

- One format or feature per PR
- `cargo test` must pass
- Keep `CLAUDE.md` up to date if the architecture changes

## License

By contributing you agree your contributions are licensed under the [MIT License](LICENSE).
