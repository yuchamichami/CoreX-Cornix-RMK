#!/usr/bin/env python3
"""Package the stock Cornix experiment separately from the CoreX release."""
import hashlib
import json
import lzma
import re
import shutil
from pathlib import Path
from make_uf2 import convert
from stock_cornix import generate

ROOT = Path(__file__).resolve().parents[1]

def main():
    version = (ROOT / 'source/corex-rmk-pair/VERSION').read_text().strip()
    config, vial = generate()
    assert config == (ROOT / 'source/corex-rmk-pair/right/keyboard-stock.toml').read_text()
    generated = max((ROOT / 'build/target/stock-right').glob('thumbv7em-none-eabihf/release/build/corex-pair-right-*/out/config_generated.rs'), key=lambda p:p.stat().st_mtime).read_text()
    def array(name):
        return bytes(map(int,re.findall(r'(\d+)u8',re.search(name+r':.*?= &\[(.*?)\];',generated).group(1))))
    definition,uid = array('VIAL_KEYBOARD_DEF'),array('VIAL_KEYBOARD_ID')
    assert uid == b'CoreXST1'
    assert json.loads(lzma.decompress(definition)) == vial
    major,minor,patch = map(int,version.split('.'))
    contract = dict(model='cornix-stock',uid=uid.hex(),firmware=major<<16|minor<<8|patch,
                    pointingDevices=[],gestures=False,definitionSize=len(definition),
                    definitionSha256=hashlib.sha256(definition).hexdigest())
    images=[]
    for half,target in [('Right','stock-right'),('Left','left')]:
        stem=ROOT/'build/firmware'/f'Cornix-Stock-{half}-v{version}'
        binary,elf,uf2=tuple(Path(str(stem)+suffix).read_bytes() for suffix in ['.bin','.elf','.uf2'])
        rebuilt,start,end=convert(target,binary,elf)
        assert uf2 == rebuilt
        if half=='Right':
            assert definition in binary and b'Cornix CoreX' in binary
        else:
            # Reuse the checked-in stock peripheral without changing its code.
            assert uf2 == (ROOT/'firmware'/f'{stem.name}.uf2').read_bytes()
        images.append(dict(filename=stem.name+'.uf2',side=half.lower(),sha256=hashlib.sha256(uf2).hexdigest(),flash_start=hex(start),flash_end_exclusive=hex(end)))
    out=ROOT/'firmware/cornix-stock-preview'
    out.mkdir(exist_ok=True)
    for image in images: shutil.copy2(ROOT/'build/firmware'/image['filename'],out/image['filename'])
    (out/'definition.xz').write_bytes(definition)
    (out/'manifest.json').write_text(json.dumps(dict(status='experimental-hardware-unverified',version=version,configuration_profile=contract,images=images),indent=2)+'\n')
    (out/'SHA256SUMS').write_text(''.join(f"{i['sha256']}  {i['filename']}\n" for i in images))
    print(json.dumps(contract,indent=2))
    print('PASS stock pair: linked ELF, UF2 vectors/bounds, compiled Vial definition; hardware untested')

if __name__=='__main__': main()
