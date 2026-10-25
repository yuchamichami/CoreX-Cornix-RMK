#!/usr/bin/env python3
"""UF2 conversion must preserve the ELF's link address and exact flash bytes."""
import os
from pathlib import Path
import struct
import subprocess
import sys
import unittest

from make_uf2 import convert

ROOT = Path(__file__).resolve().parents[1]


def application(base=0x27000):
    # A Thumb reset handler at +0x100 is valid under both raw v6/v7 range
    # checks. Only the ELF link address can reject moving v7 down to v6.
    return struct.pack('<II', 0x20010000, base + 0x101) + b'\0' * 248 + b'code' * 16


def executable(segments, entry, machine=40):
    """Build a minimal ARM ELF; each segment is (physical, virtual, bytes, flags)."""
    offset = 52 + 32 * len(segments)
    headers, payloads = [], []
    for physical, virtual, payload, flags in segments:
        headers.append(struct.pack('<8I', 1, offset, virtual, physical,
                                   len(payload), len(payload), flags, 4))
        payloads.append(payload)
        offset += len(payload)
    ident = b'\x7fELF\x01\x01\x01' + b'\0' * 9
    header = struct.pack('<16sHHIIIIIHHHHHH', ident, 2, machine, 1, entry,
                         52, 0, 0, 52, 32, len(segments), 0, 0, 0)
    return header + b''.join(headers) + b''.join(payloads)


def linked_application(base=0x27000):
    data = application(base)
    return data, executable([(base, base, data, 5)], base + 0x101)


class FirmwareConversionTests(unittest.TestCase):
    def test_each_supported_target_preserves_payload_and_address(self):
        for side, base in [('stock-right', 0x1000), ('left', 0x1000), ('raytac-right-26000', 0x26000),
                           ('raytac-right-27000', 0x27000),
                           ('raytac-left-26000', 0x26000), ('raytac-left-27000', 0x27000)]:
            with self.subTest(side=side):
                data, elf = linked_application(base)
                uf2, start, end = convert(side, data, elf)
                self.assertEqual((start, end), (base, base + 512))
                self.assertEqual(struct.unpack_from('<I', uf2, 12)[0], base)
                self.assertEqual(uf2[32:288] + uf2[544:608], data)

    def test_v7_cannot_be_relocated_down_to_v6(self):
        data, elf = linked_application()
        with self.assertRaisesRegex(ValueError, 'ELF link base'):
            convert('raytac-right-26000', data, elf)

    def test_v6_cannot_be_relocated_up_to_v7(self):
        data, elf = linked_application(0x26000)
        with self.assertRaisesRegex(ValueError, 'ELF load address'):
            convert('raytac-right-27000', data, elf)

    def test_mismatched_bin_rejected_even_with_same_valid_vector(self):
        data, elf = linked_application()
        with self.assertRaisesRegex(ValueError, 'Binary does not match'):
            convert('raytac-right-27000', data[:-1] + b'X', elf)

    def test_ram_initializers_use_physical_flash_address_and_zero_filled_gaps(self):
        base = 0x27000
        code = application(base)
        data = code + b'\0' * 16 + b'RAM!'
        elf = executable([(base, base, code, 5),
                          (base + len(code) + 16, 0x20010000, b'RAM!', 6)], base + 0x101)
        uf2, _, _ = convert('raytac-right-27000', data, elf)
        self.assertEqual((uf2[32:288] + uf2[544:800])[:len(data)], data)

    def test_protected_flash_load_rejected(self):
        data = application()
        elf = executable([(0x27000, 0x27000, data, 5),
                          (0xB0000, 0xB0000, b'settings', 4)], 0x27101)
        with self.assertRaisesRegex(ValueError, 'ELF load address'):
            convert('raytac-right-27000', data, elf)

    def test_overlapping_load_segments_rejected(self):
        data = application()
        elf = executable([(0x27000, 0x27000, data, 5),
                          (0x27100, 0x27100, b'other', 4)], 0x27101)
        with self.assertRaisesRegex(ValueError, 'Overlapping'):
            convert('raytac-right-27000', data, elf)

    def test_truncated_and_wrong_architecture_elf_rejected(self):
        data, elf = linked_application()
        wrong_machine = executable([(0x27000, 0x27000, data, 5)], 0x27101, machine=3)
        for bad in (b'', elf[:51], elf[:75], elf[:-1], wrong_machine):
            with self.subTest(length=len(bad)), self.assertRaises(ValueError):
                convert('raytac-right-27000', data, bad)

    def test_entry_must_match_the_reset_vector(self):
        data = application()
        elf = executable([(0x27000, 0x27000, data, 5)], 0x27105)
        with self.assertRaisesRegex(ValueError, 'reset vector differ'):
            convert('raytac-right-27000', data, elf)

    def test_cli_requires_elf_before_reading_input_or_writing_output(self):
        result = subprocess.run([sys.executable, str(ROOT / 'tools/make_uf2.py'),
                                 'left', 'missing.bin', 'must-not-be-created.uf2'],
                                cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn('--elf', result.stderr)
        self.assertFalse((ROOT / 'must-not-be-created.uf2').exists())


class BoardSelectionTests(unittest.TestCase):
    def test_ambiguous_and_proto_right_builds_rejected_before_toolchain_lookup(self):
        for args in ([], ['right'], ['both']):
            with self.subTest(args=args):
                env = dict(os.environ, COREX_BOARD='prototype')
                result = subprocess.run(['/bin/sh', str(ROOT / 'tools/build-board.sh'), *args],
                                        cwd=ROOT, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 2)
                self.assertIn('Use ./build.sh', result.stderr)

    def test_board_and_side_must_match(self):
        for board, side in [('raytac-r-a14', 'left'), ('raytac-r-a14', 'right'), ('raytac-l-rev1', 'left'), ('raytac-l-rev1', 'right'), ('cornix-stock-right', 'left')]:
            with self.subTest(board=board):
                result = subprocess.run(['/bin/sh', str(ROOT / 'tools/build-board.sh'), side],
                                        cwd=ROOT, env=dict(os.environ, COREX_BOARD=board),
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, 2)
                self.assertIn('stock Cornix only', result.stderr)


if __name__ == '__main__':
    unittest.main()
