# Firmware source

This repository builds **stock Cornix right and left only**. Run `./build.sh [right|left|all]` from the repository root.

- `corex-rmk-pair/right/keyboard-stock.toml`, `vial-stock.json`, and `src/stock.rs`: stock Cornix central (USB on the right).
- `corex-rmk-pair/left/keyboard.toml`: stock Cornix peripheral.
- `corex-rmk-pair/shared/`: pairing/status display support shared by the board sources.
- `corex-rmk-upstream/`: pinned RMK crates with the inherited CoreX changes. No sibling repository or submodule is required.

The source snapshot retains the original shared board drivers, configuration references, and host regression tests. They are not extra firmware download targets. Replacement-PCB releases remain in [CoreX-RMK](https://github.com/yuchamichami/CoreX-RMK).

See [BUILDING.md](../BUILDING.md), [provenance.json](../provenance.json), and [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md). Existing license and copyright declarations are preserved.
