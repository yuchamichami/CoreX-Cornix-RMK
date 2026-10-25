# Firmware source

- `corex-rmk-pair/right/`: CoreX R A14 central, with PAW3222 on J3 and J4. The root build command selects the production-board configuration.
- `corex-rmk-pair/left/`: CoreX L Rev1.0 and stock Cornix left peripherals. Use `./build.sh left` for CoreX left and `./build.sh stock-left` for stock Cornix left; their firmware files are not interchangeable.
- `corex-rmk-upstream/`: four RMK crates pinned to commit `8a6889854fb996be592c55075b385234133e1772`, with the CoreX patches described in [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).

Use the repository-root [`build.sh`](../build.sh) to preserve the settings schema and remove local paths from distributable firmware. See [BUILDING.md](../BUILDING.md).

Keyboard Cargo manifests retain their existing `MIT OR Apache-2.0` declarations. File-specific notices, including the Apache-2.0 PAW3222 wire implementation, take precedence.

Current firmware versions are listed in the root [README](../README.md#ダウンロード) and [release manifest](../firmware/manifest.json).
