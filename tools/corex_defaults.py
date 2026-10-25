#!/usr/bin/env python3
"""Map the shared Vial preset to the production board's physical matrix."""
import argparse
import copy
import json
import re
import tomllib
from pathlib import Path

from default_keymap import generate, rmk_action

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'keymaps/CoreX-default.vil'


def generate_corex():
    source = generate()
    result = copy.deepcopy(source)
    result['uid'] = int.from_bytes(b'CoreXRY1', 'little')
    board = ROOT / 'source/corex-rmk-pair/right'
    base = tomllib.loads((board / 'keyboard.toml').read_text())
    config = tomllib.loads((board / 'keyboard-raytac.toml').read_text())
    positions = lambda text: [tuple(map(int, cell)) for cell in re.findall(r'\((\d+),(\d+)\)', text)]
    old, new = positions(base['layout']['map']), positions(config['layout']['map'])
    assert len(old) == len(set(old)) == len(new) == len(set(new)) == 56
    for layer, keys in enumerate(source['layout']):
        for (r, c), (nr, nc) in zip(old, new):
            result['layout'][layer][nr][nc] = keys[r][c]
        expected = [rmk_action(result['layout'][layer][r][c]) for r,c in new]
        assert expected == config['keymap']['layer'][layer]['keys'].split(), 'Preset differs from firmware defaults'
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(generate_corex(), ensure_ascii=False, indent=2) + '\n'
    if args.check:
        assert DEST.read_text() == text, 'Run python3 tools/corex_defaults.py'
    else:
        DEST.write_text(text)
    print('PASS CoreX preset: UID and all 392 cells match the production board')


if __name__ == '__main__':
    main()
