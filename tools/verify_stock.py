#!/usr/bin/env python3
"""Verify the two stock Cornix distribution images without touching hardware."""
import hashlib
import json
import lzma
import struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

def verify():
    folder=ROOT/'firmware'
    manifest=json.loads((folder/'manifest.json').read_text())
    assert manifest['status']=='experimental-hardware-unverified'
    assert len(manifest['images'])==2 and {i['side'] for i in manifest['images']}=={'right','left'}
    assert {p.name for p in folder.glob('*.uf2')}=={i['filename'] for i in manifest['images']}
    profile=manifest['configuration_profile'];definition=(folder/'definition.xz').read_bytes()
    assert profile['uid']==b'CoreXST1'.hex() and profile['model']=='cornix-stock'
    assert profile['definitionSize']==len(definition)
    assert profile['definitionSha256']==hashlib.sha256(definition).hexdigest()
    assert not profile['pointingDevices'] and not profile['gestures']
    assert json.loads(lzma.decompress(definition))==json.loads((ROOT/'source/corex-rmk-pair/right/vial-stock.json').read_text())
    for image in manifest['images']:
        raw=(folder/image['filename']).read_bytes()
        assert hashlib.sha256(raw).hexdigest()==image['sha256']
        assert raw and len(raw)%512==0
        count=len(raw)//512;base=0x1000;end=base+count*256
        assert end<=0xA0000
        assert image['flash_start']==hex(base) and image['flash_end_exclusive']==hex(end)
        data=bytearray()
        for index in range(count):
            block=raw[index*512:(index+1)*512]
            assert struct.unpack_from('<8I',block)==(0x0A324655,0x9E5D5157,0x2000,base+index*256,256,index,count,0xADA52840)
            assert struct.unpack_from('<I',block,508)[0]==0x0AB16F30
            data.extend(block[32:288])
        stack,reset=struct.unpack_from('<II',data)
        assert 0x20000008<=stack<=0x20040000 and stack%8==0
        assert reset&1 and base<=(reset&~1)<end
        for private in [b'/Users/',b'/Volumes/',b'/home/',b'\\Users\\']:assert private not in data
        if image['side']=='right':assert definition in data and b'Cornix CoreX' in data
        print('PASS stock distribution:',image['filename'])
    assert (folder/'SHA256SUMS').read_text()==''.join(f"{i['sha256']}  {i['filename']}\n" for i in manifest['images'])

if __name__=='__main__':verify()
