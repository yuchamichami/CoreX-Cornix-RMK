#!/usr/bin/env python3
"""Stock Cornix physical contract, checked against pinned manufacturer data."""
import hashlib
import json
import re
import tomllib
import unittest
from pathlib import Path
from test_default_keymap import normalize
ROOT=Path(__file__).resolve().parents[1]
BOARD=ROOT/'source/corex-rmk-pair/right'
CONFIG=tomllib.loads((BOARD/'keyboard-stock.toml').read_text())

class StockCornix(unittest.TestCase):
    def test_every_manufacturer_assignment_is_preserved(self):
        stock=json.loads((ROOT/'keymaps/reference/cornix-default-keymap.vil').read_text())
        self.assertEqual(CONFIG['keymap']['layers'],10)
        for layer,original in zip(CONFIG['keymap']['layer'],stock['layout'],strict=True):
            cells=layer['keys'].split()
            for row in range(8):
                for col in range(7):
                    value=original[(row+4)%8][col]
                    self.assertEqual(cells[row*7+col],normalize('KC_NO' if value==-1 else value))
        for index,layer in enumerate(CONFIG['keymap']['layer']):
            for new,old in [(0,1),(1,0)]:
                self.assertEqual(layer['encoders'][new],[normalize(v) for v in reversed(stock['encoder_layout'][index][old])])

    def test_pins_match_stock_right_and_have_no_shared_owners(self):
        central=CONFIG['split']['central'];matrix=central['matrix']
        self.assertEqual(matrix['row_pins'],['P1_09','P0_28','P0_03','P1_10'])
        self.assertEqual(matrix['col_pins'],['P0_30','P0_31','P0_29','P0_02','P1_13','P0_10','P0_09'])
        self.assertFalse(matrix['row2col'])
        encoder=central['input_device']['encoder'][0]
        self.assertEqual([encoder['pin_a'],encoder['pin_b'],encoder['resolution']],['P1_04','P1_06',4])
        pins=matrix['row_pins']+matrix['col_pins']+[encoder['pin_a'],encoder['pin_b'],central['battery_adc_pin'],'P0_24','P0_13']
        self.assertEqual(len(pins),len(set(pins)))
        self.assertNotIn('auto_mouse_layer',CONFIG['behavior'])
        self.assertEqual(CONFIG['storage']['start_addr'],0xC0000)

    def test_fifty_physical_keys_and_two_encoders_have_correct_addresses(self):
        vial=json.loads((BOARD/'vial-stock.json').read_text())
        physical=[];encoders=[]
        for row in vial['layouts']['keymap']:
            for item in row:
                if isinstance(item,str):
                    pos=tuple(map(int,item.split('\n')[0].split(',')))
                    (encoders if '\ne' in item else physical).append(pos)
        self.assertEqual(len(set(physical)),50)
        self.assertEqual(set(encoders),{(0,0),(0,1),(1,0),(1,1)})
        for pos in [(0,5),(1,6),(3,3),(3,4),(3,5),(6,6)]: self.assertIn(pos,physical)
        cells=CONFIG['keymap']['layer'][0]['keys'].split()
        self.assertEqual(cells[1*7+5],'H')
        self.assertEqual(cells[3*7+4],'MO(4)')
        self.assertEqual(len(vial['customKeycodes']),8)

    def test_packaged_contract_matches_sources(self):
        import lzma
        folder=ROOT/'firmware/cornix-stock-preview'
        manifest=json.loads((folder/'manifest.json').read_text());definition=(folder/'definition.xz').read_bytes()
        self.assertEqual(json.loads(lzma.decompress(definition)),json.loads((BOARD/'vial-stock.json').read_text()))
        self.assertEqual(manifest['configuration_profile']['definitionSha256'],hashlib.sha256(definition).hexdigest())
        for image in manifest['images']:self.assertEqual(image['sha256'],hashlib.sha256((folder/image['filename']).read_bytes()).hexdigest())
        self.assertEqual((folder/'Cornix-Stock-Left-v0.11.4.uf2').read_bytes(),(ROOT/'firmware/Cornix-Stock-Left-v0.11.4.uf2').read_bytes())

if __name__=='__main__':unittest.main()
