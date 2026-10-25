#!/bin/sh
# Build stock Cornix applications only. Never accesses connected hardware.
set -eu
COREX_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
COREX_SIDE=${1:-all}
case "$COREX_SIDE" in stock-right) COREX_SIDE=right ;; stock-left) COREX_SIDE=left ;; esac
case "$COREX_SIDE" in right|left|all) ;; *) echo 'Usage: ./build.sh [right|left|all]' >&2; exit 2 ;; esac
[ "$#" -le 1 ] || { echo 'Stock Cornix does not use s140v6/s140v7 variants' >&2; exit 2; }
unset COREX_BOARD COREX_RAYTAC_APP_BASE KEYBOARD_TOML_PATH
python3 "$COREX_ROOT/tools/stock_cornix.py" --check
for COREX_HALF in right left; do
    if [ "$COREX_SIDE" != all ] && [ "$COREX_SIDE" != "$COREX_HALF" ]; then continue; fi
    COREX_TARGET="${CARGO_TARGET_DIR:-$COREX_ROOT/build/target}/stock-$COREX_HALF"
    case "$COREX_HALF" in right) COREX_BOARD=cornix-stock-right ;; left) COREX_BOARD=prototype ;; esac
    COREX_BOARD="$COREX_BOARD" CARGO_TARGET_DIR="$COREX_TARGET" "$COREX_ROOT/tools/build-board.sh" "$COREX_HALF"
done
